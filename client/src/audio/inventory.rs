//! The sounds of the inventory: a stack picked up or put down, a craft, and a refusal.

use bevy::prelude::*;
use shared::players::inventory::{InventoryAction, InventoryActionOutcome};

use super::playback::PlaySound;
use super::sounds::Sound;
use crate::player::InventoryActionPredicted;
use crate::ui::widgets::slot::ActionRefused;

/// The sound of an accepted inventory action: a craft knocks; a stack onto the cursor (more of
/// its item, or another stack) is picked, items off it into slots placed, by the actions on the
/// cursor; every other action is silent, a drop too, as in Minecraft: the item seen flying is
/// enough. A refusal sounds through its `ActionRefused`.
pub fn inventory_sound(outcome: &InventoryActionOutcome) -> Option<Sound> {
    outcome.result.ok()?;
    match outcome.action {
        InventoryAction::TakeCraftResult
        | InventoryAction::CraftAll
        | InventoryAction::CraftToHotbar(_) => Some(Sound::Craft),
        InventoryAction::LeftClickSlot(_)
        | InventoryAction::RightClickSlot(_)
        | InventoryAction::Drag { .. }
        | InventoryAction::CollectAll
        | InventoryAction::TakeFromCatalog { .. } => {
            match (outcome.held_before, outcome.held_after) {
                (_, None) if outcome.held_before.is_some() => Some(Sound::ItemPlace),
                (None, Some(_)) => Some(Sound::ItemPick),
                (Some(before), Some(after)) if before.item_id != after.item_id => {
                    Some(Sound::ItemPick)
                }
                (Some(before), Some(after)) if after.nb > before.nb => Some(Sound::ItemPick),
                (Some(before), Some(after)) if after.nb < before.nb => Some(Sound::ItemPlace),
                _ => None,
            }
        }
        InventoryAction::Close
        | InventoryAction::OpenCraftingTable(_)
        | InventoryAction::OpenChest(_)
        | InventoryAction::OpenFurnace(_)
        | InventoryAction::QuickMoveSlot(_)
        | InventoryAction::SwapWithHotbar { .. }
        | InventoryAction::MoveAllOfType { .. }
        | InventoryAction::FillCraftingGrid { .. }
        | InventoryAction::DestroyFloating
        | InventoryAction::DropSlot { .. }
        | InventoryAction::DropFloating { .. } => None,
    }
}

