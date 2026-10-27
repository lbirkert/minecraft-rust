use std::time::Instant;

use nalgebra::Point3;

use crate::config::{
    CHUNK_LOAD_DISTANCE, CHUNK_OFFSET_Y, CHUNK_RENDER_DISTANCE, CHUNK_VOLUME, CHUNK_WIDTH_X,
    CHUNK_WIDTH_Z,
};
use crate::primitives::BlockData;

pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub block_data: Vec<BlockData>,
    // pub blocks: [u32; CHUNK_VOLUME], // 0.5 MiB
    pub blocks: Vec<u32>, // 0.5 MiB
    pub loaded_at: Instant,
}

impl Chunk {
    pub fn get_id(x: i32, z: i32) -> u64 {
        (x as u32 as u64) | ((z as u32 as u64) << 32)
    }

    pub fn get_xz_from_vec3(pos: &Point3<i32>) -> (i32, i32) {
        (
            pos.x.div_euclid(CHUNK_WIDTH_X),
            pos.z.div_euclid(CHUNK_WIDTH_Z),
        )
    }

    pub fn get_bottom_left(x: i32, z: i32) -> Point3<i32> {
        Point3::new(x * CHUNK_WIDTH_X, CHUNK_OFFSET_Y, z * CHUNK_WIDTH_Z)
    }

    pub fn get_block_id(pos: &Point3<i32>) -> Option<(i32, i32, u32)> {
        let (x, z) = Self::get_xz_from_vec3(pos);
        let bottom_left = Self::get_bottom_left(x, z);
        let rel_pos = pos - bottom_left;
        let block_idx = rel_pos.x + (rel_pos.z + rel_pos.y * CHUNK_WIDTH_Z) * CHUNK_WIDTH_X;

        if block_idx < 0 || block_idx >= CHUNK_VOLUME as i32 {
            return None;
        }

        Some((x, z, block_idx as u32))
    }

    pub fn get_block_data(&self, pos: &Point3<i32>) -> Option<&BlockData> {
        let (x, z, bid) = Self::get_block_id(pos)?;
        if self.x != x || self.z != z {
            return None;
        }

        let data = &self.block_data[self.blocks[bid as usize] as usize];

        Some(data)
    }

    pub fn should_render(x: i32, z: i32, pos: &Point3<i32>) -> bool {
        let (cx, cz) = Chunk::get_xz_from_vec3(pos);
        (cx - x).abs() <= CHUNK_RENDER_DISTANCE && (cz - z).abs() <= CHUNK_RENDER_DISTANCE
    }

    pub fn should_load(x: i32, z: i32, pos: &Point3<i32>) -> bool {
        let (cx, cz) = Chunk::get_xz_from_vec3(pos);
        (cx - x).abs() <= CHUNK_LOAD_DISTANCE && (cz - z).abs() <= CHUNK_LOAD_DISTANCE
    }
}
