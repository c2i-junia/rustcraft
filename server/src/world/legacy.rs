//! The `world.ron` format written before chunks became dense arrays: a chunk was a map from
//! local position to block, next to bookkeeping that is no longer saved. Only the conversion
//! to the current format lives here.

use std::collections::HashMap;

use bevy::{
    log::{debug, warn},
    math::IVec3,
};
use serde::Deserialize;
use shared::{
    world::{
        global_block_to_chunk_pos, to_local_pos, BlockData, Chunk, MobId, ServerMob, WorldSeed,
    },
    CHUNK_SIZE,
};

use super::save::WorldData;

#[derive(Deserialize)]
pub(super) struct LegacyWorldData {
    map: HashMap<IVec3, LegacyChunk>,
    mobs: HashMap<MobId, ServerMob>,
    seed: WorldSeed,
    name: String,
    time: u64,
}

/// The `ts` and `sent_to_clients` of a saved chunk are ignored.
#[derive(Deserialize)]
struct LegacyChunk {
    map: HashMap<IVec3, BlockData>,
}

impl From<LegacyWorldData> for WorldData {
    /// The generator used to write blocks of a structure outside the local range of their chunk
    /// (a leaf at local x = 16 or y = 17). Such a block goes to the chunk its global position
    /// belongs to, if that chunk is in the file and the cell is still air, and is dropped
    /// otherwise. Blocks within range always win. Among the others, the one of the chunk and
    /// local position that sort first wins.
    fn from(legacy: LegacyWorldData) -> Self {
        let mut chunks: HashMap<IVec3, Chunk> = legacy
            .map
            .keys()
            .map(|&chunk_pos| (chunk_pos, Chunk::new()))
            .collect();
        let mut out_of_range: Vec<(IVec3, IVec3, BlockData)> = Vec::new();

        for (chunk_pos, legacy_chunk) in &legacy.map {
            for (&local, &block) in &legacy_chunk.map {
                if Chunk::contains(local) {
                    if let Some(chunk) = chunks.get_mut(chunk_pos) {
                        chunk.set(local, block);
                    }
                } else {
                    out_of_range.push((*chunk_pos, local, block));
                }
            }
        }

        out_of_range.sort_by_key(|(chunk_pos, local, _)| {
            (
                (chunk_pos.x, chunk_pos.y, chunk_pos.z),
                (local.x, local.y, local.z),
            )
        });
        let (mut moved, mut dropped) = (0, 0);
        for (chunk_pos, local, block) in out_of_range {
            // The file is not trusted: a position that does not fit in an i32 is dropped
            let global = chunk_pos
                .checked_mul(IVec3::splat(CHUNK_SIZE))
                .and_then(|origin| origin.checked_add(local));
            let target = global.and_then(|global| {
                let chunk = chunks.get_mut(&global_block_to_chunk_pos(&global))?;
                Some((chunk, to_local_pos(&global)))
            });
            match target {
                Some((chunk, cell)) if chunk.get(cell).is_none() => {
                    chunk.set(cell, block);
                    moved += 1;
                }
                _ => {
                    debug!(
                        "Dropped the block {:?} saved out of its chunk {chunk_pos} at local {local}",
                        block.id
                    );
                    dropped += 1;
                }
            }
        }
        if moved + dropped > 0 {
            warn!(
                "Old world: {moved} blocks saved outside of their chunk were moved to the chunk they belong to, {dropped} were dropped"
            );
        }

        WorldData {
            map: chunks,
            mobs: legacy.mobs,
            seed: legacy.seed,
            name: legacy.name,
            time: legacy.time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::BlockId;

    /// Written by the code that preceded dense chunks, for a world made of two chunks (0,0,0)
    /// and (1,0,0). Chunk (0,0,0) holds stone at local (1,2,3), leaves at local (16,2,3) and
    /// (0,17,0), that is outside of it.
    const OLD_WORLD: &str = r#"(
    map: {
        (
            0,
            0,
            0,
        ): (
            map: {(1,2,3):(id:Stone,direction:Front,breaking_progress:0),(0,17,0):(id:OakLeaves,direction:Front,breaking_progress:0),(16,2,3):(id:OakLeaves,direction:Back,breaking_progress:0)},
            ts: 7,
            sent_to_clients: [5],
        ),
        (
            1,
            0,
            0,
        ): (
            map: {},
            ts: 0,
            sent_to_clients: [],
        ),
    },
    mobs: {},
    seed: (42),
    name: "old",
    time: 9,
)"#;

    fn convert(text: &str) -> WorldData {
        ron::de::from_str::<LegacyWorldData>(text).unwrap().into()
    }

    #[test]
    fn a_world_in_the_old_format_converts() {
        let world = convert(OLD_WORLD);

        assert_eq!(world.name, "old");
        assert_eq!(world.seed.0, 42);
        assert_eq!(world.time, 9);
        assert_eq!(world.map.len(), 2);
        let first = &world.map[&IVec3::ZERO];
        assert_eq!(
            first.get(IVec3::new(1, 2, 3)).map(|b| b.id),
            Some(BlockId::Stone)
        );
        // the leaf at local x = 16 belongs to the next chunk, which is in the file
        assert_eq!(first.iter().count(), 1);
        let second = &world.map[&IVec3::X];
        assert_eq!(
            second.get(IVec3::new(0, 2, 3)).map(|b| (b.id, b.direction)),
            Some((BlockId::OakLeaves, shared::world::BlockDirection::Back))
        );
        assert_eq!(second.iter().count(), 1);
        // the leaf at local y = 17 belongs to the chunk (0,1,0), which is not: it is dropped
    }

    #[test]
    fn a_block_out_of_range_never_replaces_another_block() {
        let world = convert(
            r#"(map: {
                (0,0,0): (map: {(16,2,3):(id:OakLeaves,direction:Front,breaking_progress:0)}, ts: 0, sent_to_clients: []),
                (1,0,0): (map: {(0,2,3):(id:Stone,direction:Front,breaking_progress:2)}, ts: 0, sent_to_clients: []),
            }, mobs: {}, seed: (1), name: "w", time: 0)"#,
        );

