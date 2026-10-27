use nalgebra::Vector3;

use crate::primitives::{Chunk, Player, World};

// cell based raycast  TODO: return hit position + normal
pub fn get_selected_block(player: &Player, world: &World) -> Option<(u64, u32)> {
    let (sy, cy) = player.yaw.sin_cos();
    let (sp, cp) = player.pitch.sin_cos();
    let direction = Vector3::new(sy * cp, sp, -cy * cp).normalize();

    let player_pos = Player::get_eye_pos(&player.pos);
    let mut pos = player_pos;
    let mut cell = pos.clone().map(|f| f.floor());
    let step = direction.map(|f| if f > 0.0 { 1.0 } else { -1.0 });
    let block_edge_step = direction.map(|f| if f > 0.0 { 1.0 } else { 0.0 });

    loop {
        let block_pos = cell.map(|f| f as i32);

        if let Some((x, z, bid)) = Chunk::get_block_id(&block_pos) {
            let cid = Chunk::get_id(x, z);
            let block = world.get_block_data_fast(&block_pos);
            if block.is_some_and(|d| !d.is_air()) {
                // collision!!
                return Some((cid, bid));
            }
        }

        let block_edge = cell + block_edge_step;

        // "time of hitting edge"
        let tohe = (block_edge - pos).map_with_location(|r, _, f| {
            if direction[r].abs() < 1e-9 {
                f32::INFINITY
            } else {
                f / direction[r]
            }
        });
        let min_tohe = tohe.min();
        let cell_update = tohe.map(|v| if v == min_tohe { 1.0 } else { 0.0 });

        pos += direction * min_tohe;
        cell += cell_update.component_mul(&step);

        if (pos - player_pos).norm_squared() > 16.0 {
            return None;
        }
    }
}
