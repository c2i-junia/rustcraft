//! What a furnace holds and how it smelts. A furnace lives in the `ChunkMap`, at the position of
//! its block, like the contents of a chest: the server holds and ticks every furnace, a client
//! only holds the one its player has open.

use serde::{Deserialize, Serialize};

use crate::world::{ItemId, ItemStack};

/// Ticks to smelt one item: 10 seconds, as in Minecraft.
pub const COOK_TICKS: u16 = 200;

/// The three stacks of a furnace and how far it is in burning and cooking. Its fields are what
/// the furnace files of a world hold, in this order.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Furnace {
    pub input: Option<ItemStack>,
    pub fuel: Option<ItemStack>,
    pub output: Option<ItemStack>,
    /// Ticks the fuel being burnt still lasts; 0 when the furnace is out.
    pub burn_left: u16,
    /// Ticks the fuel being burnt lasted in full, for the flame of the dialog.
    pub burn_total: u16,
    /// Ticks the item in `input` has cooked, up to `COOK_TICKS`.
    pub cook: u16,
}

/// What one tick did to a furnace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FurnaceChange {
    /// Nothing: it neither burns nor cooks.
    Same,
    /// It burnt or cooked, and stayed lit or out.
    Changed,
    /// It lit, or went out: its block becomes the other one of `Furnace` and `LitFurnace`.
    Toggled,
}

/// What a furnace makes of `item`, if it smelts. The server smelts by it and the client shows
/// it, so both read this one table.
pub fn smelting_result(item: ItemId) -> Option<ItemId> {
    match item {
        ItemId::Sand => Some(ItemId::Glass),
        ItemId::Cobblestone => Some(ItemId::Stone),
        _ => None,
    }
}

/// How many ticks one `item` burns in a furnace, if it is a fuel: Minecraft's durations.
pub fn fuel_ticks(item: ItemId) -> Option<u16> {
    match item {
        ItemId::Coal => Some(1600),
        ItemId::OakLog
        | ItemId::SpruceLog
        | ItemId::JungleLog
        | ItemId::OakPlanks
        | ItemId::SprucePlanks
        | ItemId::JunglePlanks
        | ItemId::CraftingTable
        | ItemId::Chest => Some(300),
        ItemId::WoodenPickaxe
        | ItemId::WoodenAxe
        | ItemId::WoodenShovel
        | ItemId::WoodenHoe
        | ItemId::WoodenSword => Some(200),
        ItemId::Stick => Some(100),
        _ => None,
    }
}

impl Furnace {
    /// Whether a fuel is burning.
    pub fn is_lit(&self) -> bool {
        self.burn_left > 0
    }

    /// The stacks the furnace holds: its input, its fuel and its output.
    pub fn stacks(&self) -> impl Iterator<Item = ItemStack> {
        [self.input, self.fuel, self.output].into_iter().flatten()
    }

    /// What the input makes, if the furnace can smelt it now: the input has a result, and the
    /// output is empty or holds that result with room for one more.
    fn smelting(&self) -> Option<ItemId> {
        let result = smelting_result(self.input?.item_id)?;
        match self.output {
            Some(output) if output.item_id != result || output.nb >= result.get_max_stack() => None,
            _ => Some(result),
        }
    }

    /// One tick, by Minecraft's rules. The fuel burns on whatever the furnace holds. Once it is
    /// out, a furnace that can smelt burns its next fuel item, on the same tick, so that a fuel
    /// cooks as many ticks as it lasts. A burning furnace that can smelt cooks, an item every
    /// `COOK_TICKS`; one that cannot loses what it cooked, and one with nothing to burn loses it
    /// two ticks at a time.
    pub fn tick(&mut self) -> FurnaceChange {
        let before = self.clone();
        self.burn_left = self.burn_left.saturating_sub(1);
        let smelting = self.smelting();
        if self.burn_left == 0 && smelting.is_some() {
            self.burn_fuel();
        }
        match smelting {
            Some(result) if self.is_lit() => {
                self.cook += 1;
                if self.cook >= COOK_TICKS {
                    self.cook = 0;
                    self.smelt(result);
                }
            }
            Some(_) => self.cook = self.cook.saturating_sub(2),
            None => self.cook = 0,
        }
        if before.is_lit() != self.is_lit() {
            FurnaceChange::Toggled
        } else if before != *self {
            FurnaceChange::Changed
        } else {
            FurnaceChange::Same
        }
    }

    /// Lights the next fuel item, if the fuel slot holds a fuel.
    fn burn_fuel(&mut self) {
        let Some(fuel) = self.fuel else {
            return;
        };
        let Some(ticks) = fuel_ticks(fuel.item_id) else {
            return;
        };
        self.fuel = one_less(fuel);
        self.burn_left = ticks;
        self.burn_total = ticks;
    }

