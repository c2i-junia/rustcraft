//! What changes by itself around the players: plants grow, the rain fills the cisterns and puts
//! the campfires out.

use bevy::prelude::*;
use rand::Rng;
use shared::{
    world::{world_position_to_chunk_position, BlockData, BlockId, ChunkMap, WeatherKind},
    CHUNK_SIZE,
};
use std::collections::HashSet;

use super::{data::ServerWorldMap, generation::sapling_tree, weather::ServerWeather};

/// How far around the chunk of a player things change, in chunks.
const REACH: i32 = 3;
const REACH_Y: i32 = 2;
/// Cells looked at in each chunk on each tick: each is, every 51 seconds on average.
const CELLS_PER_CHUNK: usize = 4;
/// How far up a cell looks for a roof.
const SKY: i32 = 64;

pub fn growth_system(mut world_map: ResMut<ServerWorldMap>, weather: Res<ServerWeather>) {
    let mut rng = rand::thread_rng();
    let rains = weather.kind != WeatherKind::Clear;
    let mut around = HashSet::new();
    for player in world_map.players.values() {
        let centre = world_position_to_chunk_position(player.position);
        for x in -REACH..=REACH {
            for y in -REACH_Y..=REACH_Y {
                for z in -REACH..=REACH {
                    around.insert(centre + IVec3::new(x, y, z));
                }
            }
        }
    }
    for chunk_pos in around {
        for _ in 0..CELLS_PER_CHUNK {
            let mut cell = || rng.gen_range(0..CHUNK_SIZE);
            let pos = chunk_pos * CHUNK_SIZE + IVec3::new(cell(), cell(), cell());
            change(&mut world_map.chunks, pos, rains, &mut rng);
        }
    }
}

/// Lets the block at `pos` change.
fn change(chunks: &mut ChunkMap, pos: IVec3, rains: bool, rng: &mut impl Rng) {
    let Some(block) = chunks.get_block(pos).copied() else {
        return;
    };
    let rained_on = |chunks: &ChunkMap| {
        rains && (1..=SKY).all(|dy| chunks.get_block(pos + IVec3::Y * dy).is_none())
    };
    let turned = match block.id {
        BlockId::RyeSprout | BlockId::YoungRye => {
            // The rain waters the crops
            let one_in = if rained_on(chunks) { 2 } else { 4 };
            if !rng.gen_ratio(1, one_in) {
                return;
            }
            if block.id == BlockId::RyeSprout {
                BlockId::YoungRye
            } else {
                BlockId::RipeRye
            }
        }
        BlockId::BerryBush if rng.gen_ratio(1, 5) => BlockId::RipeBerryBush,
        BlockId::RainCistern if rained_on(chunks) => BlockId::FullRainCistern,
        BlockId::Campfire if rained_on(chunks) => BlockId::UnlitCampfire,
        BlockId::OakSapling
        | BlockId::SpruceSapling
        | BlockId::JungleSapling
        | BlockId::WillowSapling
        | BlockId::AcaciaSapling
            if rng.gen_ratio(1, 6) =>
        {
            return grow_tree(chunks, pos, block.id, rng.gen());
        }
        _ => return,
    };
    chunks.set_block(pos, BlockData::new(turned, block.direction));
}

/// Grows the tree of the sapling at `pos`, unless its trunk has no room. It takes no block but
/// the sapling.
fn grow_tree(chunks: &mut ChunkMap, pos: IVec3, sapling: BlockId, seed: u32) {
    let tree = sapling_tree(sapling, pos, seed);
    let is_log = |id: BlockId| id.is_mined_faster_by(shared::world::ToolKind::Axe);
    let blocked = tree
        .iter()
        .any(|(at, id, _)| is_log(*id) && *at != pos && chunks.get_block(*at).is_some());
    if blocked {
        return;
    }
    chunks.remove_block(pos);
    for (at, id, direction) in tree {
        if chunks.get_block(at).is_none() {
            chunks.set_block(at, BlockData::new(id, direction));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::BlockDirection;

    const AT: IVec3 = IVec3::new(4, 4, 4);

    /// What `id` becomes after many chances to change.
    fn becomes(id: BlockId, rains: bool) -> ChunkMap {
        let mut chunks = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        chunks.set_block(AT, BlockData::new(id, BlockDirection::Front));
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..200 {
            change(&mut chunks, AT, rains, &mut rng);
        }
        chunks
    }

    #[test]
    fn plants_ripen_and_the_rain_fills_cisterns_and_puts_campfires_out() {
        for (id, rains, after) in [
            (BlockId::RyeSprout, false, BlockId::RipeRye),
            (BlockId::BerryBush, false, BlockId::RipeBerryBush),
            (BlockId::RainCistern, false, BlockId::RainCistern),
            (BlockId::RainCistern, true, BlockId::FullRainCistern),
            (BlockId::Campfire, false, BlockId::Campfire),
            (BlockId::Campfire, true, BlockId::UnlitCampfire),
            (BlockId::AcaciaSapling, false, BlockId::AcaciaLog),
            (BlockId::SpruceSapling, false, BlockId::SpruceLog),
            (BlockId::JungleSapling, false, BlockId::JungleLog),
        ] {
            let chunks = becomes(id, rains);
            assert_eq!(chunks.get_block(AT).map(|b| b.id), Some(after), "{id:?}");
        }
    }
}
