use std::time::Instant;

use crate::{
    config::{CHUNK_OFFSET_Y, CHUNK_VOLUME, CHUNK_WIDTH_X, CHUNK_WIDTH_Y, CHUNK_WIDTH_Z},
    primitives::{BlockData, Chunk},
};

pub struct ChunkGenerator {
    seed: u32, // TODO: procedual generation
}

impl ChunkGenerator {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    pub fn generate_chunk(&self, cx: i32, cz: i32) -> Chunk {
        let block_data = vec![
            BlockData::Air,
            BlockData::Bedrock,
            BlockData::Stone,
            BlockData::Dirt,
            BlockData::Grass,
        ];

        let mut blocks = Vec::with_capacity(CHUNK_VOLUME);
        for y in (CHUNK_OFFSET_Y)..(CHUNK_WIDTH_Y + CHUNK_OFFSET_Y) {
            for z in 0..CHUNK_WIDTH_X {
                for x in 0..CHUNK_WIDTH_Z {
                    let x = x + cx * CHUNK_WIDTH_X;
                    let z = z + cz * CHUNK_WIDTH_Z;
                    // let height: i32 = ((x as f32 * 0.15).sin() * 20.0) as i32
                    //     + ((z as f32 * 0.1).sin() * 5.0) as i32;

                    let height: i32 = 10;
                    let mut block = 0;

                    if y == 0 {
                        block = 1;
                    } else if y < height - 4 {
                        block = 2;
                    } else if y < height - 1 {
                        block = 3;
                    } else if y < height {
                        block = 4;
                    };

                    blocks.push(block);
                }
            }
        }

        Chunk {
            x: cx,
            z: cz,
            block_data,
            blocks,
            loaded_at: Instant::now(), // This belongs in loader.rs
        }
    }
}
