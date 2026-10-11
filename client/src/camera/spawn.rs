use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::PrimaryEguiContext;

use super::camera_control_system;

use crate::state::{GameSet, GameState};

#[derive(Component)]
pub struct CameraController {
    pub distance: f32,
    pub angle_x: f32,
    pub angle_y: f32,
    pub mouse_sensitivity: f32,
}

const DEFAULT_DISTANCE: f32 = 10.0;
const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.003;

impl Default for CameraController {
    fn default() -> Self {
        Self {
            distance: DEFAULT_DISTANCE,
            angle_x: 0.0,
            angle_y: 20.0f32.to_radians(),
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
        }
    }
}

impl From<Quat> for CameraController {
    fn from(quat: Quat) -> Self {
        Self {
            distance: DEFAULT_DISTANCE,
            angle_x: quat.to_euler(EulerRot::XYZ).0,
            angle_y: quat.to_euler(EulerRot::XYZ).1,
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
        }
    }
}

/// The camera is created when the connection starts, because the loading screen positions it
/// from the saved player, and lives until the game is left (`DespawnOnExit`). Cancelling the
/// loading screen goes straight back to the menu without ever entering the game, so the
/// camera has to be despawned on that transition too.
pub fn camera_plugin(app: &mut App) {
    app.add_systems(OnEnter(GameState::PreGameLoading), spawn_camera)
        .add_systems(
            Update,
            camera_control_system
                .in_set(GameSet::Camera)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            OnTransition {
                exited: GameState::PreGameLoading,
                entered: GameState::Menu,
            },
            despawn_camera,
        );
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(117, 159, 183)),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: f32::to_radians(60.0),
            ..Default::default()
        }),
        Transform::from_translation(Vec3::new(0.0, 5.0, 10.0))
            .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        CameraController::default(),
        PrimaryEguiContext,
        DespawnOnExit(GameState::Game),
    ));
}

fn despawn_camera(mut commands: Commands, cameras: Query<Entity, With<CameraController>>) {
    for camera in &cameras {
        commands.entity(camera).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            // What `camera_control_system` reads once the game state is reached
            .add_message::<bevy::input::mouse::MouseMotion>()
            .insert_resource(shared::players::ViewMode::FirstPerson)
            .insert_resource(crate::ui::hud::UIMode::Closed)
            .add_plugins(camera_plugin);
        app.update();
        app
    }

    fn go_to(app: &mut App, state: GameState) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(state);
        app.update();
    }

    fn cameras(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<CameraController>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn cancelling_the_connection_and_retrying_leaves_one_camera() {
        let mut app = app();
        go_to(&mut app, GameState::PreGameLoading);
        assert_eq!(cameras(&mut app), 1);

        go_to(&mut app, GameState::Menu);
        assert_eq!(cameras(&mut app), 0);

        go_to(&mut app, GameState::PreGameLoading);
        go_to(&mut app, GameState::Game);
        assert_eq!(cameras(&mut app), 1);
    }

    #[test]
    fn leaving_the_game_removes_the_camera() {
        let mut app = app();
        go_to(&mut app, GameState::PreGameLoading);
        go_to(&mut app, GameState::Game);
        go_to(&mut app, GameState::Menu);
        assert_eq!(cameras(&mut app), 0);
    }
}
