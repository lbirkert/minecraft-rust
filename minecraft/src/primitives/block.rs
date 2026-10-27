use nalgebra::{Point3, Vector3};

use crate::physics::Aabb;

#[derive(Clone)]
pub enum BlockKind {
    Air,
    Grass,
    Dirt,
    Stone,
}

impl BlockKind {
    pub fn get_model(&self) -> Option<&'static str> {
        match self {
            BlockKind::Air => None,
            BlockKind::Grass => Some("block/grass_normal"),
            BlockKind::Dirt => Some("block/dirt"),
            BlockKind::Stone => Some("block/stone"),
        }
    }
}

#[derive(Clone)]
pub struct BlockData {
    pub kind: BlockKind,
    // TODO: add nbt here
}

impl BlockData {
    pub fn is_air(&self) -> bool {
        matches!(self.kind, BlockKind::Air)
    }
}

// this is a namespace right now, TODO: actually impl a block structure
pub struct Block {}

impl Block {
    pub const BLOCK_MIN: Vector3<f32> = Vector3::new(0.0, 0.0, 0.0);
    pub const BLOCK_MAX: Vector3<f32> = Vector3::new(1.0, 1.0, 1.0);

    // TODO: use model to construct Aabb's
    pub fn get_aabb(pos: &Point3<i32>) -> Aabb {
        let pos = Point3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        Aabb {
            min: pos + Self::BLOCK_MIN,
            max: pos + Self::BLOCK_MAX,
        }
    }
}
