use bevy::prelude::*;
use shared::world::{ServerWorldMap, WorldSeed};

use crate::world::generation::generate_chunk;

use super::broadcast_world::{get_all_active_chunks, BROADCAST_RENDER_DISTANCE};

pub fn background_world_generation_system(
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
) {
    let all_chunks = get_all_active_chunks(&world_map.players, BROADCAST_RENDER_DISTANCE);
    let mut generated = 0;
    for c in all_chunks {
        let chunk = world_map.chunks.map.get(&c);

        if chunk.is_none() {
            let chunk = generate_chunk(c, seed.0);
            info!("Generated chunk: {:?}", c);
            world_map.chunks.map.insert(c, chunk);
            generated += 1;
        }

        if generated >= 1 {
            break;
        }
    }
}
