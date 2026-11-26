//! The anvil: works the items of its two slots (`world::forge`) for the levels of a player. Only
//! the server does it; a client sees it in the next `PlayerUpdate`.

use super::inventory::{InventoryRefusal, ANVIL_LEFT_SLOT, ANVIL_RIGHT_SLOT};
use super::Player;
use crate::world::{forge, ForgeRefusal, Forging, ItemName};

impl Player {
    /// What the anvil would do with its slots and `name`, if the player can pay for it: free in
    /// creative.
    pub fn forging(&self, name: ItemName) -> Result<Forging, InventoryRefusal> {
        let slots = &self.inventory.inner;
        let left = slots
            .get(&ANVIL_LEFT_SLOT)
            .ok_or(InventoryRefusal::CannotForge(ForgeRefusal::NothingToWork))?;
        let right = slots.get(&ANVIL_RIGHT_SLOT);
        let mut forging = forge(left, right, name).map_err(InventoryRefusal::CannotForge)?;
        if self.game_mode.is_creative() {
            forging.cost = 0;
        } else if !self.has_levels(forging.cost) {
            return Err(InventoryRefusal::NotEnoughLevels(forging.cost));
        }
        Ok(forging)
    }

    /// Works the items of the anvil for its cost: the first slot takes the result, the second
    /// gives what the work used. The caller checks that the anvil is in reach.
    pub(super) fn forge(&mut self, name: ItemName) -> Result<(), InventoryRefusal> {
        let forging = self.forging(name)?;
        self.inventory.inner.insert(ANVIL_LEFT_SLOT, forging.result);
        self.inventory
            .remove_item_from_stack(ANVIL_RIGHT_SLOT, forging.used);
        self.spend_levels(forging.cost);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::PlayerFrameInput;
    use crate::players::blocks::CallerType;
    use crate::players::experience::Experience;
    use crate::players::inventory::{simulate_inventory_actions, InventoryAction};
    use crate::world::{BlockId, ChunkMap, ItemId, ItemStack, ItemType};
    use bevy::math::{IVec3, Vec3};
    use bevy::prelude::Transform;

    const ANVIL: IVec3 = IVec3::new(1, 1, 1);

    #[test]
    fn the_anvil_repairs_for_levels_and_gives_back_what_it_holds_at_the_close() {
        use CallerType::Server;
        use InventoryAction::{Close, Forge, OpenAnvil, QuickMoveSlot};
        let mut world = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        world.fill_box(ANVIL, ANVIL, BlockId::Anvil);
        let position = Vec3::new(1.5, 1.5, 3.5);
        let mut player = Player::new(1, "smith".into(), position, Transform::default());
        player.experience = Experience {
            level: 5,
            points: 0,
        };
        let pickaxe = ItemStack {
            item_id: ItemId::IronPickaxe,
            item_type: ItemType::Tool { wear: 100 },
            nb: 1,
        };
        let ingots = ItemStack {
            item_id: ItemId::IronIngot,
            item_type: ItemType::Generic,
            nb: 5,
        };
        player.inventory.inner.insert(0, pickaxe);
        player.inventory.inner.insert(1, ingots);
        let mut act = |player: &mut Player, action| {
            let input = PlayerFrameInput {
                inventory_actions: vec![action],
                ..Default::default()
            };
            simulate_inventory_actions(player, &mut world, &input, Server)[0].clone()
        };
        let name = ItemName::new("Pioche");
        assert_eq!(
            act(&mut player, Forge(name)).result,
            Err(InventoryRefusal::NoAnvil)
        );
        assert_eq!(act(&mut player, OpenAnvil(ANVIL)).result, Ok(()));
        // The shift-clicks put the pickaxe in the first slot, the ingots in the second
        assert_eq!(act(&mut player, QuickMoveSlot(0)).result, Ok(()));
        assert_eq!(act(&mut player, QuickMoveSlot(1)).result, Ok(()));
        assert_eq!(player.inventory.inner[&ANVIL_RIGHT_SLOT].nb, 5);
        // Two ingots and the name: 3 levels
        assert_eq!(act(&mut player, Forge(name)).result, Ok(()));
        assert_eq!(player.experience.level, 2);
        let repaired = player.inventory.inner[&ANVIL_LEFT_SLOT];
        assert_eq!(repaired.item_type.wear(), Some(0));
        assert_eq!(repaired.custom_name(), Some("Pioche"));
        assert_eq!(player.inventory.inner[&ANVIL_RIGHT_SLOT].nb, 3);
        // Whole now: nothing more to do
        assert_eq!(
            act(&mut player, Forge(name)).result,
            Err(InventoryRefusal::CannotForge(ForgeRefusal::Unchanged))
        );
        // The inventory full, what the anvil holds falls at the feet of the player
        for slot in 0..crate::MAX_INVENTORY_SLOTS {
            player.inventory.inner.insert(
                slot,
                ItemStack {
                    item_id: ItemId::Dirt,
                    item_type: ItemType::Block(BlockId::Dirt),
                    nb: 64,
                },
            );
        }
        let closed = act(&mut player, Close);
        assert_eq!(closed.dropped.len(), 2);
        assert_eq!(closed.dropped[0], repaired);
        assert!(!player.inventory.inner.contains_key(&ANVIL_LEFT_SLOT));
        assert_eq!(player.inventory.open_anvil, None);
    }
}
