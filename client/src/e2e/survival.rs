//! Survival in the game, through the integrated server: the hearts, a deadly fall, the death
//! screen and the respawn. And the stances: sprinting widens the view, sneaking lowers the eyes.
//! And the zoom, which narrows the view and slows the mouse. And what drowns and burns: water,
//! fire and lava.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use shared::players::air::MAX_AIR;
use shared::players::game_mode::{GameMode, MAX_HEALTH};
use shared::players::{Player, PLAYER_HEIGHT};
use shared::world::{BlockId, ItemId, SEA_LEVEL};
use shared::CHUNK_SIZE;

use super::game::{enter_world, hold_click, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::{CameraController, FOV_DEGREES, LAVA_FOG_COLOR, ZOOM_SCALE};
use crate::player::{CurrentPlayerMarker, HurtVignette, PlayerBody, SNEAK_BODY_SCALE};
use crate::ui::hud::air::{air_label, AirBar};
use crate::ui::hud::burning::BurningFlames;
use crate::ui::hud::health::{health_label, HealthBar};
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{HudBottom, InventoryRoot};
use crate::ui::menus::death::{DeathScreen, DEATH_TITLE, RESPAWN};
use server::WorldGenerator;

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

    /// The label the bubbles give the accessibility tree.
    fn air_label(&mut self) -> String {
        let bar = self.get(&L::marker::<AirBar>());
        self.world()
            .get::<AccessibleLabel>(bar)
            .map(|label| label.0.clone())
            .unwrap_or_default()
    }
}

