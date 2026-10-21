//! The furnaces smelt on the server: each furnace of the loaded chunks ticks once per tick. One
//! in a chunk that is not loaded waits, as in Minecraft.

use bevy::prelude::*;
use shared::world::{BlockData, BlockId, ChunkMap, FurnaceChange};

use crate::world::data::ServerWorldMap;

/// Ticks every furnace of the loaded chunks once.
pub fn tick_furnaces_system(mut world_map: ResMut<ServerWorldMap>) {
    tick_furnaces(&mut world_map.chunks);
}

/// Ticks every furnace of `chunks` once. One that changed is recorded, for the save and for the
/// player who has it open. Its block is a `LitFurnace` while it burns and a `Furnace` otherwise,
/// turned as it was: the clients learn it lit or went out from the block updates.
fn tick_furnaces(chunks: &mut ChunkMap) {
    let positions: Vec<IVec3> = chunks.furnace_positions().collect();
    for pos in positions {
        let Some(mut furnace) = chunks.furnace(pos).cloned() else {
            continue;
        };
        if furnace.tick() != FurnaceChange::Same {
            if let Some(stored) = chunks.furnace_mut(pos) {
                *stored = furnace.clone();
            }
        }
        let id = if furnace.is_lit() {
            BlockId::LitFurnace
        } else {
            BlockId::Furnace
        };
        // Checked on every tick, which also puts out a lit block whose furnace was lost
        if let Some(block) = chunks
            .get_block(pos)
            .copied()
            .filter(|block| block.id != id)
        {
            chunks.set_block(pos, BlockData { id, ..block });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockDirection, Furnace, ItemId, ItemStack};

    const AT: IVec3 = IVec3::new(3, 4, 5);

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    /// A map holding `furnace` at `AT`, in the block `block`, and no change recorded.
    fn holding(block: BlockData, furnace: Furnace) -> ChunkMap {
        let mut chunks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        chunks.set_block(AT, block);
        chunks.insert_furnace(AT, furnace);
        chunks.take_changed_blocks();
        chunks
    }

    #[test]
    fn a_furnace_that_lights_becomes_a_lit_furnace_turned_as_it_was() {
        let block = BlockData {
            breaking_progress: 3,
            ..BlockData::new(BlockId::Furnace, BlockDirection::Right)
        };
        let mut chunks = holding(
            block,
            Furnace {
                input: Some(stack(ItemId::Sand, 2)),
                fuel: Some(stack(ItemId::Coal, 1)),
                ..Default::default()
            },
        );

        tick_furnaces(&mut chunks);

        assert!(chunks.furnace(AT).unwrap().is_lit());
        assert_eq!(
            chunks.get_block(AT),
            Some(&BlockData {
                id: BlockId::LitFurnace,
                ..block
            })
        );
        assert_eq!(chunks.take_changed_furnaces(), vec![AT]);
        assert_eq!(chunks.take_changed_blocks(), vec![AT]);

        // Burning on, it stays lit
        tick_furnaces(&mut chunks);
        assert_eq!(chunks.take_changed_furnaces(), vec![AT]);
        assert!(chunks.take_changed_blocks().is_empty());
    }

    #[test]
    fn a_furnace_that_goes_out_becomes_a_furnace_again() {
        let mut chunks = holding(
            BlockData::new(BlockId::LitFurnace, BlockDirection::Back),
            Furnace {
                burn_left: 1,
                burn_total: 100,
                ..Default::default()
            },
        );

        tick_furnaces(&mut chunks);

        assert_eq!(
            chunks.get_block(AT),
            Some(&BlockData::new(BlockId::Furnace, BlockDirection::Back))
        );
        assert_eq!(chunks.furnace(AT).unwrap().burn_left, 0);
        assert_eq!(chunks.take_changed_furnaces(), vec![AT]);
    }

    #[test]
    fn an_idle_furnace_changes_nothing() {
        let block = BlockData::new(BlockId::Furnace, BlockDirection::Front);
        let idle = Furnace {
            input: Some(stack(ItemId::Dirt, 2)),
            fuel: Some(stack(ItemId::Coal, 1)),
            ..Default::default()
        };
        let mut chunks = holding(block, idle.clone());

        tick_furnaces(&mut chunks);

        assert_eq!(chunks.furnace(AT), Some(&idle));
        assert!(chunks.take_changed_furnaces().is_empty());
        assert!(chunks.take_changed_blocks().is_empty());
    }

    #[test]
    fn a_lit_furnace_that_burns_nothing_is_put_out() {
        // as one whose state was lost with a damaged file
        let mut chunks = holding(
            BlockData::new(BlockId::LitFurnace, BlockDirection::Front),
            Furnace::default(),
        );

        tick_furnaces(&mut chunks);

        assert_eq!(chunks.get_block(AT).unwrap().id, BlockId::Furnace);
        assert_eq!(chunks.furnace(AT), Some(&Furnace::default()));
        assert!(chunks.take_changed_furnaces().is_empty());
    }
}