        let cell = world.map[&IVec3::X].get(IVec3::new(0, 2, 3)).unwrap();
        assert_eq!((cell.id, cell.breaking_progress), (BlockId::Stone, 2));
    }

    #[test]
    fn a_block_out_of_range_on_the_negative_side_goes_to_the_previous_chunk() {
        let world = convert(
            r#"(map: {
                (0,0,0): (map: {}, ts: 0, sent_to_clients: []),
                (1,0,0): (map: {(-1,2,3):(id:Sand,direction:Front,breaking_progress:0)}, ts: 0, sent_to_clients: []),
            }, mobs: {}, seed: (1), name: "w", time: 0)"#,
        );

        assert_eq!(
            world.map[&IVec3::ZERO]
                .get(IVec3::new(15, 2, 3))
                .map(|b| b.id),
            Some(BlockId::Sand)
        );
        assert!(world.map[&IVec3::X].is_empty());
    }

    #[test]
    fn the_first_of_two_out_of_range_blocks_for_a_cell_wins() {
        let text = r#"(map: {
                (0,0,0): (map: {(16,2,3):(id:Sand,direction:Front,breaking_progress:0)}, ts: 0, sent_to_clients: []),
                (2,0,0): (map: {(-16,2,3):(id:Glass,direction:Front,breaking_progress:0)}, ts: 0, sent_to_clients: []),
                (1,0,0): (map: {}, ts: 0, sent_to_clients: []),
            }, mobs: {}, seed: (1), name: "w", time: 0)"#;
        for _ in 0..20 {
            let world = convert(text);
            assert_eq!(
                world.map[&IVec3::X].get(IVec3::new(0, 2, 3)).map(|b| b.id),
                Some(BlockId::Sand)
            );
        }
    }

    #[test]
    fn positions_that_overflow_are_dropped_and_the_rest_is_kept() {
        let world = convert(
            r#"(map: {
                (0,0,0): (map: {(1,2,3):(id:Stone,direction:Front,breaking_progress:0)}, ts: 0, sent_to_clients: []),
                (2147483647,0,0): (map: {(16,2,3):(id:Sand,direction:Front,breaking_progress:0),(5,5,5):(id:Dirt,direction:Front,breaking_progress:0)}, ts: 0, sent_to_clients: []),
                (1,0,0): (map: {(2147483647,0,0):(id:Glass,direction:Front,breaking_progress:0),(-2147483648,0,0):(id:Glass,direction:Front,breaking_progress:0)}, ts: 0, sent_to_clients: []),
            }, mobs: {}, seed: (1), name: "w", time: 0)"#,
        );

        assert_eq!(world.map.len(), 3);
        assert_eq!(world.map[&IVec3::ZERO].iter().count(), 1);
        assert_eq!(world.map[&IVec3::new(i32::MAX, 0, 0)].iter().count(), 1);
        assert!(world.map[&IVec3::X].is_empty());
    }
}
