use crate::input::data::GameAction;
use crate::input::keyboard::is_action_pressed;
use crate::network::buffered_client::CurrentFrameInputs;
use crate::state::KeyMap;
use crate::ui::hud::{InventoryRoot, UIMode};
use crate::world::{BlockChanged, ClientWorldMap};
use bevy::color::palettes::css::WHITE;
use bevy::prelude::*;
use shared::messages::NetworkAction;
use shared::players::blocks::{aim, held_item, simulate_player_block_interactions, CallerType};
use shared::players::inventory::{
    simulate_inventory_actions, InventoryAction, InventoryActionOutcome,
};
use shared::players::{Inventory, Player, ViewMode};
use shared::world::{BlockId, FaceDirectionExt};

use super::CurrentPlayerMarker;

/// Outlines the block the player aims at and, while a mouse button is held on it, predicts
/// breaking it or placing a block against it, and reports the blocks that changed. A right
/// click on a crafting table, a chest or a furnace opens it instead, unless the player sneaks.
/// Holding an empty bucket, the player aims at the water sources too.
pub fn handle_block_interactions(
    mut player_query: Query<(&mut Player, &Transform), With<CurrentPlayerMarker>>,
    camera_query: Query<&Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
    world_map: ResMut<ClientWorldMap>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut pressed_in_ui: Local<Vec<MouseButton>>,
    (keyboard_input, key_map): (Res<ButtonInput<KeyCode>>, Res<KeyMap>),
    mut inventory_root: Query<&mut Visibility, With<InventoryRoot>>,
    ui_mode: Res<UIMode>,
    view_mode: Res<ViewMode>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut gizmos: Gizmos,
    mut block_changes: MessageWriter<BlockChanged>,
) {
    // A button pressed over a UI, to close it for instance, is not for the world: it is ignored
    // until released
    if *ui_mode != UIMode::Closed {
        for button in mouse_input.get_pressed() {
            if !pressed_in_ui.contains(button) {
                pressed_in_ui.push(*button);
            }
        }
        return;
    }
    pressed_in_ui.retain(|button| mouse_input.pressed(*button));
    let held = |button| mouse_input.pressed(button) && !pressed_in_ui.contains(&button);
    let (Ok((mut player, player_transform)), Ok(camera_transform)) =
        (player_query.single_mut(), camera_query.single())
    else {
        return;
    };
    let world_map = world_map.into_inner();

    let Some(target) = aim(
        &world_map.blocks,
        camera_transform,
        &player_transform.translation,
        *view_mode,
        held_item(&player, &frame_inputs.0),
    ) else {
        return;
    };

    let center = (target.bbox.max + target.bbox.min) / 2.0;
    let size = target.bbox.max - target.bbox.min;
    gizmos.cube(
        Transform::from_translation(center.into()).with_scale(size.into()),
        WHITE,
    );

    if held(MouseButton::Left) {
        frame_inputs.0.inputs.insert(NetworkAction::LeftClick);
    }
    // The simulation must see the sneak key, which decides between opening and placing
    let sneaking = is_action_pressed(GameAction::FlyDown, &keyboard_input, &key_map);
    if sneaking {
        frame_inputs.0.inputs.insert(NetworkAction::SneakOrFlyDown);
    }
    let opens = match world_map
        .blocks
        .get_block(target.position)
        .map(|block| block.id)
    {
        Some(BlockId::CraftingTable) => Some(InventoryAction::OpenCraftingTable(target.position)),
        Some(BlockId::Chest) => Some(InventoryAction::OpenChest(target.position)),
        Some(BlockId::Furnace | BlockId::LitFurnace) => {
            Some(InventoryAction::OpenFurnace(target.position))
        }
        _ => None,
    };
    let opens = opens.filter(|_| !sneaking && mouse_input.just_pressed(MouseButton::Right));
    if let Some(open) = opens {
        frame_inputs.0.inventory_actions.push(open);
        if let Ok(mut visibility) = inventory_root.single_mut() {
            *visibility = Visibility::Visible;
        }
    } else if held(MouseButton::Right) {
        frame_inputs.0.inputs.insert(NetworkAction::RightClick);
    }

    // The aimed block, and the cell a block placed against it takes
    let touched = [target.position, target.position + target.face.to_ivec3()];
    let before = touched.map(|position| world_map.blocks.get_block(position).copied());

    simulate_player_block_interactions(
        &mut player,
        &mut world_map.blocks,
        &frame_inputs.0,
        CallerType::Client,
    );

    for (position, before) in touched.into_iter().zip(before) {
        let after = world_map.blocks.get_block(position).copied();
        if after != before {
            block_changes.write(BlockChanged {
                position,
                before,
                after,
                by_local_player: true,
            });
        }
    }
}

/// What one inventory action of the local player did, as predicted when the player made it.
/// Replays of the reconciliation are not reported: each action is reported once, which the
/// sounds and the shake of a refused slot rely on.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct InventoryActionPredicted(pub InventoryActionOutcome);

/// Predicts the inventory actions of the frame on the local player, shows the result, and
/// reports what each action did.
pub fn predict_inventory_system(
    mut player: Query<&mut Player, With<CurrentPlayerMarker>>,
    frame_inputs: Res<CurrentFrameInputs>,
    mut world_map: ResMut<ClientWorldMap>,
    mut inventory: ResMut<Inventory>,
    mut predicted: MessageWriter<InventoryActionPredicted>,
) {
    let Ok(mut player) = player.single_mut() else {
        return;
    };
    if frame_inputs.0.inventory_actions.is_empty() {
        return;
    }
    let outcomes = simulate_inventory_actions(
        &mut player,
        &mut world_map.blocks,
        &frame_inputs.0,
        CallerType::Client,
    );
    inventory.clone_from(&player.inventory);
    predicted.write_batch(outcomes.into_iter().map(InventoryActionPredicted));
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::inventory::{InventoryActionOutcome, InventoryRefusal};
    use shared::world::{ItemId, ItemStack};

    #[test]
    fn the_prediction_reports_each_inventory_action_of_the_frame() {
        let mut app = App::new();
        app.add_message::<InventoryActionPredicted>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<CurrentFrameInputs>()
            .insert_resource(Inventory::new())
            .add_systems(Update, predict_inventory_system);
        let dirt = ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb: 5,
        };
        let mut player = Player::default();
        player.inventory.inner.insert(0, dirt);
        app.world_mut().spawn((player, CurrentPlayerMarker {}));
        let actions = &mut app
            .world_mut()
            .resource_mut::<CurrentFrameInputs>()
            .into_inner()
            .0
            .inventory_actions;
        actions.extend([
            InventoryAction::LeftClickSlot(0),
            InventoryAction::TakeCraftResult,
        ]);

        app.update();

        let reported: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<InventoryActionPredicted>>()
            .drain()
            .map(|predicted| predicted.0)
            .collect();
        assert_eq!(
            reported,
            [
                InventoryActionOutcome {
                    action: InventoryAction::LeftClickSlot(0),
                    held_before: None,
                    held_after: Some(dirt),
                    result: Ok(()),
                    thrown: None,
                },
                InventoryActionOutcome {
                    action: InventoryAction::TakeCraftResult,
                    held_before: Some(dirt),
                    held_after: Some(dirt),
                    result: Err(InventoryRefusal::NoRecipe),
                    thrown: None,
                },
            ]
        );
    }
}