/// Where a player stands at the bottom of water 4 blocks deep or more, in the world of the
/// seed 42, near its spawn: the columns around are as deep, so it sinks straight down.
fn lake_bottom() -> Vec3 {
    let generator = WorldGenerator::new(42);
    let spawn = generator.spawn_position().as_ivec3();
    let deep = |x, z| generator.terrain().column_shape(x, z).height <= SEA_LEVEL - 4;
    let (x, z) = (1..64)
        .flat_map(|ring: i32| {
            (-ring..=ring).flat_map(move |i| [(i, -ring), (i, ring), (-ring, i), (ring, i)])
        })
        .map(|(x, z)| (spawn.x + x * 4, spawn.z + z * 4))
        .find(|&(x, z)| (-1..=1).all(|dx| (-1..=1).all(|dz| deep(x + dx, z + dz))))
        .expect("a lake near the spawn");
    let ground = generator.terrain().column_shape(x, z).height;
    Vec3::new(
        x as f32 + 0.5,
        ground as f32 + 1.0 + PLAYER_HEIGHT / 2.0,
        z as f32 + 0.5,
    )
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
    // On land, with all its air, no bubble shows
    assert!(!h.is_visible(&L::marker::<AirBar>()));
    h.snapshot("hearts");

    // The open inventory hides them, as in Minecraft: they would cover its bottom row
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    h.frames(2);
    assert!(!h.is_visible(&L::marker::<HealthBar>()));
    h.snapshot("inventory_without_hearts");
    h.press(KeyCode::KeyE);
    h.frames(2);
    assert!(h.is_visible(&L::marker::<HealthBar>()));

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
fn a_fall_that_hurts_reddens_the_edges_of_the_screen_for_a_moment() {
    let mut h = enter_world(GameFolder::new().with_world("Hurt"), "Hurt");
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    let ground = player(h.world()).position.y;
    assert!(!h.is_visible(&L::marker::<HurtVignette>()));

    h.set_game_mode(GameMode::Creative);
    h.press(KeyCode::KeyF);
    h.wait_until("the flight", SCREEN, |w| player(w).is_flying);
    h.key_down(KeyCode::Space);
    h.wait_until("8 blocks up", SERVER, |w| {
        player(w).position.y > ground + 8.0
    });
    h.key_up(KeyCode::Space);

    // Back in survival, it falls and lands hurt
    h.set_game_mode(GameMode::Survival);
    h.wait_for(&L::marker::<HurtVignette>(), SERVER);
    let health = player(h.world()).health;
    assert!(health < MAX_HEALTH && health > 0, "health {health}");
    assert_eq!(h.health_label(), health_label(health));
    h.snapshot("hurt");
    h.wait_until("the vignette gone", SCREEN, |w| {
        w.get::<Visibility>(
            w.try_query_filtered::<Entity, With<HurtVignette>>()
                .unwrap()
                .single(w)
                .unwrap(),
        ) == Some(&Visibility::Hidden)
    });
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

/// Where a player stands at the bottom of a lava lake two blocks deep or more, at the bottom of
/// a cave of the world of the seed 42, near its spawn.
fn lava_lake_bottom() -> Vec3 {
    let generator = WorldGenerator::new(42);
    let centre = generator
        .spawn_position()
        .as_ivec3()
        .div_euclid(IVec3::splat(CHUNK_SIZE));
    let is_lava = |chunk: &shared::world::Chunk, local: IVec3| {
        chunk
            .get(local)
            .is_some_and(|block| block.id == BlockId::Lava)
    };
    for ring in 0..16 {
        for i in -ring..=ring {
            for (dx, dz) in [(i, -ring), (i, ring), (-ring, i), (ring, i)] {
                let chunk_pos = IVec3::new(centre.x + dx, 0, centre.z + dz);
                let chunk = generator.generate_chunk(chunk_pos);
                for (local, _) in chunk.iter() {
                    let deep = (0..2).all(|dy| is_lava(&chunk, local + IVec3::Y * dy));
                    let floor = local.y > 0
                        && chunk
                            .get(local - IVec3::Y)
                            .is_some_and(|block| block.id != BlockId::Lava);
                    if deep && floor {
                        let cell = chunk_pos * CHUNK_SIZE + local;
                        return Vec3::new(
                            cell.x as f32 + 0.5,
                            cell.y as f32 + PLAYER_HEIGHT / 2.0 + 0.01,
                            cell.z as f32 + 0.5,
                        );
                    }
                }
            }
        }
    }
    panic!("no lava lake near the spawn");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_player_in_lava_burns_loses_health_and_sees_an_orange_fog() {
    let folder =
        GameFolder::new()
            .with_world("Lava")
            .with_player_at("Lava", "Tester", lava_lake_bottom());
    let mut h = enter_world(folder, "Lava");
    h.wait_until("the player on fire", SERVER, |w| player(w).is_burning());
    h.wait_until("a hit of the lava", SERVER, |w| {
        player(w).health < MAX_HEALTH
    });
    h.wait_for(&L::marker::<BurningFlames>(), SCREEN);
    h.wait_until("the orange fog", SCREEN, |w| {
        let mut fogs = w.try_query::<&DistanceFog>().unwrap();
        fogs.iter(w).any(|fog| fog.color == LAVA_FOG_COLOR)
    });
    h.snapshot("lava");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_player_with_its_head_under_water_sees_its_bubbles_go() {
    let folder =
        GameFolder::new()
            .with_world("Lake")
            .with_player_at("Lake", "Tester", lake_bottom());
    let mut h = enter_world(folder, "Lake");
    h.wait_until("the air going", SERVER, |w| player(w).air < MAX_AIR);
    h.wait_for(&L::marker::<AirBar>(), SCREEN);
    h.wait_until("a bubble gone", SERVER, |w| player(w).air <= MAX_AIR - 30);
    h.frames(1);
    let air = player(h.world()).air;
    assert!(air > 0, "the air ran out already");
    assert_eq!(h.air_label(), air_label(air));
    assert_ne!(h.air_label(), air_label(MAX_AIR));
    h.snapshot("bubbles");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_fire_lit_at_the_feet_burns_the_player_who_sees_its_flames() {
    let folder = GameFolder::new().with_world("Fire").with_items(
        "Fire",
        "Tester",
        &[(0, ItemId::FlintAndSteel, 1)],
    );
    let mut h = enter_world(folder, "Fire");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::FlintAndSteel, 1))
    });
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    // Straight down, at the ground under the feet
    let world = h.world_mut();
    world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap()
        .angle_y = 1.5;
    h.frames(10);
    assert!(!h.is_visible(&L::marker::<BurningFlames>()));

    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_until("the player on fire", SERVER, |w| player(w).is_burning());
    h.wait_for(&L::marker::<BurningFlames>(), SCREEN);
    h.wait_until("a hit of the fire", SERVER, |w| {
        player(w).health < MAX_HEALTH
    });
    h.snapshot("burning");
}
