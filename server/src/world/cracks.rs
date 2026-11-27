//! The cracks of a block mend once the players stop hitting it: after a short wait, its
//! breaking progress falls back to nothing, a stage at a time. The server alone mends them; the
//! clients see the cracks recede through the block updates.

use std::collections::HashMap;

use bevy::prelude::*;
use shared::{world::ChunkMap, TICKS_PER_SECOND};

use super::data::ServerWorldMap;

/// The ticks a block keeps its cracks after its last hit, before they start to mend: half a
/// second, longer than the gaps between the hits of a held button.
const MEND_DELAY_TICKS: u32 = TICKS_PER_SECOND as u32 / 2;
/// The ticks the cracks of a block about to break take to mend entirely, whatever the block.
const MEND_TICKS: u32 = 2 * TICKS_PER_SECOND as u32;

/// The cracked blocks the server knows, with the progress each had on the last tick and the
/// ticks since it was last hit.
///
/// Not saved: a block cracked in a saved chunk keeps its cracks until it is hit again.
#[derive(Resource, Default)]
pub struct Cracks {
    cracked: HashMap<IVec3, Crack>,
}

#[derive(Clone, Copy)]
struct Crack {
    progress: u8,
    idle_ticks: u32,
}

/// Mends the cracks of the blocks the players stopped hitting.
pub fn mend_cracks_system(mut cracks: ResMut<Cracks>, mut world_map: ResMut<ServerWorldMap>) {
    cracks.tick(&mut world_map.chunks);
}

impl Cracks {
    /// Learns the blocks cracked since the last tick from the change log of `chunks`, which
    /// the broadcast drains after this, then mends those left alone long enough.
    fn tick(&mut self, chunks: &mut ChunkMap) {
        for pos in chunks.changed_blocks() {
            let Some(block) = chunks.get_block(pos) else {
                continue;
            };
            if block.breaking_progress > 0 {
                self.cracked.entry(pos).or_insert(Crack {
                    progress: block.breaking_progress,
                    idle_ticks: 0,
                });
            }
        }

        self.cracked.retain(|&pos, crack| {
            // A block gone, replaced, unloaded or broken has nothing left to mend
            let Some(mut block) = chunks.get_block(pos).copied() else {
                return false;
            };
            let Some(break_time) = block.id.get_break_time() else {
                return false;
            };
            if block.breaking_progress == 0 {
                return false;
            }
            if block.breaking_progress > crack.progress {
                crack.idle_ticks = 0;
            }
            crack.idle_ticks += 1;
            if crack.idle_ticks > MEND_DELAY_TICKS {
                let step = (break_time as u32).div_ceil(MEND_TICKS) as u8;
                block.breaking_progress = block.breaking_progress.saturating_sub(step);
                chunks.set_block(pos, block);
            }
            crack.progress = block.breaking_progress;
            block.breaking_progress > 0
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockData, BlockDirection, BlockId};

    const AT: IVec3 = IVec3::new(3, 4, 5);

    /// A map holding a stone at `AT` cracked by `progress`, its change recorded as a hit's.
    fn cracked(progress: u8) -> ChunkMap {
        let mut chunks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        chunks.set_block(
            AT,
            BlockData {
                breaking_progress: progress,
                ..BlockData::new(BlockId::Stone, BlockDirection::Front)
            },
        );
        chunks
    }

    fn progress(chunks: &ChunkMap) -> u8 {
        chunks.get_block(AT).unwrap().breaking_progress
    }

    /// Ticks `cracks` once, the broadcast draining the change log after it as on the server.
    fn tick(cracks: &mut Cracks, chunks: &mut ChunkMap) -> Vec<IVec3> {
        cracks.tick(chunks);
        chunks.take_changed_blocks()
    }

    #[test]
    fn the_cracks_wait_then_mend_a_little_each_tick_until_none_are_left() {
        let break_time = BlockId::Stone.get_break_time().unwrap();
        let mut chunks = cracked(break_time - 1);
        let mut cracks = Cracks::default();

        for _ in 0..MEND_DELAY_TICKS {
            tick(&mut cracks, &mut chunks);
            assert_eq!(progress(&chunks), break_time - 1, "still waiting");
        }

        let mut seen = vec![progress(&chunks)];
        let mut ticks = 0;
        while progress(&chunks) > 0 {
            assert_eq!(
                tick(&mut cracks, &mut chunks),
                vec![AT],
                "sent to the clients"
            );
            seen.push(progress(&chunks));
            ticks += 1;
        }
        assert!(seen.windows(2).all(|w| w[1] < w[0]), "{seen:?}");
        assert!(ticks <= MEND_TICKS, "{ticks} ticks");
        assert!(ticks > 1, "gradual, not at once");
        assert!(cracks.cracked.is_empty());
        assert!(
            tick(&mut cracks, &mut chunks).is_empty(),
            "nothing more to send"
        );
    }

    #[test]
    fn a_hit_puts_off_the_mending() {
        let mut chunks = cracked(4);
        let mut cracks = Cracks::default();
        for _ in 0..MEND_DELAY_TICKS {
            tick(&mut cracks, &mut chunks);
        }

        // Hit again just before the cracks would mend
        let mut block = *chunks.get_block(AT).unwrap();
        block.breaking_progress = 6;
        chunks.set_block(AT, block);
        for _ in 0..MEND_DELAY_TICKS {
            tick(&mut cracks, &mut chunks);
            assert_eq!(progress(&chunks), 6);
        }
        tick(&mut cracks, &mut chunks);
        assert!(progress(&chunks) < 6);
    }

    #[test]
    fn a_block_broken_or_replaced_is_forgotten() {
        let mut chunks = cracked(4);
        let mut cracks = Cracks::default();
        tick(&mut cracks, &mut chunks);

        chunks.remove_block(AT);
        tick(&mut cracks, &mut chunks);
        assert!(cracks.cracked.is_empty());

        chunks.set_block(AT, BlockData::new(BlockId::Dirt, BlockDirection::Front));
        tick(&mut cracks, &mut chunks);
        assert!(cracks.cracked.is_empty());
    }

    #[test]
    fn a_block_that_was_never_hit_is_left_alone() {
        let mut chunks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        chunks.set_block(AT, BlockData::new(BlockId::Stone, BlockDirection::Front));
        let mut cracks = Cracks::default();
        tick(&mut cracks, &mut chunks);
        assert!(cracks.cracked.is_empty());
    }
}
