use nalgebra::{Point3, Vector3};

use crate::physics::Aabb;

#[derive(Clone, Eq, PartialEq)]
pub enum BlockData {
    Air,
    Grass,
    Dirt,
    Stone,
    Bedrock,
    AcaciaStairs,
    AcaciaBark,
    AcaciaLeaves,
    AcaciaLog,
    AcaciaPlanks,
    AcaciaDoor,
    AcaciaFence,
}

impl BlockData {
    pub fn get_model(&self) -> Option<&'static str> {
        match self {
            BlockData::Air => None,
            BlockData::Grass => Some("block/grass_normal"),
            BlockData::Dirt => Some("block/dirt"),
            BlockData::Stone => Some("block/stone"),
            BlockData::Bedrock => Some("block/bedrock"),
            BlockData::AcaciaStairs => Some("block/acacia_stairs"),
            BlockData::AcaciaBark => Some("block/acacia_bark"),
            BlockData::AcaciaLeaves => Some("block/acacia_leaves"),
            BlockData::AcaciaLog => Some("block/acacia_log"),
            BlockData::AcaciaPlanks => Some("block/acacia_planks"),
            BlockData::AcaciaDoor => Some("block/acacia_door_bottom"),
            BlockData::AcaciaFence => Some("block/acacia_fence_post"), // TODO: variants
        }
    }

    pub fn is_air(&self) -> bool {
        matches!(self, BlockData::Air)
    }

    pub fn is_unbreakable(&self) -> bool {
        matches!(self, BlockData::Bedrock)
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
