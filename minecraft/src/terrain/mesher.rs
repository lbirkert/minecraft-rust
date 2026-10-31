use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    time::Instant,
};

use log::info;
use nalgebra::Point3;
use rayon::ThreadPool;
use wgpu::util::DeviceExt;

use crate::{
    assets::{
        Assets,
        model::{Cullface, ModelQuad},
    },
    config::{
        CHUNK_OFFSET_Y, CHUNK_RENDER_AFTER, CHUNK_RENDER_DISTANCE, CHUNK_WIDTH_X, CHUNK_WIDTH_Y,
        CHUNK_WIDTH_Z,
    },
    primitives::{BlockData, Chunk, World},
    renderer::terrain::Instance,
};

pub struct TerrainMesher {
    thread_pool: Rc<ThreadPool>,
    tx: Sender<MeshTaskOutput>,
    rx: Receiver<MeshTaskOutput>,
    tasks: HashSet<u64>,
    assets: Arc<Assets>,
    pub meshes: HashMap<u64, TerrainMesh>,
}

pub struct TerrainMesh {
    pub x: i32,
    pub z: i32,
    pub buffer: wgpu::Buffer,
    pub loaded_at: Instant,
    pub has_neighbors: [bool; 4],
    reload: bool,
}

struct MeshTask {
    chunk: Arc<Chunk>,
    assets: Arc<Assets>,
    xpos: Option<Arc<Chunk>>,
    xneg: Option<Arc<Chunk>>,
    zpos: Option<Arc<Chunk>>,
    zneg: Option<Arc<Chunk>>,
    has_neighbors: [bool; 4],
}

struct MeshTaskOutput {
    x: i32,
    z: i32,
    instances: Vec<Instance>,
    has_neighbors: [bool; 4],
}

impl MeshTask {
    pub fn get_block_data(&self, pos: &Point3<i32>) -> Option<&BlockData> {
        let (mut cx, mut cz) = Chunk::get_xz_from_vec3(pos);
        cx -= self.chunk.x;
        cz -= self.chunk.z;

        match (cx, cz, &self.xpos, &self.xneg, &self.zpos, &self.zneg) {
            (0, 0, _, _, _, _) => self.chunk.get_block_data(pos),
            (1, 0, Some(xpos), _, _, _) => xpos.get_block_data(pos),
            (-1, 0, _, Some(xneg), _, _) => xneg.get_block_data(pos),
            (0, 1, _, _, Some(zpos), _) => zpos.get_block_data(pos),
            (0, -1, _, _, _, Some(zneg)) => zneg.get_block_data(pos),
            _ => None,
        }
    }
}

