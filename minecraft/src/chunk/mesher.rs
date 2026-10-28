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
use nalgebra::{Point3, Vector3};
use rayon::ThreadPool;
use wgpu::util::DeviceExt;

use crate::{
    assets::Assets,
    chunk::renderer::Instance,
    config::{
        CHUNK_OFFSET_Y, CHUNK_RENDER_AFTER, CHUNK_RENDER_DISTANCE, CHUNK_WIDTH_X, CHUNK_WIDTH_Y,
        CHUNK_WIDTH_Z,
    },
    primitives::{BlockData, Chunk, World},
};

pub struct ChunkMesher {
    thread_pool: Rc<ThreadPool>,
    tx: Sender<MeshTaskOutput>,
    rx: Receiver<MeshTaskOutput>,
    tasks: HashSet<u64>,
    assets: Arc<Assets>,
    pub meshes: HashMap<u64, ChunkMesh>,
}

pub struct ChunkMesh {
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

impl ChunkMesher {
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
                ChunkMesh {
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
        let rel_pos = pos - &Chunk::get_bottom_left(x, z);

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
                if !Chunk::should_render(c.x, c.z, &pos) {
                    return Some((c.x, c.z));
                }
                None
            })
            .collect::<Vec<(i32, i32)>>();

        for c in invisible {
            self.unload_mesh(c.0, c.1);
        }

        // load visible chunks
        let (cx, cz) = Chunk::get_xz_from_vec3(&pos);

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
        if !self.tasks.insert(cid.clone()) {
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
            contents: bytemuck::cast_slice(&contents),
            usage: wgpu::BufferUsages::VERTEX,
        })
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

                    let b = chunk.get_block_data(&Point3::new(wx, wy, wz)).unwrap();
                    if b.is_air() {
                        continue;
                    }

                    let key = b.get_model().unwrap();
                    let model = task.assets.model_registry.models.get(key).unwrap();

                    // get neighbors
                    let xpos = task.get_block_data(&Point3::new(wx + 1, wy, wz));
                    let xneg = task.get_block_data(&Point3::new(wx - 1, wy, wz));
                    let ypos = task.get_block_data(&Point3::new(wx, wy + 1, wz));
                    let yneg = task.get_block_data(&Point3::new(wx, wy - 1, wz));
                    let zpos = task.get_block_data(&Point3::new(wx, wy, wz + 1));
                    let zneg = task.get_block_data(&Point3::new(wx, wy, wz - 1));

                    if xpos.is_some_and(|b| b.is_air()) {
                        instances.push(Instance::new(model.offset, x, y, z));
                    }
                    if xneg.is_some_and(|b| b.is_air()) {
                        instances.push(Instance::new(model.offset + 1, x, y, z));
                    }
                    if ypos.is_some_and(|b| b.is_air()) {
                        instances.push(Instance::new(model.offset + 2, x, y, z));
                    }
                    if yneg.is_some_and(|b| b.is_air()) {
                        instances.push(Instance::new(model.offset + 3, x, y, z));
                    }
                    if zpos.is_some_and(|b| b.is_air()) {
                        instances.push(Instance::new(model.offset + 4, x, y, z));
                    }
                    if zneg.is_some_and(|b| b.is_air()) {
                        instances.push(Instance::new(model.offset + 5, x, y, z));
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