/// Plays the sounds of the inventory actions the player made this frame, and the refused sound
/// for its refusals: each kind once a frame, however many actions an input carries.
pub fn inventory_sounds(
    mut predicted: MessageReader<InventoryActionPredicted>,
    mut refusals: MessageReader<ActionRefused>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let mut heard = Vec::new();
    let refused = refusals.read().next().map(|_| Sound::Refused);
    // Read to the end: the next frame does not hear them again
    refusals.clear();
    let played = predicted
        .read()
        .filter_map(|InventoryActionPredicted(outcome)| inventory_sound(outcome))
        .chain(refused);
    for sound in played {
        if !heard.contains(&sound) {
            heard.push(sound);
        }
    }
    sounds.write_batch(heard.into_iter().map(PlaySound::here));
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::inventory::InventoryRefusal;
    use shared::world::{ItemId, ItemStack};

    fn stack(item_id: ItemId, nb: u32) -> Option<ItemStack> {
        Some(ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        })
    }

    fn outcome(
        action: InventoryAction,
        held_before: Option<ItemStack>,
        held_after: Option<ItemStack>,
    ) -> InventoryActionOutcome {
        InventoryActionOutcome {
            action,
            held_before,
            held_after,
            result: Ok(()),
            thrown: None,
        }
    }

    #[test]
    fn a_stack_onto_the_cursor_is_picked_and_off_it_placed() {
        use InventoryAction as A;
        let dirt = |nb| stack(ItemId::Dirt, nb);
        let cases = [
            (A::LeftClickSlot(0), None, dirt(5), Some(Sound::ItemPick)),
            (A::LeftClickSlot(0), dirt(5), None, Some(Sound::ItemPlace)),
            // A swap leaves another stack on the cursor
            (
                A::LeftClickSlot(0),
                dirt(5),
                stack(ItemId::Stone, 5),
                Some(Sound::ItemPick),
            ),
            (
                A::LeftClickSlot(0),
                dirt(5),
                dirt(64),
                Some(Sound::ItemPick),
            ),
            (
                A::LeftClickSlot(0),
                dirt(64),
                dirt(20),
                Some(Sound::ItemPlace),
            ),
            (A::LeftClickSlot(0), None, None, None),
            (A::RightClickSlot(0), None, dirt(3), Some(Sound::ItemPick)),
            (
                A::RightClickSlot(0),
                dirt(5),
                dirt(4),
                Some(Sound::ItemPlace),
            ),
            (A::RightClickSlot(0), dirt(5), dirt(5), None),
            (
                A::Drag {
                    slots: 0b11,
                    one_each: false,
                },
                dirt(17),
                dirt(1),
                Some(Sound::ItemPlace),
            ),
            (A::CollectAll, dirt(3), dirt(64), Some(Sound::ItemPick)),
            (A::CollectAll, dirt(3), dirt(3), None),
            (
                A::TakeFromCatalog {
                    item: ItemId::Dirt,
                    count: 64,
                    to_inventory: false,
                },
                None,
                dirt(64),
                Some(Sound::ItemPick),
            ),
        ];
        for (action, before, after, sound) in cases {
            assert_eq!(
                inventory_sound(&outcome(action, before, after)),
                sound,
                "{action:?} {before:?} -> {after:?}"
            );
        }
    }

    #[test]
    fn a_craft_knocks_whatever_the_cursor_does() {
        let planks = stack(ItemId::OakPlanks, 4);
        for (action, before, after) in [
            (InventoryAction::TakeCraftResult, None, planks),
            (InventoryAction::CraftAll, None, None),
            (InventoryAction::CraftToHotbar(3), planks, planks),
        ] {
            assert_eq!(
                inventory_sound(&outcome(action, before, after)),
                Some(Sound::Craft),
                "{action:?}"
            );
        }
    }

    #[test]
    fn the_actions_that_move_nothing_through_the_cursor_are_silent() {
        use InventoryAction as A;
        let dirt = stack(ItemId::Dirt, 5);
        for (action, before, after) in [
            (A::QuickMoveSlot(0), None, None),
            (
                A::SwapWithHotbar {
                    slot: 12,
                    hotbar: 0,
                },
                None,
                None,
            ),
            (
                A::MoveAllOfType {
                    slot: 0,
                    item: ItemId::Dirt,
                },
                None,
                None,
            ),
            (
                A::FillCraftingGrid {
                    recipe: 0,
                    times: 1,
                },
                None,
                None,
            ),
            (A::Close, dirt, None),
            (A::DestroyFloating, dirt, None),
            (A::OpenChest(IVec3::ZERO), None, None),
            (A::OpenCraftingTable(IVec3::ZERO), None, None),
            (A::OpenFurnace(IVec3::ZERO), None, None),
        ] {
            assert_eq!(
                inventory_sound(&outcome(action, before, after)),
                None,
                "{action:?}"
            );
        }
    }

    #[test]
    fn a_drop_is_silent_even_when_it_empties_the_cursor() {
        use InventoryAction as A;
        let dirt = |nb| stack(ItemId::Dirt, nb);
        for (action, before, after) in [
            (A::DropFloating { all: true }, dirt(5), None),
            (A::DropFloating { all: false }, dirt(5), dirt(4)),
            (A::DropSlot { slot: 3, all: true }, None, None),
        ] {
            let thrown = InventoryActionOutcome {
                thrown: dirt(1),
                ..outcome(action, before, after)
            };
            assert_eq!(inventory_sound(&thrown), None, "{action:?}");
        }
    }

    #[test]
    fn a_refused_action_sounds_through_its_refusal_alone() {
        let refused = InventoryActionOutcome {
            result: Err(InventoryRefusal::NoRecipe),
            ..outcome(InventoryAction::TakeCraftResult, None, None)
        };
        assert_eq!(inventory_sound(&refused), None);
    }

    fn heard(app: &mut App) -> Vec<Sound> {
        app.world_mut()
            .resource_mut::<Messages<PlaySound>>()
            .drain()
            .map(|request| {
                assert_eq!(request.at, None);
                request.sound
            })
            .collect()
    }

    #[test]
    fn a_frame_plays_each_kind_of_inventory_sound_once() {
        let mut app = App::new();
        app.add_message::<InventoryActionPredicted>()
            .add_message::<ActionRefused>()
            .add_message::<PlaySound>()
            .add_systems(Update, inventory_sounds);
        let dirt = stack(ItemId::Dirt, 5);
        let slot = app.world_mut().spawn_empty().id();
        app.world_mut().write_message_batch(
            [
                outcome(InventoryAction::LeftClickSlot(0), None, dirt),
                outcome(InventoryAction::TakeCraftResult, None, dirt),
                outcome(InventoryAction::LeftClickSlot(1), None, dirt),
            ]
            .map(InventoryActionPredicted),
        );
        app.world_mut().write_message_batch([
            ActionRefused { slot: Some(slot) },
            ActionRefused { slot: None },
        ]);

        app.update();

        assert_eq!(
            heard(&mut app),
            [Sound::ItemPick, Sound::Craft, Sound::Refused]
        );
        app.update();
        assert_eq!(heard(&mut app), []);
    }
}