impl TerrainMesher {
    pub fn new(thread_pool: Rc<ThreadPool>, assets: Arc<Assets>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            thread_pool,
            tx,
            rx,
            assets,
            tasks: Default::default(),
            meshes: Default::default(),
        }
    }

    pub fn process_task_outputs(&mut self, device: &wgpu::Device) {
        if let Ok(output) = self.rx.try_recv() {
            let buffer = Self::create_instance_buffer(device, &output.instances);
            let cid = Chunk::get_id(output.x, output.z);
            self.tasks.remove(&cid);
            self.meshes.insert(
                cid,
                TerrainMesh {
                    has_neighbors: output.has_neighbors,
                    x: output.x,
                    z: output.z,
                    loaded_at: Instant::now(),
                    buffer,
                    reload: false,
                },
            );
        }
    }

    pub fn reload_mesh(&mut self, x: i32, z: i32) -> Option<()> {
        let cid = Chunk::get_id(x, z);
        let mesh = self.meshes.get_mut(&cid)?;
        mesh.reload = true;
        Some(())
    }

    pub fn block_update(&mut self, pos: &Point3<i32>) -> Option<()> {
        let (x, z) = Chunk::get_xz_from_vec3(pos);
        let rel_pos = pos - Chunk::get_bottom_left(x, z);

        if rel_pos.x == 0 {
            self.reload_mesh(x - 1, z);
        }

        if rel_pos.x == 15 {
            self.reload_mesh(x + 1, z);
        }

        if rel_pos.z == 0 {
            self.reload_mesh(x, z - 1);
        }

        if rel_pos.z == 15 {
            self.reload_mesh(x, z + 1);
        }

        self.reload_mesh(x, z)
    }

    // load/unload meshes for a player
    pub fn update_chunks(&mut self, pos: &Point3<i32>, world: &World) {
        // unload invisible chunks
        let invisible = self
            .meshes
            .values()
            .filter_map(|c| {
                if !Chunk::should_render(c.x, c.z, pos) {
                    return Some((c.x, c.z));
                }
                None
            })
            .collect::<Vec<(i32, i32)>>();

        for c in invisible {
            self.unload_mesh(c.0, c.1);
        }

        // load visible chunks
        let (cx, cz) = Chunk::get_xz_from_vec3(pos);

        let now = Instant::now();

        for x in (cx - CHUNK_RENDER_DISTANCE)..=(cx + CHUNK_RENDER_DISTANCE) {
            for z in (cz - CHUNK_RENDER_DISTANCE)..=(cz + CHUNK_RENDER_DISTANCE) {
                let cid = Chunk::get_id(x, z);
                let Some(chunk) = world.chunk_loader.get_chunk_fast(x, z) else {
                    continue;
                };
                let xpos = world.chunk_loader.get_chunk_fast(x + 1, z);
                let xneg = world.chunk_loader.get_chunk_fast(x - 1, z);
                let zpos = world.chunk_loader.get_chunk_fast(x, z + 1);
                let zneg = world.chunk_loader.get_chunk_fast(x, z - 1);

                match self.meshes.get_mut(&cid) {
                    Some(mesh) => {
                        let has_neighbors = [
                            xpos.is_some(),
                            xneg.is_some(),
                            zpos.is_some(),
                            zneg.is_some(),
                        ];

                        // reload mesh if has_neighbors changed or reload requested
                        if mesh.has_neighbors != has_neighbors || mesh.reload {
                            self.load_mesh(
                                chunk.clone(),
                                self.assets.clone(),
                                xpos.cloned(),
                                xneg.cloned(),
                                zpos.cloned(),
                                zneg.cloned(),
                            );
                        }
                    }
                    None => {
                        let has_all_neighbors =
                            xpos.is_some() && xneg.is_some() && zpos.is_some() && zneg.is_some();
                        if has_all_neighbors
                            || (now - chunk.loaded_at).as_millis() > CHUNK_RENDER_AFTER
                        {
                            self.load_mesh(
                                chunk.clone(),
                                self.assets.clone(),
                                xpos.cloned(),
                                xneg.cloned(),
                                zpos.cloned(),
                                zneg.cloned(),
                            );
                        }
                    }
                }
            }
        }
    }

    fn load_mesh(
        &mut self,
        chunk: Arc<Chunk>,
        assets: Arc<Assets>,
        xpos: Option<Arc<Chunk>>,
        xneg: Option<Arc<Chunk>>,
        zpos: Option<Arc<Chunk>>,
        zneg: Option<Arc<Chunk>>,
    ) -> bool {
        let cid = Chunk::get_id(chunk.x, chunk.z);
        if !self.tasks.insert(cid) {
            return false;
        }

        self.spawn_mesh_task(MeshTask {
            has_neighbors: [
                xpos.is_some(),
                xneg.is_some(),
                zpos.is_some(),
                zneg.is_some(),
            ],
            chunk,
            assets,
            xpos,
            xneg,
            zpos,
            zneg,
        });

        true
    }

    fn unload_mesh(&mut self, x: i32, z: i32) {
        let cid = Chunk::get_id(x, z);
        self.meshes.remove(&cid);
    }

    fn spawn_mesh_task(&self, task: MeshTask) {
        let tx = self.tx.clone();
        self.thread_pool.spawn(move || {
            Self::run_mesh_task(task, tx);
        });
    }

    fn create_instance_buffer(device: &wgpu::Device, contents: &[Instance]) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk instance buffer"),
            contents: bytemuck::cast_slice(contents),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    fn is_quad_occluded(task: &MeshTask, pos: &Point3<i32>, quad: &ModelQuad) -> bool {
        let Some(block_data) = (match quad.cullface {
            Cullface::East => task.get_block_data(&Point3::new(pos.x + 1, pos.y, pos.z)),
            Cullface::West => task.get_block_data(&Point3::new(pos.x - 1, pos.y, pos.z)),
            Cullface::Up => task.get_block_data(&Point3::new(pos.x, pos.y + 1, pos.z)),
            Cullface::Down => task.get_block_data(&Point3::new(pos.x, pos.y - 1, pos.z)),
            Cullface::South => task.get_block_data(&Point3::new(pos.x, pos.y, pos.z + 1)),
            Cullface::North => task.get_block_data(&Point3::new(pos.x, pos.y, pos.z - 1)),
            Cullface::None => return false,
        }) else {
            return false;
        };

        let Some(model) = block_data.get_model() else {
            return false;
        };
        let model = task.assets.model_registry.models.get(model).unwrap();

        let other_cullface = quad.cullface.rev();
        let quads = model.quads.iter().filter(|q| q.cullface == other_cullface);
        for other_quad in quads {
            if ModelQuad::occludes(other_quad, quad) {
                return true;
            }
        }

        false
    }

    fn run_mesh_task(task: MeshTask, tx: Sender<MeshTaskOutput>) {
        let start = Instant::now();

        let chunk = &task.chunk;

        let mut instances = Vec::new();

        for y in 0..CHUNK_WIDTH_Y {
            for z in 0..CHUNK_WIDTH_Z {
                for x in 0..CHUNK_WIDTH_X {
                    let wx = x + chunk.x * CHUNK_WIDTH_X;
                    let wy = y + CHUNK_OFFSET_Y;
                    let wz = z + chunk.z * CHUNK_WIDTH_Z;

                    let pos = Point3::new(wx, wy, wz);
                    let b = chunk.get_block_data(&pos).unwrap();
                    if b.is_air() {
                        continue;
                    }

                    let key = b.get_model().unwrap();
                    let model = task.assets.model_registry.models.get(key).unwrap();

                    for (i, quad) in model.quads.iter().enumerate() {
                        if !Self::is_quad_occluded(&task, &pos, quad) {
                            instances.push(Instance::new(model.offset + i as u32, x, y, z));
                        }
                    }
                }
            }
        }

        let delta = Instant::now() - start;
        info!(
            "meshing chunk {} {} - took {}ms",
            chunk.x,
            chunk.z,
            delta.as_millis()
        );

        tx.send(MeshTaskOutput {
            has_neighbors: task.has_neighbors,
            x: chunk.x,
            z: chunk.z,
            instances,
        })
        .unwrap();
    }
}
