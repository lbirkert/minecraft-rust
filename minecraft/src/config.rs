pub const CHUNK_WIDTH_X: i32 = 16;
pub const CHUNK_WIDTH_Z: i32 = 16;
pub const CHUNK_WIDTH_Y: i32 = 512;
// pub const CHUNK_OFFSET_Y: i32 = -128;
pub const CHUNK_OFFSET_Y: i32 = 0;
pub const CHUNK_VOLUME: usize = (CHUNK_WIDTH_X * CHUNK_WIDTH_Y * CHUNK_WIDTH_Z) as usize;
pub const CHUNK_LOAD_DISTANCE: i32 = 8;
pub const CHUNK_RENDER_DISTANCE: i32 = 4;

pub const CHUNK_RENDER_AFTER: u128 = 10 * 1000; // time to load mesh after no neighbor is found.

//pub const CHUNK_SIM_DISTANCE: i32 = 8;
