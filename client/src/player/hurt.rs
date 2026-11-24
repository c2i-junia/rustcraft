//! What the local player feels when it is hurt: a red vignette at the edges of the screen and a
//! tilt of the camera, both fading out over `HURT_SECONDS`, the hurt sound (`audio`) and the
//! hearts blinking (`ui/hud/health.rs`). The server hurts, the client only sees its health drop.

use bevy::prelude::*;
use shared::players::Player;

use super::CurrentPlayerMarker;
use crate::state::GameState;

/// How long the feedback of a hurt lasts, in seconds.
pub const HURT_SECONDS: f32 = 0.5;

/// The alpha of the red at the edges of the screen, right after a hurt.
const VIGNETTE_ALPHA: f32 = 0.6;

/// The red of the vignette.
const VIGNETTE_RED: Color = Color::srgb(0.75, 0.0, 0.0);

/// How far the camera rolls right after a hurt.
const TILT_DEGREES: f32 = 8.0;

/// How long the hearts stay hidden, then shown, while they blink, in seconds.
const BLINK_SECONDS: f32 = 0.1;

/// The health of the local player dropped.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalPlayerHurt;

/// The time since the local player was last hurt, while the feedback lasts.
#[derive(Resource, Default, Debug)]
pub struct HurtFeedback {
    pub elapsed: Option<f32>,
}

impl HurtFeedback {
    /// From 1, right after a hurt, down to 0 at `HURT_SECONDS`.
    pub fn strength(&self) -> f32 {
        self.elapsed
            .map_or(0.0, |elapsed| (1.0 - elapsed / HURT_SECONDS).max(0.0))
    }

    /// Whether the blinking hearts are hidden at this moment: in the first `BLINK_SECONDS`
    /// after a hurt, and every other `BLINK_SECONDS` after it.
    pub fn hearts_hidden(&self) -> bool {
        self.elapsed.is_some_and(|elapsed| {
            elapsed < HURT_SECONDS && ((elapsed / BLINK_SECONDS) as u32).is_multiple_of(2)
        })
    }
}

/// The red at the edges of the screen.
#[derive(Component)]
pub struct HurtVignette;

/// The roll added to the camera for the frame being drawn, taken off at the start of the next.
#[derive(Resource, Default)]
struct CameraTilt(f32);

pub fn hurt_plugin(app: &mut App) {
    app.add_message::<LocalPlayerHurt>()
        .init_resource::<HurtFeedback>()
        .init_resource::<CameraTilt>()
        .add_systems(OnEnter(GameState::Game), spawn_hurt_vignette)
        .add_systems(OnExit(GameState::Game), reset_hurt_feedback)
        .add_systems(First, untilt_camera)
        .add_systems(
            PostUpdate,
            tilt_camera
                .before(TransformSystems::Propagate)
                .run_if(in_state(GameState::Game)),
        );
}

/// Writes `LocalPlayerHurt` when the health of the local player drops from one frame to the
/// next. A respawn raises it, and a player spawned anew is not compared with the one before.
pub fn detect_local_hurt(
    player: Query<(Entity, &Player), With<CurrentPlayerMarker>>,
    mut last: Local<Option<(Entity, u32)>>,
    mut hurt: MessageWriter<LocalPlayerHurt>,
) {
    let Ok((entity, player)) = player.single() else {
        return;
    };
    if last.is_some_and(|(was, health)| was == entity && player.health < health) {
        hurt.write(LocalPlayerHurt);
    }
    *last = Some((entity, player.health));
}

/// Starts the feedback at each hurt and lets it fade out.
pub fn advance_hurt_feedback(
    mut hurts: MessageReader<LocalPlayerHurt>,
    mut feedback: ResMut<HurtFeedback>,
    time: Res<Time>,
) {
    if let Some(elapsed) = &mut feedback.elapsed {
        *elapsed += time.delta_secs();
        if *elapsed >= HURT_SECONDS {
            feedback.elapsed = None;
        }
    }
    if hurts.read().count() > 0 {
        feedback.elapsed = Some(0.0);
    }
}

/// The colours of the vignette for a feedback of `strength`: clear in the middle, red at the
/// edges.
fn vignette_gradient(strength: f32) -> BackgroundGradient {
    BackgroundGradient::from(RadialGradient {
        stops: vec![
            ColorStop::percent(Color::NONE, 50),
            ColorStop::percent(VIGNETTE_RED.with_alpha(VIGNETTE_ALPHA * strength), 100),
        ],
        ..default()
    })
}

fn spawn_hurt_vignette(mut commands: Commands) {
    commands.spawn((
        HurtVignette,
        Name::new("Hurt vignette"),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        vignette_gradient(0.0),
        Visibility::Hidden,
        Pickable::IGNORE,
        DespawnOnExit(GameState::Game),
    ));
}

