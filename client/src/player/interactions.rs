use crate::input::data::GameAction;
use crate::input::keyboard::is_action_pressed;
use crate::network::buffered_client::CurrentFrameInputs;
use crate::state::KeyMap;
use crate::ui::hud::{InventoryRoot, UIMode};
use crate::world::{BlockChanged, ClientWorldMap};
use bevy::color::palettes::css::WHITE;
use bevy::prelude::*;
use shared::messages::NetworkAction;
use shared::players::blocks::{simulate_player_block_interactions, CallerType};
use shared::players::inventory::{simulate_inventory_actions, InventoryAction};
use shared::players::{Inventory, Player, ViewMode};
use shared::world::{raycast, BlockId, FaceDirectionExt};

use super::CurrentPlayerMarker;

/// Outlines the block the player aims at and, while a mouse button is held on it, predicts
/// breaking it or placing a block against it, and reports the blocks that changed. A right
/// click on a crafting table opens it instead, unless the player sneaks.
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

    let Some(target) = raycast::raycast(
        &world_map.blocks,
        camera_transform,
        &player_transform.translation,
        *view_mode,
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
    let aims_at_a_table = world_map
        .blocks
        .get_block(target.position)
        .is_some_and(|block| block.id == BlockId::CraftingTable);
    if aims_at_a_table && !sneaking && mouse_input.just_pressed(MouseButton::Right) {
        frame_inputs
            .0
            .inventory_actions
            .push(InventoryAction::OpenCraftingTable(target.position));
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

/// Predicts the inventory actions of the frame on the local player, and shows the result.
pub fn predict_inventory_system(
    mut player: Query<&mut Player, With<CurrentPlayerMarker>>,
    frame_inputs: Res<CurrentFrameInputs>,
    mut world_map: ResMut<ClientWorldMap>,
    mut inventory: ResMut<Inventory>,
) {
    let Ok(mut player) = player.single_mut() else {
        return;
    };
    if frame_inputs.0.inventory_actions.is_empty() {
        return;
    }
    simulate_inventory_actions(
        &mut player,
        &mut world_map.blocks,
        &frame_inputs.0,
        CallerType::Client,
    );
    inventory.clone_from(&player.inventory);
}