    /// One input item becomes one `result` in the output.
    fn smelt(&mut self, result: ItemId) {
        self.input = self.input.and_then(one_less);
        self.output = Some(ItemStack {
            item_id: result,
            item_type: result.get_default_type(),
            nb: self.output.map_or(0, |output| output.nb) + 1,
        });
    }
}

/// `stack` with an item less, `None` once it is empty.
fn one_less(stack: ItemStack) -> Option<ItemStack> {
    (stack.nb > 1).then_some(ItemStack {
        nb: stack.nb - 1,
        ..stack
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    fn furnace(
        input: Option<ItemStack>,
        fuel: Option<ItemStack>,
        output: Option<ItemStack>,
    ) -> Furnace {
        Furnace {
            input,
            fuel,
            output,
            ..Default::default()
        }
    }

    /// Ticks `furnace` `ticks` times.
    fn run(furnace: &mut Furnace, ticks: u32) {
        for _ in 0..ticks {
            furnace.tick();
        }
    }

    const COAL_TICKS: u16 = 1600;

    #[test]
    fn sand_and_cobblestone_smelt_and_the_rest_does_not() {
        assert_eq!(smelting_result(ItemId::Sand), Some(ItemId::Glass));
        assert_eq!(smelting_result(ItemId::Cobblestone), Some(ItemId::Stone));
        for item in [ItemId::Dirt, ItemId::Glass, ItemId::Coal, ItemId::Stick] {
            assert_eq!(smelting_result(item), None, "{item:?}");
        }
    }

    #[test]
    fn coal_wood_and_wooden_things_burn_as_in_minecraft() {
        assert_eq!(fuel_ticks(ItemId::Coal), Some(COAL_TICKS));
        for item in [
            ItemId::OakLog,
            ItemId::SpruceLog,
            ItemId::JungleLog,
            ItemId::OakPlanks,
            ItemId::SprucePlanks,
            ItemId::JunglePlanks,
            ItemId::CraftingTable,
            ItemId::Chest,
        ] {
            assert_eq!(fuel_ticks(item), Some(300), "{item:?}");
        }
        for item in [
            ItemId::WoodenPickaxe,
            ItemId::WoodenAxe,
            ItemId::WoodenShovel,
            ItemId::WoodenHoe,
            ItemId::WoodenSword,
        ] {
            assert_eq!(fuel_ticks(item), Some(200), "{item:?}");
        }
        assert_eq!(fuel_ticks(ItemId::Stick), Some(100));
        for item in [
            ItemId::Dirt,
            ItemId::Sand,
            ItemId::Cobblestone,
            ItemId::Torch,
        ] {
            assert_eq!(fuel_ticks(item), None, "{item:?}");
        }
    }

    #[test]
    fn an_empty_furnace_stays_the_same() {
        let mut empty = Furnace::default();
        assert_eq!(empty.tick(), FurnaceChange::Same);
        assert_eq!(empty, Furnace::default());
        assert!(!empty.is_lit());
    }

    #[test]
    fn it_lights_with_fuel_and_something_to_smelt_burning_one_fuel_item() {
        let mut lit = furnace(
            Some(stack(ItemId::Sand, 3)),
            Some(stack(ItemId::Coal, 2)),
            None,
        );
        assert_eq!(lit.tick(), FurnaceChange::Toggled);
        assert!(lit.is_lit());
        assert_eq!(lit.fuel, Some(stack(ItemId::Coal, 1)));
        assert_eq!((lit.burn_left, lit.burn_total), (COAL_TICKS, COAL_TICKS));
        assert_eq!(lit.cook, 1);
        assert_eq!(lit.tick(), FurnaceChange::Changed);

        // The last fuel item leaves the slot empty
        let mut lit = furnace(
            Some(stack(ItemId::Sand, 3)),
            Some(stack(ItemId::Stick, 1)),
            None,
        );
        lit.tick();
        assert_eq!(lit.fuel, None);
        assert_eq!(lit.burn_total, 100);
    }

    #[test]
    fn it_stays_out_with_nothing_it_can_smelt() {
        let coal = Some(stack(ItemId::Coal, 2));
        for idle in [
            furnace(None, coal, None),
            furnace(Some(stack(ItemId::Dirt, 3)), coal, None),
            furnace(
                Some(stack(ItemId::Sand, 3)),
                Some(stack(ItemId::Dirt, 3)),
                None,
            ),
            furnace(
                Some(stack(ItemId::Sand, 3)),
                coal,
                Some(stack(ItemId::Stone, 1)),
            ),
            furnace(
                Some(stack(ItemId::Sand, 3)),
                coal,
                Some(stack(ItemId::Glass, 64)),
            ),
        ] {
            let mut furnace = idle.clone();
            assert_eq!(furnace.tick(), FurnaceChange::Same, "{idle:?}");
            assert_eq!(furnace, idle);
        }
    }

    #[test]
    fn one_item_cooks_in_cook_ticks() {
        let mut cooking = furnace(
            Some(stack(ItemId::Sand, 2)),
            Some(stack(ItemId::Coal, 1)),
            None,
        );
        run(&mut cooking, u32::from(COOK_TICKS) - 1);
        assert_eq!(cooking.cook, COOK_TICKS - 1);
        assert_eq!(cooking.output, None);

        cooking.tick();
        assert_eq!(cooking.cook, 0);
        assert_eq!(cooking.input, Some(stack(ItemId::Sand, 1)));
        assert_eq!(cooking.output, Some(stack(ItemId::Glass, 1)));

        run(&mut cooking, u32::from(COOK_TICKS));
        assert_eq!(cooking.input, None);
        assert_eq!(cooking.output, Some(stack(ItemId::Glass, 2)));
    }

    #[test]
    fn a_coal_smelts_eight_items_and_the_next_one_follows_without_a_gap() {
        let mut cooking = furnace(
            Some(stack(ItemId::Cobblestone, 20)),
            Some(stack(ItemId::Coal, 2)),
            None,
        );
        run(&mut cooking, u32::from(COAL_TICKS));
        assert_eq!(cooking.output, Some(stack(ItemId::Stone, 8)));
        assert_eq!(cooking.fuel, Some(stack(ItemId::Coal, 1)));
        assert_eq!(cooking.burn_left, 1);

        // The first coal runs out and the second lights on the same tick
        assert_eq!(cooking.tick(), FurnaceChange::Changed);
        assert!(cooking.is_lit());
        assert_eq!(cooking.fuel, None);
        assert_eq!(cooking.cook, 1);

        run(&mut cooking, u32::from(COAL_TICKS) - 1);
        assert_eq!(cooking.output, Some(stack(ItemId::Stone, 16)));
        assert_eq!(cooking.tick(), FurnaceChange::Toggled);
        assert!(!cooking.is_lit());
        assert_eq!(cooking.input, Some(stack(ItemId::Cobblestone, 4)));
    }

    #[test]
    fn the_fuel_burns_on_without_input_and_the_cooking_is_lost() {
        let mut cooking = furnace(
            Some(stack(ItemId::Sand, 2)),
            Some(stack(ItemId::Stick, 1)),
            None,
        );
        run(&mut cooking, 50);
        assert_eq!(cooking.cook, 50);

        cooking.input = None;
        assert_eq!(cooking.tick(), FurnaceChange::Changed);
        assert_eq!(cooking.cook, 0);
        assert_eq!(cooking.burn_left, 50);
        run(&mut cooking, 49);
        assert_eq!(cooking.tick(), FurnaceChange::Toggled);
        assert!(!cooking.is_lit());
        assert_eq!(cooking.tick(), FurnaceChange::Same);
    }

    #[test]
    fn out_of_fuel_the_cooking_goes_back_two_ticks_per_tick() {
        let mut cooking = furnace(
            Some(stack(ItemId::Sand, 2)),
            Some(stack(ItemId::Stick, 1)),
            None,
        );
        run(&mut cooking, 100);
        assert_eq!((cooking.cook, cooking.burn_left), (100, 1));
        assert_eq!(cooking.tick(), FurnaceChange::Toggled);
        assert_eq!(cooking.cook, 98);
        run(&mut cooking, 48);
        assert_eq!(cooking.cook, 2);
        assert_eq!(cooking.tick(), FurnaceChange::Changed);
        assert_eq!(cooking.cook, 0);
        assert_eq!(cooking.tick(), FurnaceChange::Same);
        assert_eq!(cooking.input, Some(stack(ItemId::Sand, 2)));
    }

    #[test]
    fn the_output_fills_up_to_a_full_stack_and_the_furnace_stops() {
        let mut cooking = furnace(
            Some(stack(ItemId::Sand, 5)),
            Some(stack(ItemId::Coal, 1)),
            Some(stack(ItemId::Glass, 63)),
        );
        run(&mut cooking, u32::from(COOK_TICKS));
        assert_eq!(cooking.output, Some(stack(ItemId::Glass, 64)));
        assert_eq!(cooking.input, Some(stack(ItemId::Sand, 4)));

        // The fuel burns on, but nothing cooks
        let burn_left = cooking.burn_left;
        cooking.tick();
        assert_eq!(cooking.cook, 0);
        assert_eq!(cooking.burn_left, burn_left - 1);
        assert_eq!(cooking.input, Some(stack(ItemId::Sand, 4)));
    }

    #[test]
    fn furnace_encoding_is_pinned() {
        use bincode::Options;
        let furnace = Furnace {
            input: Some(stack(ItemId::Sand, 3)),
            fuel: None,
            output: Some(stack(ItemId::Glass, 1)),
            burn_left: 5,
            burn_total: 7,
            cook: 9,
        };
        assert_eq!(
            bincode::options().serialize(&furnace).unwrap(),
            [1, 6, 1, 7, 3, 0, 1, 9, 1, 10, 1, 5, 7, 9],
            "the fields of Furnace changed: the furnace files of the saved worlds would be \
            misread; bump PROTOCOL_ID and convert them"
        );
    }
}
