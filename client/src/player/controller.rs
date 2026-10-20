use crate::input::data::GameAction;
use crate::input::keyboard::*;
use crate::network::buffered_client::{
    CurrentFrameInputs, CurrentFrameInputsExt, PlayerTickInputsBuffer, SyncTime, SyncTimeExt,
};
use crate::state::KeyMap;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::UIMode;
use crate::world::ClientWorldMap;
use bevy::prelude::*;
use shared::messages::{NetworkAction, PlayerFrameInput};
use shared::players::movement::{is_terrain_loaded, simulate_player_movement};
use shared::players::{MovementState, Player, ViewMode};

use super::CurrentPlayerMarker;

pub fn update_frame_inputs_system(
    camera: Query<&Transform, With<Camera>>,
    hotbar: Query<&Hotbar>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    view_mode: Res<ViewMode>,
) {
    if frame_inputs.0.delta_ms == 0 {
        return;
    }

    let (Ok(camera), Ok(hotbar)) = (camera.single(), hotbar.single()) else {
        return;
    };
    frame_inputs.0.camera = *camera;
    frame_inputs.0.hotbar_slot = hotbar.selected;
    frame_inputs.0.view_mode = *view_mode;
}

#[derive(Component)]
pub struct PlayerMaterialHandle {
    pub handle: Handle<StandardMaterial>,
}

pub fn pre_input_update_system(
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut sync_time: ResMut<SyncTime>,
) {
    sync_time.advance();
    frame_inputs.reset(sync_time.curr_time_ms, sync_time.delta());
}

/// Whether the local player is simulated this frame: some time has passed since the last frame,
/// it is alive and its terrain has arrived. A frame that is not simulated is not sent either, so
/// that the server only ever simulates the inputs the client predicted.
pub fn local_player_is_simulated(
    frame_inputs: Res<CurrentFrameInputs>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    world_map: Res<ClientWorldMap>,
) -> bool {
    frame_inputs.0.delta_ms != 0
        && player.single().is_ok_and(|player| {
            !player.is_dead() && is_terrain_loaded(player.position, &world_map.blocks)
        })
}

/// The movement keys, held down, and the action each of them sends to the server.
const HELD_ACTIONS: [(GameAction, NetworkAction); 6] = [
    (GameAction::MoveBackward, NetworkAction::MoveBackward),
    (GameAction::MoveForward, NetworkAction::MoveForward),
    (GameAction::MoveLeft, NetworkAction::MoveLeft),
    (GameAction::MoveRight, NetworkAction::MoveRight),
    (GameAction::Jump, NetworkAction::JumpOrFlyUp),
    (GameAction::FlyDown, NetworkAction::SneakOrFlyDown),
];

/// The local player moved by one predicted frame. Replays of the reconciliation are not
/// reported: each frame is reported once, as the player lives it.
#[derive(Message, Debug, Clone, Copy)]
pub struct LocalPlayerMoved {
    pub from: MovementState,
    pub to: MovementState,
}

/// Whether the player sprints after this frame. As in Minecraft, the sprint key starts a sprint
/// while moving forward, and the sprint lasts until the player stops moving forward, or sneaks
/// on foot: the key need not be held.
fn keeps_sprinting(
    was_sprinting: bool,
    sprint_key: bool,
    inputs: &PlayerFrameInput,
    flying: bool,
) -> bool {
    inputs.is_pressed(NetworkAction::MoveForward)
        && (flying || !inputs.is_pressed(NetworkAction::SneakOrFlyDown))
        && (was_sprinting || sprint_key)
}

