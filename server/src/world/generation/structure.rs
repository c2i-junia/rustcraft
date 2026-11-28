//! What the structures of the world, the villages and the dungeons, share: the blocks of a plan
//! cut into the chunks they fall in, what their containers hold, and the turning of a piece.

use std::collections::HashMap;

use bevy::prelude::*;
use shared::world::{BlockData, BlockDirection, Chunk, Container, ItemStack, CHEST_SLOTS};
use shared::CHUNK_SIZE;

use super::PositionRandom;

/// The blocks of a plan and what its containers hold, read by chunk.
pub struct StructureBlocks {
    /// The blocks by column of chunks, in the order they are placed: a later one replaces an
    /// earlier one. `None` is air.
    blocks: HashMap<IVec2, Vec<(IVec3, Option<BlockData>)>>,
    containers: Vec<(IVec3, Container)>,
}

impl StructureBlocks {
    pub fn new(
        blocks: Vec<(IVec3, Option<BlockData>)>,
        containers: Vec<(IVec3, Container)>,
    ) -> Self {
        let mut by_column: HashMap<IVec2, Vec<_>> = HashMap::new();
        for (pos, block) in blocks {
            let chunk_column = pos.xz().div_euclid(IVec2::splat(CHUNK_SIZE));
            by_column
                .entry(chunk_column)
                .or_default()
                .push((pos, block));
        }
        Self {
            blocks: by_column,
            containers,
        }
    }

    /// Places the blocks that fall in the chunk at `chunk_pos`: they replace whatever was
    /// generated there.
    pub fn place(&self, chunk: &mut Chunk, chunk_pos: IVec3) {
        let Some(blocks) = self.blocks.get(&chunk_pos.xz()) else {
            return;
        };
        let origin = chunk_pos * CHUNK_SIZE;
        for &(pos, block) in blocks {
            let local = pos - origin;
            if !(0..CHUNK_SIZE).contains(&local.y) {
                continue;
            }
            match block {
                Some(block) => chunk.set(local, block),
                None => {
                    chunk.remove(local);
                }
            }
        }
    }

    /// What the containers in the chunk at `chunk_pos` hold, by their position in the chunk.
    pub fn chunk_containers(
        &self,
        chunk_pos: IVec3,
    ) -> impl Iterator<Item = (IVec3, Container)> + '_ {
        let origin = chunk_pos * CHUNK_SIZE;
        self.containers.iter().filter_map(move |(pos, container)| {
            let local = *pos - origin;
            Chunk::contains(local).then(|| (local, container.clone()))
        })
    }

    /// Every block, in the order placed.
    #[cfg(test)]
    pub fn all(&self) -> impl Iterator<Item = (IVec3, Option<BlockData>)> + '_ {
        self.blocks.values().flatten().copied()
    }
}

pub use shared::world::structure_loot::LootEntry;

/// Each draw of a container uses its own salt from here.
const SALT_LOOT: u64 = 0x2000;

/// What the container at `pos` holds of `table` in a world of `seed`: a few stacks, in slots
/// drawn from its position, so that it holds the same whichever chunk is generated first.
pub fn fill_container(seed: u32, pos: IVec3, table: &[LootEntry]) -> Container {
    let random = PositionRandom {
        seed,
        coords: [pos.x, pos.y, pos.z],
    };
    let mut container = Container::default();
    for (i, &(item, chance, fewest, most)) in table.iter().enumerate() {
        let salt = SALT_LOOT + 4 * i as u64;
        if random.unit(salt) >= chance {
            continue;
        }
        let nb = fewest + random.below(salt + 1, (most - fewest + 1) as u64) as u32;
        // the first free slot from the one drawn
        let start = random.below(salt + 2, CHEST_SLOTS as u64) as usize;
        let slots = CHEST_SLOTS as usize;
        if let Some(slot) = (0..slots)
            .map(|k| (start + k) % slots)
            .find(|&slot| container.slots[slot].is_none())
        {
            container.slots[slot] = Some(ItemStack {
                item_id: item,
                item_type: item.get_default_type(),
                nb,
            });
        }
    }
    container
}

/// The four directions on the ground, in the order of `BlockDirection`: front (-Z), right (+X),
/// back (+Z), left (-X).
pub const DIRECTIONS: [IVec2; 4] = [IVec2::NEG_Y, IVec2::X, IVec2::Y, IVec2::NEG_X];
pub const BLOCK_DIRECTIONS: [BlockDirection; 4] = [
    BlockDirection::Front,
    BlockDirection::Right,
    BlockDirection::Back,
    BlockDirection::Left,
];

/// Turns `v` by `turns` quarter turns, front toward right.
pub fn rotate(v: IVec2, turns: usize) -> IVec2 {
    (0..turns % 4).fold(v, |v, _| IVec2::new(-v.y, v.x))
}

/// Turns a direction by `turns` quarter turns, as `rotate` turns its step.
pub fn rotate_direction(direction: BlockDirection, turns: usize) -> BlockDirection {
    let index = BLOCK_DIRECTIONS
        .iter()
        .position(|d| *d == direction)
        .unwrap();
    BLOCK_DIRECTIONS[(index + turns) % 4]
}
