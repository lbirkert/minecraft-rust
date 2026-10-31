use nalgebra::{Point3, Vector3};

use crate::{physics::Aabb, primitives::Inventory};

#[derive(Default, Eq, PartialEq, Hash, Clone, Copy)]
pub struct PlayerId(u64);

impl From<u64> for PlayerId {
    fn from(val: u64) -> Self {
        PlayerId(val)
    }
}

#[derive(Default)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub pos: Point3<f32>,  // pos of feet center
    pub vel: Vector3<f32>, // velocity of feet center
    pub yaw: f32,
    pub pitch: f32,
    pub inventory: Inventory,
}

impl Player {
    pub const PLAYER_MIN: Vector3<f32> = Vector3::new(-0.3, 0.0, -0.3);
    pub const PLAYER_MAX: Vector3<f32> = Vector3::new(0.3, 1.8, 0.3);
    pub const PLAYER_EYE: Vector3<f32> = Vector3::new(0.0, 1.6, 0.0);

    pub fn get_aabb(pos: &Point3<f32>) -> Aabb {
        Aabb {
            min: pos + Self::PLAYER_MIN,
            max: pos + Self::PLAYER_MAX,
        }
    }

    pub fn get_eye_pos(pos: &Point3<f32>) -> Point3<f32> {
        pos + Self::PLAYER_EYE
    }
}
