use bevy::{camera::visibility::RenderLayers, prelude::*};
use bevy_inspector_egui::bevy_egui::PrimaryEguiContext;

use super::{
    apply_fullscreen_system, camera_control_system, draw_the_eye_through_portals,
    give_the_eye_back, setup_tone_curve, underwater_fog_system, zoom_system, EyeThroughPortal,
    ViewSettings, Zoom, SKY_COLOR,
};

use crate::portal::{SURFACE_LAYER, VIEW_MODEL_LAYER};
use crate::state::{GameSet, GameState};
use crate::world::precipitation::render::PrecipitationView;
use bevy::render::render_resource::TextureUsages;

#[derive(Component)]
pub struct CameraController {
    pub distance: f32,
    pub angle_x: f32,
    pub angle_y: f32,
}

const DEFAULT_DISTANCE: f32 = 10.0;

/// How far from the camera its near plane is. The eye is only 0.1 below the top of the
/// player's box (`EYE_HEIGHT`), against a ceiling when the head touches it: Bevy's 0.1 would put
/// the corners of the plane up to 0.2 above the eye, into the block above, and everything behind
/// that block would be cut away, the world seen through.
const NEAR: f32 = 0.04;

impl Default for CameraController {
    fn default() -> Self {
        Self {
            distance: DEFAULT_DISTANCE,
            angle_x: 0.0,
            angle_y: 20.0f32.to_radians(),
        }
    }
}

impl From<Quat> for CameraController {
    fn from(quat: Quat) -> Self {
        Self {
            distance: DEFAULT_DISTANCE,
            angle_x: quat.to_euler(EulerRot::XYZ).0,
            angle_y: quat.to_euler(EulerRot::XYZ).1,
        }
    }
}

/// The camera is created when the connection starts, because the loading screen positions it
/// from the saved player, and lives until the game is left (`DespawnOnExit`). Cancelling the
/// loading screen goes straight back to the menu without ever entering the game, so the
/// camera has to be despawned on that transition too.
pub fn camera_plugin(app: &mut App) {
    app.init_resource::<Zoom>()
        .init_resource::<ViewSettings>()
        .add_systems(Update, apply_fullscreen_system)
        .add_systems(Startup, setup_tone_curve)
        .add_systems(OnEnter(GameState::PreGameLoading), spawn_camera)
        .add_systems(
            Update,
            zoom_system
                .in_set(GameSet::Input)
                .run_if(in_state(GameState::Game)),
        )
        .init_resource::<EyeThroughPortal>()
        .add_systems(First, give_the_eye_back)
        .add_systems(
            Update,
            (
                camera_control_system,
                draw_the_eye_through_portals,
                underwater_fog_system,
            )
                .chain()
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

fn spawn_camera(mut commands: Commands, settings: Res<ViewSettings>) {
    commands.spawn((
        Camera3d {
            // Read by the pass of the rain and the snow, which draws after the TAA
            depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING)
                .into(),
            ..default()
        },
        PrecipitationView,
        Camera {
            clear_color: ClearColorConfig::Custom(SKY_COLOR),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: settings.fov_radians(),
            near: NEAR,
            near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -NEAR),
            ..default()
        }),
        // The world, the view through the portals and what the player holds
        RenderLayers::from_layers(&[0, SURFACE_LAYER, VIEW_MODEL_LAYER]),
        Transform::from_translation(Vec3::new(0.0, 5.0, 10.0))
            .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        CameraController::default(),
        PrimaryEguiContext,
        // The ears of the spatial sounds, one block apart
        SpatialListener::new(1.0),
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
            // What the camera systems read once the game state is reached
            .add_message::<bevy::input::mouse::MouseMotion>()
            .insert_resource(shared::players::ViewMode::FirstPerson)
            .insert_resource(crate::ui::hud::UIMode::Closed)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(crate::input::keyboard::default_bindings())
            .init_resource::<crate::world::ClientWorldMap>()
            .init_resource::<crate::world::RenderDistance>()
            .insert_resource(crate::world::time::ClientTime(0))
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
    fn the_near_plane_of_the_eye_stays_within_the_box_of_the_player() {
        use crate::camera::MAX_FOV_DEGREES;
        use crate::player::SPRINT_FOV_SCALE;
        use shared::players::constants::EYE_HEIGHT;
        use shared::players::{PLAYER_HEIGHT, PLAYER_WIDTH};

        let mut app = app();
        go_to(&mut app, GameState::PreGameLoading);
        let Projection::Perspective(projection) = app
            .world_mut()
            .query_filtered::<&Projection, With<CameraController>>()
            .single(app.world())
            .unwrap()
            .clone()
        else {
            panic!("a perspective");
        };
        assert_eq!(
            projection.near_clip_plane,
            Vec4::new(0.0, 0.0, -1.0, -projection.near)
        );
        // Nothing solid is ever in the box of the player: the near plane of its eye, at the widest
        // field of view while sprinting and on a 32:9 screen, looking anywhere, must stay in it,
        // or it cuts the block above a head against a ceiling and the world is seen through
        let half_height =
            projection.near * (MAX_FOV_DEGREES.to_radians() * SPRINT_FOV_SCALE / 2.0).tan();
        let half_width = half_height * 32.0 / 9.0;
        let (room_above, room_below) = (
            PLAYER_HEIGHT / 2.0 - EYE_HEIGHT,
            PLAYER_HEIGHT / 2.0 + EYE_HEIGHT,
        );
        for pitch in -89..=89 {
            for yaw in (0..360).step_by(15) {
                let eye = Quat::from_rotation_y((yaw as f32).to_radians())
                    * Quat::from_rotation_x((pitch as f32).to_radians());
                for (x, y) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                    let corner = eye * Vec3::new(x * half_width, y * half_height, -projection.near);
                    assert!(
                        corner.y < room_above
                            && -corner.y < room_below
                            && corner.x.abs() < PLAYER_WIDTH / 2.0
                            && corner.z.abs() < PLAYER_WIDTH / 2.0,
                        "pitch {pitch}, yaw {yaw}: corner {corner} out of the box"
                    );
                }
            }
        }
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