/// Completes the frame's inputs with the movement keys, predicts their effect on the local
/// player and queues the frame for the server.
pub fn player_movement_system(
    mut player_query: Query<
        (&mut Player, &mut Transform),
        (With<CurrentPlayerMarker>, Without<Camera>),
    >,
    (keyboard_input, ui_mode, key_map): (Res<ButtonInput<KeyCode>>, Res<UIMode>, Res<KeyMap>),
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut tick_buffer: ResMut<PlayerTickInputsBuffer>,
    world_map: Res<ClientWorldMap>,
    mut moves: MessageWriter<LocalPlayerMoved>,
    mut sprinting: Local<bool>,
) {
    if frame_inputs.0.delta_ms == 0 {
        return;
    }
    // The player has not been spawned yet
    let Ok((mut player, mut player_transform)) = player_query.single_mut() else {
        return;
    };

    // Gameplay keys are ignored while a dialog is open or something is being typed
    if *ui_mode == UIMode::Closed {
        if is_action_just_pressed(GameAction::ToggleFlyMode, &keyboard_input, &key_map) {
            frame_inputs.0.inputs.insert(NetworkAction::ToggleFlyMode);
        }
        for (key, action) in HELD_ACTIONS {
            if is_action_pressed(key, &keyboard_input, &key_map) {
                frame_inputs.0.inputs.insert(action);
            }
        }
    }
    let sprint_key = *ui_mode == UIMode::Closed
        && is_action_pressed(GameAction::Sprint, &keyboard_input, &key_map);
    *sprinting = keeps_sprinting(*sprinting, sprint_key, &frame_inputs.0, player.is_flying);
    if *sprinting {
        frame_inputs.0.inputs.insert(NetworkAction::Sprint);
    }

    let from = player.movement_state();
    simulate_player_movement(&mut player, &world_map.blocks, &frame_inputs.0);

    // The frame is final: record what it predicts and queue it for the server, which takes its
    // inventory actions along
    frame_inputs.0.predicted = player.movement_state();
    moves.write(LocalPlayerMoved {
        from,
        to: frame_inputs.0.predicted,
    });
    tick_buffer.buffer.push(frame_inputs.0.clone());
    frame_inputs.0.inventory_actions.clear();

    player_transform.translation = player.position;
}

/// Colour of the local player's body: red in third person, invisible in first person so that
/// it does not block the view.
pub const LOCAL_PLAYER_COLOR: Color = Color::srgba(1.0, 0.0, 0.0, 1.0);
const HIDDEN_PLAYER_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);

pub fn first_and_third_person_view_system(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut view_mode: ResMut<ViewMode>,
    player_query: Query<&PlayerMaterialHandle, With<CurrentPlayerMarker>>,
    key_map: Res<KeyMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
) {
    if *ui_mode == UIMode::Closed
        && is_action_just_pressed(GameAction::ToggleViewMode, &keyboard_input, &key_map)
    {
        view_mode.toggle();
    }

    // The player has not been spawned yet
    let Ok(material_handle) = player_query.single() else {
        return;
    };
    let color = match *view_mode {
        ViewMode::FirstPerson => HIDDEN_PLAYER_COLOR,
        ViewMode::ThirdPerson => LOCAL_PLAYER_COLOR,
    };
    // Reaching the material mutably marks it changed, and the renderer then prepares it again:
    // only when its colour has to change
    let current = materials.get(&material_handle.handle).map(|m| m.base_color);
    if current.is_some_and(|current| current != color) {
        if let Some(mut material) = materials.get_mut(&material_handle.handle) {
            material.base_color = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(actions: &[NetworkAction]) -> PlayerFrameInput {
        PlayerFrameInput {
            inputs: actions.iter().cloned().collect(),
            ..default()
        }
    }

    #[test]
    fn the_sprint_key_starts_a_sprint_that_lasts_while_moving_forward() {
        let forward = inputs(&[NetworkAction::MoveForward]);
        assert!(!keeps_sprinting(false, false, &forward, false));
        assert!(keeps_sprinting(false, true, &forward, false));
        // The key may be released
        assert!(keeps_sprinting(true, false, &forward, false));
        // Without moving forward, the key does nothing and the sprint stops
        assert!(!keeps_sprinting(false, true, &inputs(&[]), false));
        assert!(!keeps_sprinting(true, false, &inputs(&[]), false));
    }

    #[test]
    fn sneaking_stops_a_sprint_on_foot_but_not_in_flight() {
        let sneak = inputs(&[NetworkAction::MoveForward, NetworkAction::SneakOrFlyDown]);
        assert!(!keeps_sprinting(true, true, &sneak, false));
        assert!(keeps_sprinting(true, true, &sneak, true));
    }
}
