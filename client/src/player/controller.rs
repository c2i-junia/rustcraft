use crate::camera::CameraController;
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

use super::{CurrentPlayerMarker, PlayerBody};
use crate::portal::BODY_LAYER;
use bevy::camera::visibility::RenderLayers;

pub fn update_frame_inputs_system(
    camera: Query<&Transform, With<CameraController>>,
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
    /// Whether the player went through a portal on the way.
    pub through_portal: bool,
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
/// player and queues the frame for the server. A player that went through a portal has its
/// camera turned with it, here only: a replay of the reconciliation does not turn it again.
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
    mut cameras: Query<&mut CameraController>,
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
    let outcome = simulate_player_movement(&mut player, &world_map.blocks, &frame_inputs.0);
    if let Some(turned) = outcome.turned {
        // `camera_control_system` places the camera from its yaw later in the frame
        for mut camera in &mut cameras {
            camera.angle_x += turned;
        }
    }

    // The frame is final: record what it predicts and queue it for the server, which takes its
    // inventory actions along
    frame_inputs.0.predicted = player.movement_state();
    moves.write(LocalPlayerMoved {
        from,
        to: frame_inputs.0.predicted,
        through_portal: outcome.turned.is_some(),
    });
    tick_buffer.buffer.push(frame_inputs.0.clone());
    frame_inputs.0.inventory_actions.clear();

    player_transform.translation = player.position;
}

/// Colour of the local player's body.
pub const LOCAL_PLAYER_COLOR: Color = Color::srgba(1.0, 0.0, 0.0, 1.0);

/// Switches between first and third person with the view mode key. In first person the body of
/// the local player is drawn by the portal cameras only (`BODY_LAYER`): the player sees itself
/// through a portal, but its camera, inside the body, does not draw it. In third person the
/// player's camera draws it too.
pub fn first_and_third_person_view_system(
    mut view_mode: ResMut<ViewMode>,
    players: Query<&Children, With<CurrentPlayerMarker>>,
    mut bodies: Query<(Entity, Option<&RenderLayers>), With<PlayerBody>>,
    key_map: Res<KeyMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    mut commands: Commands,
) {
    if *ui_mode == UIMode::Closed
        && is_action_just_pressed(GameAction::ToggleViewMode, &keyboard_input, &key_map)
    {
        view_mode.toggle();
    }

    let layers = match *view_mode {
        ViewMode::FirstPerson => RenderLayers::layer(BODY_LAYER),
        ViewMode::ThirdPerson => RenderLayers::layer(0),
    };
    for children in &players {
        let mut bodies = bodies.iter_many_mut(children);
        while let Some((body, current)) = bodies.fetch_next() {
            if current != Some(&layers) {
                commands.entity(body).insert(layers.clone());
            }
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

    /// The layers and the colour of the local player's body, after a frame in `view`, with the
    /// view mode key pressed if `toggle`.
    fn body_in(view: ViewMode, toggle: bool) -> (Option<RenderLayers>, Color) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .insert_resource(view)
            .insert_resource(UIMode::Closed)
            .insert_resource(crate::input::keyboard::default_bindings())
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, first_and_third_person_view_system);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(LOCAL_PLAYER_COLOR);
        let body = app
            .world_mut()
            .spawn((PlayerBody, MeshMaterial3d(material.clone())))
            .id();
        app.world_mut()
            .spawn(CurrentPlayerMarker {})
            .add_child(body);
        if toggle {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::F5);
        }
        app.update();
        let layers = app.world().get::<RenderLayers>(body).cloned();
        let color = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color;
        (layers, color)
    }

    #[test]
    fn in_first_person_the_body_is_seen_through_the_portals_only() {
        let (layers, color) = body_in(ViewMode::FirstPerson, false);
        assert_eq!(layers, Some(RenderLayers::layer(BODY_LAYER)));
        assert_eq!(color, LOCAL_PLAYER_COLOR);
        // The player's camera does not draw that layer
        assert!(
            !RenderLayers::from_layers(&[0, crate::portal::SURFACE_LAYER])
                .intersects(&RenderLayers::layer(BODY_LAYER))
        );
    }

    #[test]
    fn in_third_person_the_body_is_seen_by_the_players_camera() {
        let (layers, color) = body_in(ViewMode::ThirdPerson, false);
        assert_eq!(layers, Some(RenderLayers::layer(0)));
        assert_eq!(color, LOCAL_PLAYER_COLOR);
        // Back to first person with the key
        let (layers, _) = body_in(ViewMode::ThirdPerson, true);
        assert_eq!(layers, Some(RenderLayers::layer(BODY_LAYER)));
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
