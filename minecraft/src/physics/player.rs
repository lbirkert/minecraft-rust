//! To be honest: AI made most of the collision algos.
//! Didn't want to spend the time researching how to do this.
//! Might rewrite this in the future as a challenge, but we have different problems!

use nalgebra::{Point3, Vector3};

use crate::{
    physics::Aabb,
    primitives::{Player, World, block::Block},
};

#[derive(Default)]
pub struct PlayerPhysics {
    is_on_ground: bool,

    pub is_sprinting: bool,
    pub input: Vector3<f32>,
    pub is_flying: bool, // TODO: forward to player struct
}

impl PlayerPhysics {
    fn swept_block_range(
        min: &Point3<f32>,
        max: &Point3<f32>,
        movement: &Vector3<f32>,
    ) -> (Point3<i32>, Point3<i32>) {
        let end_min = min + movement;
        let end_max = max + movement;

        let swept_min = Point3::new(
            min.x.min(end_min.x),
            min.y.min(end_min.y),
            min.z.min(end_min.z),
        );

        let swept_max = Point3::new(
            max.x.max(end_max.x),
            max.y.max(end_max.y),
            max.z.max(end_max.z),
        );

        (
            Point3::new(
                swept_min.x.floor() as i32,
                swept_min.y.floor() as i32,
                swept_min.z.floor() as i32,
            ),
            Point3::new(
                swept_max.x.floor() as i32,
                swept_max.y.floor() as i32,
                swept_max.z.floor() as i32,
            ),
        )
    }

    fn find_earliest_collision(
        aabb: &Aabb, // replace with your AABB type
        world: &World,
        movement: Vector3<f32>,
    ) -> Option<(f32, Vector3<f32>)> {
        let (min, max) = Self::swept_block_range(&aabb.min, &aabb.max, &movement);

        let mut earliest: Option<(f32, Vector3<f32>)> = None;

        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let block_pos = Point3::new(x, y, z);

                    let Some(block) = world.get_block_data_fast(&block_pos) else {
                        continue;
                    };

                    if block.is_air() {
                        continue;
                    }

                    let block_aabb = Block::get_aabb(&block_pos);

                    if let Some((t, normal)) = aabb.sweep(&block_aabb, movement) {
                        if earliest.is_none_or(|(best_t, _)| t < best_t) {
                            earliest = Some((t, normal));
                        }
                    }
                }
            }
        }

        earliest
    }

    fn move_player(&mut self, player: &mut Player, world: &World, mut remaining: Vector3<f32>) {
        const MAX_COLLISIONS: usize = 5;
        const EPSILON: f32 = 1e-6;

        for _ in 0..MAX_COLLISIONS {
            if remaining.norm_squared() < EPSILON * EPSILON {
                break;
            }

            let aabb = Player::get_aabb(&Point3::from(player.pos));

            match Self::find_earliest_collision(&aabb, world, remaining) {
                Some((t, normal)) => {
                    // Move to the first point of contact.
                    let t = t.clamp(0.0, 1.0);
                    player.pos += remaining * t;

                    // Keep only the unconsumed movement.
                    remaining *= 1.0 - t;

                    // Assuming +Y is up.
                    if normal.y > 0.5 {
                        self.is_on_ground = true;

                        if player.vel.y < 0.0 {
                            player.vel.y = 0.0;
                        }
                    }

                    // Remove movement into the collision surface.
                    let into = remaining.dot(&normal);

                    if into < 0.0 {
                        remaining -= normal * into;
                    }
                }

                None => {
                    player.pos += remaining;
                    break;
                }
            }
        }
    }

    pub fn update(&mut self, player: &mut Player, world: &World, dt: f32) {
        let mut horizontal_accel = if self.is_on_ground { 45.0 } else { 20.0 };

        if self.is_sprinting {
            horizontal_accel *= 1.5;
        }

        // input is normalized horizontal movement in local/world space
        player.vel.x += self.input.x * horizontal_accel * dt;
        player.vel.z += self.input.z * horizontal_accel * dt;

        if !self.is_flying {
            player.vel.y -= 18.0 * dt;
        }

        let hdrag = if self.is_on_ground {
            10.0 // ordinary stone-like ground
        } else {
            6.0
        };
        let vdrag = 0.8;
        let hdrag = (-hdrag * dt).exp();
        let vdrag = (-vdrag * dt).exp();

        player.vel.x *= hdrag;
        player.vel.z *= hdrag;
        // vertical drag
        player.vel.y *= vdrag;

        let vel = player.vel * dt;

        let sim_steps = ((vel.norm() * 2.0).ceil() as usize).max(1);
        let rel_vel = vel / sim_steps as f32;

        self.is_on_ground = false;

        for _ in 0..sim_steps {
            self.move_player(player, world, rel_vel);
        }
    }

    pub fn jump(&mut self, player: &mut Player) {
        if self.is_on_ground {
            player.vel.y = 7.0;
            self.is_on_ground = false;
        }
    }
}