/// Fades the vignette with the feedback, and hides it once it is over.
pub fn update_hurt_vignette(
    feedback: Res<HurtFeedback>,
    mut vignette: Query<(&mut BackgroundGradient, &mut Visibility), With<HurtVignette>>,
) {
    let Ok((mut gradient, mut visibility)) = vignette.single_mut() else {
        return;
    };
    let strength = feedback.strength();
    if strength == 0.0 {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    *gradient = vignette_gradient(strength);
    *visibility = Visibility::Inherited;
}

/// Rolls the camera with the feedback, for the frame being drawn only: the inputs and the aim
/// read the camera before it.
fn tilt_camera(
    feedback: Res<HurtFeedback>,
    mut tilt: ResMut<CameraTilt>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let roll = TILT_DEGREES.to_radians() * feedback.strength();
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    if roll != 0.0 {
        transform.rotation *= Quat::from_rotation_z(roll);
    }
    tilt.0 = roll;
}

/// Takes off the roll `tilt_camera` added for the last frame.
fn untilt_camera(mut tilt: ResMut<CameraTilt>, mut camera: Query<&mut Transform, With<Camera3d>>) {
    if tilt.0 == 0.0 {
        return;
    }
    if let Ok(mut transform) = camera.single_mut() {
        transform.rotation *= Quat::from_rotation_z(-tilt.0);
    }
    tilt.0 = 0.0;
}

fn reset_hurt_feedback(mut feedback: ResMut<HurtFeedback>, mut tilt: ResMut<CameraTilt>) {
    *feedback = HurtFeedback::default();
    tilt.0 = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn app_with_player(health: u32) -> (App, Entity) {
        let mut app = App::new();
        app.add_message::<LocalPlayerHurt>()
            .add_systems(Update, detect_local_hurt);
        let player = app
            .world_mut()
            .spawn((
                Player {
                    health,
                    ..default()
                },
                CurrentPlayerMarker {},
            ))
            .id();
        (app, player)
    }

    fn hurts(app: &mut App) -> usize {
        app.world_mut()
            .resource_mut::<Messages<LocalPlayerHurt>>()
            .drain()
            .count()
    }

    fn set_health(app: &mut App, player: Entity, health: u32) {
        app.world_mut().get_mut::<Player>(player).unwrap().health = health;
    }

    #[test]
    fn a_drop_of_health_is_a_hurt_and_a_rise_is_not() {
        let (mut app, player) = app_with_player(20);
        app.update();
        assert_eq!(
            hurts(&mut app),
            0,
            "the first frame has nothing to compare with"
        );
        set_health(&mut app, player, 15);
        app.update();
        assert_eq!(hurts(&mut app), 1);
        app.update();
        assert_eq!(hurts(&mut app), 0);
        // Dead, then respawned
        set_health(&mut app, player, 0);
        app.update();
        assert_eq!(hurts(&mut app), 1);
        set_health(&mut app, player, 20);
        app.update();
        assert_eq!(hurts(&mut app), 0);
    }

    #[test]
    fn a_new_local_player_is_not_compared_with_the_last_one() {
        let (mut app, player) = app_with_player(20);
        app.update();
        app.world_mut().despawn(player);
        app.world_mut().spawn((
            Player {
                health: 4,
                ..default()
            },
            CurrentPlayerMarker {},
        ));
        app.update();
        assert_eq!(hurts(&mut app), 0);
    }

    #[test]
    fn the_feedback_fades_out_over_half_a_second() {
        let mut app = App::new();
        app.add_message::<LocalPlayerHurt>()
            .init_resource::<HurtFeedback>()
            .init_resource::<Time>()
            .add_systems(Update, advance_hurt_feedback);
        let feedback = |app: &App| {
            let feedback = app.world().resource::<HurtFeedback>();
            (feedback.strength(), feedback.hearts_hidden())
        };
        let advance = |app: &mut App, seconds: f32| {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(seconds));
            app.update();
        };

        advance(&mut app, 0.0);
        assert_eq!(feedback(&app), (0.0, false));
        app.world_mut().write_message(LocalPlayerHurt);
        advance(&mut app, 0.0);
        assert_eq!(feedback(&app), (1.0, true));
        advance(&mut app, 0.15);
        let (strength, hidden) = feedback(&app);
        assert!((strength - 0.7).abs() < 1e-4, "{strength}");
        assert!(!hidden, "the hearts blink back");
        advance(&mut app, 0.1);
        assert!(feedback(&app).1, "and hide again");
        advance(&mut app, 0.3);
        assert_eq!(feedback(&app), (0.0, false));
        assert_eq!(app.world().resource::<HurtFeedback>().elapsed, None);
    }
}
