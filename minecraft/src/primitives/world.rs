use std::{collections::HashMap, rc::Rc};

use nalgebra::{Point3, Vector3};
use rayon::ThreadPool;

use crate::{
    chunk::ChunkLoader,
    primitives::{BlockData, Chunk, Player, PlayerId},
};

pub struct World {
    pub players: HashMap<PlayerId, Player>,
    pub chunk_loader: ChunkLoader,
}

impl World {
    pub fn new(thread_pool: Rc<ThreadPool>, seed: u32) -> Self {
        Self {
            players: Default::default(),
            chunk_loader: ChunkLoader::new(thread_pool, seed),
        }
    }

    pub fn add_player(&mut self, player: Player) {
        // assert that the player was not present (duplicate)
        assert!(self.players.insert(player.id, player).is_none());
    }

    pub fn get_block_data_fast(&self, pos: &Point3<i32>) -> Option<&BlockData> {
        let (x, z) = Chunk::get_xz_from_vec3(pos);
        let chunk = self.chunk_loader.get_chunk_fast(x, z)?;
        chunk.get_block_data(pos)
    }
}
