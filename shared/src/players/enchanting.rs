//! The engraving table: spends the levels of a player on the enchantments of the item in its
//! slot. Only the server does it; a client sees it in the next `PlayerUpdate`.

use super::inventory::{InventoryRefusal, ENGRAVING_SLOT};
use super::Player;
use crate::world::{enchantment_cost, Enchantment};

impl Player {
    /// What raising `enchantment` by a level on the item of the engraving slot costs, if the
    /// player can: free in creative.
    pub fn enchanting_cost(&self, enchantment: Enchantment) -> Result<u32, InventoryRefusal> {
        let stack = self.inventory.inner.get(&ENGRAVING_SLOT);
        let stack = stack.ok_or(InventoryRefusal::NothingToEnchant)?;
        if stack.enchanted(enchantment).is_none() {
            return Err(InventoryRefusal::NothingToEnchant);
        }
        if self.game_mode.is_creative() {
            return Ok(0);
        }
        let cost = enchantment_cost(stack.enchantment_level(enchantment) + 1);
        if self.has_levels(cost) {
            Ok(cost)
        } else {
            Err(InventoryRefusal::NotEnoughLevels(cost))
        }
    }

    /// Raises `enchantment` by a level on the item of the engraving slot, for its cost. The
    /// caller checks that the table is in reach.
    pub(super) fn enchant(&mut self, enchantment: Enchantment) -> Result<(), InventoryRefusal> {
        let cost = self.enchanting_cost(enchantment)?;
        let stack = self.inventory.inner.get_mut(&ENGRAVING_SLOT);
        let enchanted = stack.and_then(|stack| {
            *stack = stack.enchanted(enchantment)?;
            Some(())
        });
        enchanted.ok_or(InventoryRefusal::NothingToEnchant)?;
        self.spend_levels(cost);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::PlayerFrameInput;
    use crate::players::armour::ArmourPiece;
    use crate::players::blocks::CallerType;
    use crate::players::experience::Experience;
    use crate::players::game_mode::{DamageSource, GameMode};
    use crate::players::inventory::{simulate_inventory_actions, InventoryAction};
    use crate::world::{BlockId, ChunkMap, ItemId, ItemStack, MobKind};
    use bevy::math::{IVec3, Vec3};
    use bevy::prelude::Transform;

    const TABLE: IVec3 = IVec3::new(1, 1, 1);

    /// A player of `level` by a table, with `item` in its first slot.
    fn engraver(item_id: ItemId, level: u32) -> (Player, ChunkMap) {
        let mut world = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        world.fill_box(TABLE, TABLE, BlockId::EngravingTable);
        let position = Vec3::new(1.5, 1.5, 3.5);
        let mut player = Player::new(1, "tester".into(), position, Transform::default());
        player.experience = Experience { level, points: 0 };
        let item_type = item_id.get_default_type();
        let stack = ItemStack {
            item_id,
            item_type,
            nb: 1,
        };
        player.inventory.inner.insert(0, stack);
        (player, world)
    }

    fn act(
        player: &mut Player,
        world: &mut ChunkMap,
        caller: CallerType,
        action: InventoryAction,
    ) -> Result<(), InventoryRefusal> {
        let input = PlayerFrameInput {
            inventory_actions: vec![action],
            ..Default::default()
        };
        simulate_inventory_actions(player, world, &input, caller)[0].result
    }

    #[test]
    fn the_table_spends_levels_on_what_takes_the_enchantment_and_refuses_the_rest() {
        use CallerType::{Client, Server};
        use InventoryAction::{Close, Enchant, LeftClickSlot, OpenEngravingTable, QuickMoveSlot};
        let sharpness = Enchant(Enchantment::Sharpness);
        let (mut player, mut world) = engraver(ItemId::IronSword, 9);
        let mut does = |caller, action| {
            let result = act(&mut player, &mut world, caller, action);
            (result, player.experience.level)
        };
        // No table open, then nothing in it
        let no_table = Err(InventoryRefusal::NoEngravingTable);
        assert_eq!(does(Server, sharpness), (no_table, 9));
        assert_eq!(
            does(Server, LeftClickSlot(ENGRAVING_SLOT)).0,
            Err(InventoryRefusal::InvalidSlot(ENGRAVING_SLOT))
        );
        assert_eq!(does(Server, OpenEngravingTable(IVec3::ZERO)).0, no_table);
        assert_eq!(does(Server, OpenEngravingTable(TABLE)).0, Ok(()));
        let nothing = Err(InventoryRefusal::NothingToEnchant);
        assert_eq!(does(Server, sharpness), (nothing, 9));
        assert_eq!(does(Server, QuickMoveSlot(0)).0, Ok(()));
        let protection = Enchant(Enchantment::Protection);
        assert_eq!(does(Server, protection), (nothing, 9));
        // A client predicts nothing; the server spends 3 levels, then 5, then lacks 8
        assert_eq!(does(Client, sharpness), (Ok(()), 9));
        assert_eq!(does(Server, sharpness), (Ok(()), 6));
        assert_eq!(does(Server, sharpness), (Ok(()), 1));
        let lacking = Err(InventoryRefusal::NotEnoughLevels(8));
        assert_eq!(does(Server, sharpness), (lacking, 1));
        assert_eq!(does(Server, Close).0, Ok(()));
        let sword = player.inventory.inner[&0];
        assert_eq!(sword.bonus_blow_damage(), 2);
        assert!(!player.inventory.inner.contains_key(&ENGRAVING_SLOT));
        // The wear keeps the enchantments
        player.inventory.wear_tool(0, 5);
        let worn = player.inventory.inner[&0];
        assert_eq!(
            (worn.item_type.wear(), worn.bonus_blow_damage()),
            (Some(5), 2)
        );
    }

    #[test]
    fn protection_adds_to_the_armour_and_never_absorbs_a_whole_hit() {
        let (mut player, _) = engraver(ItemId::Stick, 0);
        player.game_mode = GameMode::Survival;
        let suit = [
            ItemId::DiamondHelmet,
            ItemId::DiamondChestplate,
            ItemId::DiamondLeggings,
            ItemId::DiamondBoots,
        ];
        for (item_id, piece) in suit.into_iter().zip(ArmourPiece::ALL) {
            let mut stack = ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb: 1,
            };
            while let Some(better) = stack.enchanted(Enchantment::Protection) {
                stack = better;
            }
            player.inventory.inner.insert(piece.slot(), stack);
        }
        // 76 % for the suit, 12 for its enchantments
        let blow = DamageSource::Mob(MobKind::Gnawer);
        assert_eq!(player.absorb(100, blow), 12);
        assert_eq!(player.absorb(100, DamageSource::Fall), 100);
    }
}
