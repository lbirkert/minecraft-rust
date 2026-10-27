use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
};

use log::info;
use nalgebra::Point3;
use rayon::ThreadPool;

use crate::{chunk::generator::ChunkGenerator, config::CHUNK_LOAD_DISTANCE, primitives::Chunk};

struct LoadTaskOutput {
    chunk: Chunk,
}

struct LoadTask {
    x: i32,
    z: i32,
}

pub struct ChunkLoader {
    chunk_generator: Arc<ChunkGenerator>,
    loaded_chunks: HashMap<u64, Arc<Chunk>>,
    thread_pool: Rc<ThreadPool>,
    tx: Sender<LoadTaskOutput>,
    rx: Receiver<LoadTaskOutput>,
    tasks: HashSet<u64>,
}

impl ChunkLoader {
    pub fn new(thread_pool: Rc<ThreadPool>, seed: u32) -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            chunk_generator: Arc::new(ChunkGenerator::new(seed)),
            loaded_chunks: Default::default(),
            tasks: Default::default(),
            thread_pool,
            tx,
            rx,
        }
    }

    // TODO: this method is unsafe, blocks UI thread, for physics, blocking makes sense though. Impl
    // pub fn get_chunk(&mut self, x: i32, z: i32) -> Arc<Chunk> {
    //     match self.get_chunk_fast(x, z) {
    //         Some(chunk) => Arc::clone(chunk),
    //         None => Arc::clone(self.load_chunk(x, z)),
    //     }
    // }

    pub fn get_chunk_fast(&self, x: i32, z: i32) -> Option<&Arc<Chunk>> {
        let cid = Chunk::get_id(x, z);
        let chunk = self.loaded_chunks.get(&cid)?;
        assert_eq!(chunk.x, x);
        assert_eq!(chunk.z, z);
        Some(chunk)
    }

    pub fn is_chunk_loaded(&self, x: i32, z: i32) -> bool {
        let cid = Chunk::get_id(x, z);
        self.loaded_chunks.contains_key(&cid)
    }

    // load/unload chunks for a player
    pub fn update_chunks(&mut self, pos: &Point3<i32>) {
        // unload invisible chunks
        let invisible = self
            .loaded_chunks
            .values()
            .filter_map(|c| {
                if !Chunk::should_load(c.x, c.z, &pos) {
                    return Some((c.x, c.z));
                }
                None
            })
            .collect::<Vec<(i32, i32)>>();

        for c in invisible {
            self.unload_chunk(c.0, c.1);
        }

        // load visible chunks
        let (cx, cz) = Chunk::get_xz_from_vec3(&pos);

        for x in (cx - CHUNK_LOAD_DISTANCE)..=(cx + CHUNK_LOAD_DISTANCE) {
            for z in (cz - CHUNK_LOAD_DISTANCE)..=(cz + CHUNK_LOAD_DISTANCE) {
                let cid = Chunk::get_id(x, z);
                if !self.loaded_chunks.contains_key(&cid) {
                    self.load_chunk(x, z);
                }
            }
        }
    }

    pub fn process_task_outputs(&mut self) {
        if let Ok(output) = self.rx.try_recv() {
            let cid = Chunk::get_id(output.chunk.x, output.chunk.z);
            let chunk = Arc::new(output.chunk);
            self.tasks.remove(&cid);
            self.loaded_chunks.insert(cid, chunk);
        }
    }

    pub fn load_chunk(&mut self, x: i32, z: i32) {
        let cid = Chunk::get_id(x, z);
        if self.tasks.contains(&cid) {
            return;
        }

        self.spawn_load_task(LoadTask { x, z });
        self.tasks.insert(cid);
    }

    pub fn unload_chunk(&mut self, x: i32, z: i32) {
        info!("unloading chunk {} {}", x, z);
        let cid = Chunk::get_id(x, z);

        self.loaded_chunks.remove(&cid).unwrap();
    }

    fn spawn_load_task(&self, task: LoadTask) {
        let tx = self.tx.clone();
        let chunk_generator = self.chunk_generator.clone();
        self.thread_pool.spawn(move || {
            Self::run_load_task(task, tx, chunk_generator);
        });
    }

    fn run_load_task(task: LoadTask, tx: Sender<LoadTaskOutput>, generator: Arc<ChunkGenerator>) {
        // TODO: also check FS here maybe?
        let chunk = generator.generate_chunk(task.x, task.z);
        tx.send(LoadTaskOutput { chunk }).unwrap();
    }
}
