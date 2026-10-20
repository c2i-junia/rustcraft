//! Survival in the game, through the integrated server: the hearts, a deadly fall, the death
//! screen and the respawn. And the stances: sprinting widens the view, sneaking lowers the eyes.
//! And the zoom, which narrows the view and slows the mouse.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use shared::players::game_mode::{GameMode, MAX_HEALTH};
use shared::players::Player;

use super::game::enter_world;
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::{CameraController, FOV_DEGREES, ZOOM_SCALE};
use crate::player::{CurrentPlayerMarker, PlayerBody, SNEAK_BODY_SCALE};
use crate::ui::hud::health::{health_label, HealthBar};
use crate::ui::hud::HudBottom;
use crate::ui::menus::death::{DeathScreen, DEATH_TITLE, RESPAWN};

/// The local player, as the client predicts it.
fn player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

impl Harness {
    /// Runs `/gamemode <mode>` from the chat, and waits for the client to be told the mode.
    pub(super) fn set_game_mode(&mut self, mode: GameMode) {
        self.press(KeyCode::KeyT);
        self.type_text(&format!("/gamemode {}", mode.name()));
        self.press(KeyCode::Enter);
        self.wait_until("the new game mode", SERVER, |w| player(w).game_mode == mode);
    }

    /// The label the hearts give the accessibility tree.
    fn health_label(&mut self) -> String {
        let bar = self.get(&L::marker::<HealthBar>());
        self.world()
            .get::<AccessibleLabel>(bar)
            .map(|label| label.0.clone())
            .unwrap_or_default()
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_deadly_fall_opens_the_death_screen_and_respawn_brings_the_player_back() {
    let mut h = enter_world(GameFolder::new().with_world("Fall"), "Fall");
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    h.frames(10);
    let spawn = player(h.world()).position;

    // A new player is in survival, in full health
    h.wait_for(&L::marker::<HealthBar>(), SCREEN);
    assert_eq!(h.health_label(), health_label(MAX_HEALTH));
    h.snapshot("hearts");

    // F does not fly in survival
    h.press(KeyCode::KeyF);
    h.frames(5);
    assert!(!player(h.world()).is_flying);

    // In creative, the hearts hide and the player flies 30 blocks up
    h.set_game_mode(GameMode::Creative);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<HealthBar>()));
    h.press(KeyCode::KeyF);
    h.wait_until("the flight", SCREEN, |w| player(w).is_flying);
    h.key_down(KeyCode::Space);
    h.wait_until("30 blocks up", SERVER, |w| {
        player(w).position.y > spawn.y + 30.0
    });
    h.key_up(KeyCode::Space);

    // Back in survival, it falls to its death
    h.set_game_mode(GameMode::Survival);
    h.wait_for(&L::text(DEATH_TITLE), SERVER);
    assert!(h.is_visible(&L::text("You hit the ground too hard")));
    assert!(player(h.world()).is_dead());
    assert!(!h.is_visible(&L::marker::<HudBottom>()), "the hotbar shows");
    h.frames(10);
    h.snapshot("death_screen");

    // Respawn: the screen goes once the server brings the player back to the spawn
    h.click(&L::text(RESPAWN));
    h.wait_until("the respawn", SERVER, |w| !player(w).is_dead());
    h.frames(5);
    assert!(h.find(&L::marker::<DeathScreen>()).is_empty());
    assert!(h.is_visible(&L::marker::<HudBottom>()));
    assert_eq!(h.health_label(), health_label(MAX_HEALTH));
    let back = player(h.world()).position;
    assert!(
        back.distance(spawn) < 0.5,
        "respawned at {back}, spawned at {spawn}"
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn sprinting_widens_the_view_and_sneaking_lowers_the_eyes() {
    let mut h = enter_world(GameFolder::new().with_world("Stance"), "Stance");
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    let fov = |w: &World| {
        let mut projections = w
            .try_query_filtered::<&Projection, With<Camera3d>>()
            .unwrap();
        match projections.single(w).unwrap() {
            Projection::Perspective(perspective) => perspective.fov,
            _ => unreachable!(),
        }
    };
    let eyes = |w: &World| {
        let mut cameras = w
            .try_query_filtered::<&Transform, With<Camera3d>>()
            .unwrap();
        let camera = cameras.single(w).unwrap().translation.y;
        camera - player(w).position.y
    };
    let body_scale = |w: &World| {
        let mut bodies = w
            .try_query_filtered::<&Transform, With<PlayerBody>>()
            .unwrap();
        bodies.single(w).unwrap().scale.y
    };
    assert!((fov(h.world()) - FOV_DEGREES.to_radians()).abs() < 1e-4);
    let standing_eyes = eyes(h.world());

    h.key_down(KeyCode::KeyW);
    h.key_down(KeyCode::ControlLeft);
    h.wait_until("the sprint", SCREEN, |w| player(w).is_sprinting);
    // The sprint lasts without its key while the player goes forward
    h.key_up(KeyCode::ControlLeft);
    h.wait_until("a wider view", SCREEN, |w| {
        fov(w) > FOV_DEGREES.to_radians() * 1.05
    });
    assert!(player(h.world()).is_sprinting);
    h.key_up(KeyCode::KeyW);
    h.wait_until("the end of the sprint", SCREEN, |w| !player(w).is_sprinting);
    h.wait_until("the view back", SCREEN, |w| {
        fov(w) == FOV_DEGREES.to_radians()
    });

    h.key_down(KeyCode::ShiftLeft);
    h.wait_until("the sneak", SCREEN, |w| player(w).is_sneaking);
    h.wait_until("a crouched body", SCREEN, |w| {
        body_scale(w) == SNEAK_BODY_SCALE
    });
    h.frames(1);
    let sneaking_eyes = eyes(h.world());
    assert!(
        sneaking_eyes < standing_eyes - 0.2,
        "eyes at {sneaking_eyes}, {standing_eyes} standing"
    );
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("a standing body", SCREEN, |w| body_scale(w) == 1.0);
    h.frames(1);
    assert!((eyes(h.world()) - standing_eyes).abs() < 0.05);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn holding_c_zooms_and_slows_the_mouse() {
    let mut h = enter_world(GameFolder::new().with_world("Zoom"), "Zoom");
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    let fov = |w: &World| {
        let mut projections = w
            .try_query_filtered::<&Projection, With<Camera3d>>()
            .unwrap();
        match projections.single(w).unwrap() {
            Projection::Perspective(perspective) => perspective.fov,
            _ => unreachable!(),
        }
    };
    // How far a move of the mouse by 100 pixels turns the camera
    let turn = |h: &mut Harness| {
        let yaw = |w: &mut World| w.query::<&CameraController>().single(w).unwrap().angle_x;
        let before = yaw(h.world_mut());
        h.world_mut().write_message(MouseMotion {
            delta: Vec2::new(100.0, 0.0),
        });
        h.frames(1);
        (yaw(h.world_mut()) - before).abs()
    };
    let normal_turn = turn(&mut h);
    assert!(normal_turn > 0.0);

    h.key_down(KeyCode::KeyC);
    h.wait_until("the zoom", SCREEN, |w| {
        fov(w) == FOV_DEGREES.to_radians() * ZOOM_SCALE
    });
    let zoomed_turn = turn(&mut h);
    assert!(
        (zoomed_turn - normal_turn * ZOOM_SCALE).abs() < 1e-5,
        "{zoomed_turn} zoomed, {normal_turn} not"
    );

    h.key_up(KeyCode::KeyC);
    h.wait_until("the view back", SCREEN, |w| {
        fov(w) == FOV_DEGREES.to_radians()
    });
}
