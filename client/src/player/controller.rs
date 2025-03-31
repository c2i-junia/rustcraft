use crate::input::data::GameAction;
use crate::input::keyboard::*;
use crate::network::buffered_client::{
    CurrentFrameInputs, CurrentFrameInputsExt, PlayerTickInputsBuffer, SyncTime, SyncTimeExt,
};
use crate::player::ViewMode;
use crate::ui::hud::debug::DebugOptions;
use crate::ui::hud::UIMode;
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use crate::KeyMap;
use bevy::prelude::*;
use shared::messages::NetworkAction;
use shared::players::movement::simulate_player_movement;
use shared::players::Player;

use super::CurrentPlayerMarker;

#[derive(Component)]
pub struct PlayerMaterialHandle {
    pub handle: Handle<StandardMaterial>,
}

pub fn pre_input_update_system(
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut tick_buffer: ResMut<PlayerTickInputsBuffer>,
    mut sync_time: ResMut<SyncTime>,
) {
    sync_time.advance();

    let inputs_of_last_frame = frame_inputs.0.clone();
    tick_buffer.buffer.push(inputs_of_last_frame);
    frame_inputs.reset(sync_time.curr_time_ms, sync_time.delta());
}

pub fn player_movement_system(
    queries: Query<(&mut Player, &mut Transform), (With<CurrentPlayerMarker>, Without<Camera>)>,
    camera: Query<&Transform, With<Camera>>,
    resources: (
        Res<ButtonInput<KeyCode>>,
        Res<UIMode>,
        Res<KeyMap>,
        ResMut<CurrentFrameInputs>,
    ),
    world_map: Res<ClientWorldMap>,
) {
    let mut player_query = queries;
    let (keyboard_input, ui_mode, key_map, mut frame_inputs) = resources;

    if frame_inputs.0.delta_ms == 0 {
        return;
    }

    let camera = camera.single();
    let camera_orientation = camera.rotation;
    frame_inputs.0.camera = camera_orientation;

    let res = player_query.get_single_mut();
    // Return early if the player has not been spawned yet
    if res.is_err() {
        debug!("player not found");
        return;
    }

    let (mut player, mut player_transform) = player_query.single_mut();

    if *ui_mode == UIMode::Closed
        && is_action_just_pressed(GameAction::ToggleFlyMode, &keyboard_input, &key_map)
    {
        frame_inputs.0.inputs.insert(NetworkAction::ToggleFlyMode);
    }

    if is_action_pressed(GameAction::MoveBackward, &keyboard_input, &key_map) {
        frame_inputs.0.inputs.insert(NetworkAction::MoveBackward);
    }
    if is_action_pressed(GameAction::MoveForward, &keyboard_input, &key_map) {
        frame_inputs.0.inputs.insert(NetworkAction::MoveForward);
    }
    if is_action_pressed(GameAction::MoveLeft, &keyboard_input, &key_map) {
        frame_inputs.0.inputs.insert(NetworkAction::MoveLeft);
    }
    if is_action_pressed(GameAction::MoveRight, &keyboard_input, &key_map) {
        frame_inputs.0.inputs.insert(NetworkAction::MoveRight);
    }
    if is_action_pressed(GameAction::Jump, &keyboard_input, &key_map) {
        frame_inputs.0.inputs.insert(NetworkAction::JumpOrFlyUp);
    }
    if is_action_pressed(GameAction::FlyDown, &keyboard_input, &key_map) {
        frame_inputs.0.inputs.insert(NetworkAction::SneakOrFlyDown);
    }

    simulate_player_movement(&mut player, world_map.as_ref(), &frame_inputs.0);

    frame_inputs.0.position = player.position;

    player_transform.translation = player.position;

    // debug!(
    //     "At t={}, player position: {:?}",
    //     frame_inputs.0.time_ms, player.position
    // );
}

pub fn first_and_third_person_view_system(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut view_mode: ResMut<ViewMode>,
    mut player_query: Query<&mut PlayerMaterialHandle, With<CurrentPlayerMarker>>,
    key_map: Res<KeyMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
) {
    if *ui_mode == UIMode::Closed
        && is_action_just_pressed(GameAction::ToggleViewMode, &keyboard_input, &key_map)
    {
        view_mode.toggle();
    }

    let material_handle = player_query.get_single_mut();
    // Return early if the player has not been spawned yet
    if material_handle.is_err() {
        debug!("player not found");
        return;
    }

    let material_handle = &material_handle.unwrap().handle;

    match *view_mode {
        ViewMode::FirstPerson => {
            // make player transparent
            if let Some(material) = materials.get_mut(material_handle) {
                material.base_color = Color::srgba(0.0, 0.0, 0.0, 0.0);
            }
        }
        ViewMode::ThirdPerson => {
            if let Some(material) = materials.get_mut(material_handle) {
                material.base_color = Color::srgba(1.0, 0.0, 0.0, 1.0);
            }
        }
    }
}

pub fn toggle_chunk_debug_mode_system(
    mut debug_options: ResMut<DebugOptions>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if is_action_just_pressed(GameAction::ToggleChunkDebugMode, &keyboard_input, &key_map) {
        debug_options.toggle_chunk_debug_mode();
    }
}

pub fn toggle_raycast_debug_mode_system(
    mut debug_options: ResMut<DebugOptions>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if is_action_just_pressed(
        GameAction::ToggleRaycastDebugMode,
        &keyboard_input,
        &key_map,
    ) {
        debug_options.toggle_raycast_debug_mode();
    }
}

pub fn chunk_force_reload_system(
    mut world_map: ResMut<ClientWorldMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    mut ev_writer: EventWriter<WorldRenderRequestUpdateEvent>,
    mut commands: Commands,
) {
    if is_action_just_pressed(GameAction::ReloadChunks, &keyboard_input, &key_map) {
        for (pos, chunk) in world_map.map.iter_mut() {
            // Despawn the chunk's entity
            if let Some(e) = chunk.entity {
                commands.entity(e).despawn_recursive();
                chunk.entity = None;
            }
            // Request a render for this chunk
            ev_writer.send(WorldRenderRequestUpdateEvent::ChunkToReload(*pos));
        }
    }
}
