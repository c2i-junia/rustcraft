//! The animals: those that live in a new world, and those a spawn egg hatches. The
//! villagers, who trade. And the hostiles: drawn, and biting a survival player.

use std::time::{Duration, Instant};

use bevy::input::{mouse::MouseButtonInput, ButtonState};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowEvent};
use shared::players::game_mode::{GameMode, MAX_HEALTH};
use shared::players::Player;
use shared::world::{ItemId, MobAction, MobKind};

use super::game::{aimed_cells, enter_world, slot};
use super::harness::{Harness, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::mob::Mob;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::{InventoryRoot, OpenTrade};
use crate::ui::menus::death::DEATH_TITLE;
use crate::world::time::ClientTime;
use accesskit::Role;

/// The animals drawn, with where they are.
fn animals(w: &World) -> Vec<(MobKind, Vec3)> {
    let Some(mut mobs) = w.try_query::<(&Mob, &Transform)>() else {
        return Vec::new();
    };
    mobs.iter(w)
        .map(|(mob, transform)| (mob.kind, transform.translation))
        .collect()
}

fn player_position(w: &mut World) -> Vec3 {
    w.query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(w)
        .unwrap()
        .translation
}

/// Turns the camera of the player toward `target`, from the camera: the server casts the aim from
/// the eye, well above the centre of the player's box.
fn look_at(h: &mut Harness, target: Vec3) {
    let world = h.world_mut();
    let (mut controller, camera) = world
        .query::<(&mut CameraController, &Transform)>()
        .single_mut(world)
        .unwrap();
    let direction = (target - camera.translation).normalize();
    controller.angle_x = (-direction.x).atan2(-direction.z);
    controller.angle_y = (-direction.y).atan2(direction.xz().length());
}

/// Looks down at the ground ahead of the player, once it is within reach and the player settled
/// on the ground, where the player then is.
fn look_at_the_ground_ahead(h: &mut Harness) -> Vec3 {
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    let player_entity = world
        .query_filtered::<Entity, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap();
    world.get_mut::<CameraController>(camera).unwrap().angle_y = 0.9;
    h.wait_until("the ground within reach", SERVER, |w| {
        aimed_cells(w, camera, player_entity).is_some_and(|(target, _)| {
            let centre = w.get::<Transform>(player_entity).unwrap().translation;
            target.as_vec3().distance(centre) < 3.
        })
    });
    // Settled on the ground
    h.frames(30);
    player_position(h.world_mut())
}

/// The local player, as the client predicts it and the server corrects it.
fn local_player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

/// Clicks the right button over the hotbar, which the world reads as a use of the held item.
fn right_click(h: &mut Harness) {
    let center = h.center_of(&L::marker::<Hotbar>());
    h.move_cursor(center);
    h.frames(1);
    let window = h
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap();
    for (state, held) in [(ButtonState::Pressed, 3), (ButtonState::Released, 1)] {
        let input = MouseButtonInput {
            button: MouseButton::Right,
            state,
            window,
        };
        h.world_mut().write_message(input);
        h.world_mut()
            .write_message(WindowEvent::MouseButtonInput(input));
        h.frames(held);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn animals_live_in_a_new_world_and_stand_on_the_ground() {
    // A seed whose spawn has herds around it: that of 42 has none within two chunks
    let folder = GameFolder::new().with_seeded_world("Meadow", 12, 0, Duration::ZERO);
    let mut h = enter_world(folder, "Meadow");
    // Standing: the bottom of its hitbox on the top of a block. The animals in sight may walk
    // out of reach, so whichever stands is looked at.
    let standing = |w: &World| {
        animals(w).into_iter().find(|&(kind, at)| {
            let feet = at.y - kind.size().y / 2.0;
            (feet - feet.round()).abs() < 0.05
        })
    };
    h.wait_until("an animal standing on the ground", SERVER, |w| {
        standing(w).is_some()
    });
    let (_, animal) = standing(h.world()).unwrap();

    look_at(&mut h, animal);
    h.frames(10);
    h.snapshot("standing_animal");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_spawn_egg_used_on_the_ground_hatches_its_animal_there() {
    let folder = GameFolder::new().with_world("Eggs").with_items(
        "Eggs",
        "Tester",
        &[(0, ItemId::SheepSpawnEgg, 1)],
    );
    let mut h = enter_world(folder, "Eggs");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::SheepSpawnEgg, 1))
    });
    let player = look_at_the_ground_ahead(&mut h);

    right_click(&mut h);
    h.wait_until("the hatched sheep", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::Sheep && at.distance(player) < 5.0)
    });
    assert_eq!(slot(h.world(), 0), None);
    h.frames(60);
    h.snapshot("hatched_sheep");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_gnawer_hatched_next_to_a_survival_player_bites_it() {
    let folder = GameFolder::new().with_world("Rats").with_items(
        "Rats",
        "Tester",
        &[(0, ItemId::GnawerSpawnEgg, 1)],
    );
    let mut h = enter_world(folder, "Rats");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::GnawerSpawnEgg, 1))
    });
    assert_eq!(local_player(h.world()).game_mode, GameMode::Survival);
    let player = look_at_the_ground_ahead(&mut h);

    right_click(&mut h);
    h.wait_until("the hatched gnawer", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::Gnawer && at.distance(player) < 5.0)
    });
    h.wait_until("a bite", SERVER, |w| local_player(w).health < MAX_HEALTH);
    // Not the 2 points of a bite to the point: a client under load may see the health after
    // a bite and a point regained
    let health = local_player(h.world()).health;
    assert!(health > 0, "health {health}");
    h.snapshot("bitten");

    // It bites on to the death, 2 points every 15 ticks
    h.wait_for(&L::text(crate::i18n::tr(DEATH_TITLE)), SERVER);
    assert!(h.is_visible(&L::text("A Gnawer got you")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_stumpling_hatched_at_night_next_to_a_survival_player_hits_it() {
    // A world at its time 0 is at the start of the night: the Stumpling stays out
    let folder = GameFolder::new().with_world("Stumps").with_items(
        "Stumps",
        "Tester",
        &[(0, ItemId::StumplingSpawnEgg, 1)],
    );
    let mut h = enter_world(folder, "Stumps");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::StumplingSpawnEgg, 1))
    });
    assert_eq!(local_player(h.world()).game_mode, GameMode::Survival);
    let player = look_at_the_ground_ahead(&mut h);

    right_click(&mut h);
    h.wait_until("the hatched stumpling", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::Stumpling && at.distance(player) < 5.0)
    });
    let (_, stumpling) = animals(h.world())
        .into_iter()
        .find(|&(kind, _)| kind == MobKind::Stumpling)
        .unwrap();
    look_at(&mut h, stumpling);
    // The night brings hostiles of its own, which may strike first on a slow machine: the blow
    // counted is a loss of health while the Stumpling strikes and no other hostile does, the
    // strikes and the health being told in messages of their own, a few frames apart at most
    let strikers = |w: &mut World| {
        let kinds: Vec<MobKind> = w
            .query::<&Mob>()
            .iter(w)
            .filter(|mob| mob.action == MobAction::Attack)
            .map(|mob| mob.kind)
            .collect();
        (
            kinds.contains(&MobKind::Stumpling),
            kinds.iter().any(|kind| *kind != MobKind::Stumpling),
        )
    };
    let start = Instant::now();
    let (mut health, mut loss) = (MAX_HEALTH, None);
    let (mut stumpling_struck, mut other_struck): (Option<u32>, Option<u32>) = (None, None);
    let blow = (0u32..)
        .find_map(|frame| {
            assert!(start.elapsed() < SERVER, "timed out waiting for a blow");
            h.frames(1);
            let now = local_player(h.world()).health;
            if now < health {
                loss = Some((frame, health - now));
            }
            health = now;
            let (stumpling, other) = strikers(h.world_mut());
            stumpling_struck = stumpling.then_some(frame).or(stumpling_struck);
            other_struck = other.then_some(frame).or(other_struck);
            let (lost_at, lost) = loss?;
            let near = |struck: Option<u32>| struck.is_some_and(|at| at.abs_diff(lost_at) <= 5);
            (near(stumpling_struck) && !near(other_struck)).then_some(lost)
        })
        .unwrap();
    assert!(health > 0 && blow.is_multiple_of(3), "a blow of {blow}");
    h.snapshot("stumpling");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_drowner_hatched_in_a_lake_grabs_a_survival_player_and_drags_it_down() {
    let folder = GameFolder::new()
        .with_world("Lake")
        .with_player("Lake", "Tester", |player| {
            player.position = super::survival::lake_bottom();
            let item_id = ItemId::DrownerSpawnEgg;
            let stack = shared::world::ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb: 1,
            };
            player.inventory.inner.insert(0, stack);
        });
    let mut h = enter_world(folder, "Lake");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::DrownerSpawnEgg, 1))
    });
    let player = look_at_the_ground_ahead(&mut h);

    right_click(&mut h);
    h.wait_until("the hatched drowner", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::Drowner && at.distance(player) < 5.0)
    });
    // The client's prediction is told of the grab: the player is dragged down
    h.wait_until("a grab", SERVER, |w| {
        let player = local_player(w);
        player.health < MAX_HEALTH && player.dragged_ms > 0
    });
    // Not the 2 points of a grab to the point: a client under load may see the health after a
    // grab and a point regained
    let (_, drowner) = animals(h.world())
        .into_iter()
        .find(|&(kind, _)| kind == MobKind::Drowner)
        .unwrap();
    look_at(&mut h, drowner);
    h.frames(5);
    h.snapshot("grabbed");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_cinderling_hatched_next_to_a_survival_player_sets_it_burning_with_its_embers() {
    let folder = GameFolder::new().with_world("Embers").with_items(
        "Embers",
        "Tester",
        &[(0, ItemId::CinderlingSpawnEgg, 1)],
    );
    let mut h = enter_world(folder, "Embers");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::CinderlingSpawnEgg, 1))
    });
    assert_eq!(local_player(h.world()).game_mode, GameMode::Survival);
    let player = look_at_the_ground_ahead(&mut h);

    right_click(&mut h);
    h.wait_until("the hatched cinderling", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::Cinderling && at.distance(player) < 5.0)
    });
    // An ember in flight, once the Cinderling keeps its distance
    h.wait_until("an ember from afar", SERVER, |w| {
        let drawn = animals(w);
        let far = drawn
            .iter()
            .any(|&(kind, at)| kind == MobKind::Cinderling && at.distance(player) > 4.5);
        far && drawn.iter().any(|&(kind, _)| kind == MobKind::Ember)
    });
    // Where the wait saw it: a few frames later, on a slow machine, it may have hit already
    let (_, ember) = animals(h.world())
        .into_iter()
        .find(|&(kind, _)| kind == MobKind::Ember)
        .unwrap();
    look_at(&mut h, ember);
    h.frames(3);
    h.snapshot("ember");
    h.wait_until("the player hit and burning", SERVER, |w| {
        let player = local_player(w);
        player.health < MAX_HEALTH && player.is_burning()
    });
    let (_, cinderling) = animals(h.world())
        .into_iter()
        .find(|&(kind, _)| kind == MobKind::Cinderling)
        .unwrap();
    look_at(&mut h, cinderling);
    h.frames(5);
    h.snapshot("cinderling");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_gust_owl_hatched_next_to_a_survival_player_circles_over_it_and_dives_at_it() {
    let folder = GameFolder::new().with_world("Owls").with_items(
        "Owls",
        "Tester",
        &[(0, ItemId::GustOwlSpawnEgg, 1)],
    );
    let mut h = enter_world(folder, "Owls");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::GustOwlSpawnEgg, 1))
    });
    assert_eq!(local_player(h.world()).game_mode, GameMode::Survival);
    let player = look_at_the_ground_ahead(&mut h);

    right_click(&mut h);
    h.wait_until("the gust owl high over the player", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::GustOwl && at.y > player.y + 5.0)
    });
    let (_, owl) = animals(h.world())
        .into_iter()
        .find(|&(kind, _)| kind == MobKind::GustOwl)
        .unwrap();
    look_at(&mut h, owl);
    h.frames(3);
    h.snapshot("circling");
    let before = player_position(h.world_mut());
    // A slow start may have cost a fall already
    let health = local_player(h.world()).health;
    h.wait_until("a dive", SERVER, |w| local_player(w).health < health);
    // 2 points, less the point of health a player hurt by that fall may get back in the same
    // update, the client drawing several ticks a frame when slow (the server tests count them)
    let lost = health - local_player(h.world()).health;
    assert!((1..=2).contains(&lost), "lost {lost}");
    // Knocked back
    h.frames(30);
    let after = player_position(h.world_mut());
    let pushed = (after - before).xz().length();
    assert!(pushed > 1.0, "pushed {pushed} blocks");
    h.snapshot("dived");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_hostiles_are_drawn_where_their_eggs_hatch_them() {
    let kinds = [
        MobKind::Gnawer,
        MobKind::Bramble,
        MobKind::Roller,
        MobKind::Stumpling,
        MobKind::Drowner,
        MobKind::Cinderling,
        MobKind::GustOwl,
    ];
    let eggs = [
        (0, ItemId::GnawerSpawnEgg, 1),
        (1, ItemId::BrambleSpawnEgg, 1),
        (2, ItemId::RollerSpawnEgg, 1),
        (3, ItemId::StumplingSpawnEgg, 1),
        (4, ItemId::DrownerSpawnEgg, 1),
        (5, ItemId::CinderlingSpawnEgg, 1),
        (6, ItemId::GustOwlSpawnEgg, 1),
    ];
    let folder = GameFolder::new()
        .with_world("Zoo")
        .with_player("Zoo", "Tester", |player| {
            // Left alone, to be looked at
            player.game_mode = GameMode::Creative;
            for (slot, item_id, nb) in eggs {
                let stack = shared::world::ItemStack {
                    item_id,
                    item_type: item_id.get_default_type(),
                    nb,
                };
                player.inventory.inner.insert(slot, stack);
            }
        });
    let mut h = enter_world(folder, "Zoo");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 6) == Some((ItemId::GustOwlSpawnEgg, 1))
    });
    look_at_the_ground_ahead(&mut h);
    let keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
    ];
    for (key, kind) in keys.into_iter().zip(kinds) {
        h.press(key);
        h.frames(2);
        right_click(&mut h);
        h.wait_until("the hatched hostile", SERVER, |w| {
            animals(w).iter().any(|&(k, _)| k == kind)
        });
        // The next one a little to the side
        let world = h.world_mut();
        let mut controller = world
            .query::<&mut CameraController>()
            .single_mut(world)
            .unwrap();
        controller.angle_x += 0.33;
        h.frames(5);
    }
    // Step back, then look at them all
    let start = local_player(h.world()).position;
    h.key_down(KeyCode::KeyS);
    h.wait_until("six blocks back", SERVER, |w| {
        (local_player(w).position - start).xz().length() > 6.0
    });
    h.key_up(KeyCode::KeyS);
    let hatched: Vec<_> = animals(h.world())
        .into_iter()
        .filter(|(kind, _)| kind.is_hostile())
        .collect();
    assert!(!hatched.is_empty(), "{:?}", animals(h.world()));
    let centre = hatched.iter().map(|&(_, at)| at).sum::<Vec3>() / hatched.len() as f32;
    look_at(&mut h, centre);
    h.frames(60);
    let drawn: Vec<MobKind> = animals(h.world()).into_iter().map(|(k, _)| k).collect();
    for kind in kinds {
        assert!(drawn.contains(&kind), "{kind:?} in {drawn:?}");
    }
    h.snapshot("hostiles");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_cow_held_rye_eats_one_and_hearts_rise_over_it() {
    let folder = GameFolder::new().with_world("Farm").with_items(
        "Farm",
        "Tester",
        &[(0, ItemId::CowSpawnEgg, 1), (1, ItemId::Rye, 5)],
    );
    let mut h = enter_world(folder, "Farm");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 1) == Some((ItemId::Rye, 5))
    });
    let player = look_at_the_ground_ahead(&mut h);
    right_click(&mut h);
    h.wait_until("the hatched cow", SERVER, |w| {
        animals(w)
            .iter()
            .any(|&(kind, at)| kind == MobKind::Cow && at.distance(player) < 5.0)
    });
    h.press(KeyCode::Digit2);
    h.frames(2);
    // The cow may walk: it is looked at again before each try, for as long as a server may take,
    // and walked to when it went out of reach (3 blocks from the eye to its box)
    let start = Instant::now();
    while slot(h.world(), 1) != Some((ItemId::Rye, 4)) {
        assert!(start.elapsed() < SERVER, "no rye eaten");
        let cow = animals(h.world())
            .into_iter()
            .find(|&(kind, _)| kind == MobKind::Cow)
            .unwrap()
            .1;
        look_at(&mut h, cow);
        let me = player_position(h.world_mut());
        if cow.xz().distance(me.xz()) > 2.0 {
            h.key_down(KeyCode::KeyW);
            h.frames(2);
            h.key_up(KeyCode::KeyW);
            continue;
        }
        h.frames(2);
        right_click(&mut h);
        h.frames(10);
    }
    h.wait_until("hearts over the cow", SERVER, |w| {
        let in_love = w
            .try_query::<&Mob>()
            .is_some_and(|mut mobs| mobs.iter(w).any(|mob| mob.in_love));
        let hearts = w
            .try_query::<&crate::portal::particles::Particle>()
            .is_some_and(|mut hearts| hearts.iter(w).count() > 0);
        in_love && hearts
    });
    h.snapshot("cow_in_love");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_villager_buys_what_a_player_sells() {
    let eggs = [
        (0, ItemId::FarmerSpawnEgg, 1),
        (1, ItemId::SmithSpawnEgg, 1),
        (2, ItemId::CartographerSpawnEgg, 1),
        (3, ItemId::Rye, 30),
    ];
    let folder = GameFolder::new()
        .with_world("Market")
        .with_player("Market", "Tester", |player| {
            for (slot, item_id, nb) in eggs {
                let stack = shared::world::ItemStack {
                    item_id,
                    item_type: item_id.get_default_type(),
                    nb,
                };
                player.inventory.inner.insert(slot, stack);
            }
        });
    let mut h = enter_world(folder, "Market");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 3) == Some((ItemId::Rye, 30))
    });
    look_at_the_ground_ahead(&mut h);
    let kinds = [MobKind::Cartographer, MobKind::Smith, MobKind::Farmer];
    let keys = [KeyCode::Digit3, KeyCode::Digit2, KeyCode::Digit1];
    for (key, kind) in keys.into_iter().zip(kinds) {
        h.press(key);
        h.frames(2);
        right_click(&mut h);
        h.wait_until("the hatched villager", SERVER, |w| {
            animals(w).iter().any(|&(k, _)| k == kind)
        });
        let world = h.world_mut();
        let mut controller = world
            .query::<&mut CameraController>()
            .single_mut(world)
            .unwrap();
        controller.angle_x += 0.9;
        h.frames(5);
    }
    let at = |w: &World, kind| animals(w).into_iter().find(|&(k, _)| k == kind).unwrap().1;
    // The farmer, with rye in hand
    h.press(KeyCode::Digit4);
    let shown = |w: &World| w.resource::<OpenTrade>().0.is_some();
    for _ in 0..10 {
        let farmer = at(h.world(), MobKind::Farmer);
        if shown(h.world()) {
            break;
        }
        look_at(&mut h, farmer);
        h.frames(2);
        right_click(&mut h);
        h.frames(10);
    }
    h.wait_until("the trade screen", SERVER, shown);
    h.wait_for(&L::text("Farmer"), SERVER);
    let sell = L::role(Role::Button, "1 Coin for 12 Rye");
    h.click(&sell);
    h.wait_until("twelve rye sold for a coin", SERVER, |w| {
        slot(w, 3) == Some((ItemId::Rye, 18)) && slot(w, 0) == Some((ItemId::Coin, 1))
    });
    h.wait_for(&L::text("7 left"), SERVER);
    h.frames(10);
    h.snapshot("trade_screen");

    h.press(KeyCode::KeyE);
    h.wait_until("the closed screen", SERVER, |w| !shown(w));
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));

    // Step back, then look at them all
    let start = local_player(h.world()).position;
    h.key_down(KeyCode::KeyS);
    h.wait_until("three blocks back", SERVER, |w| {
        (local_player(w).position - start).xz().length() > 3.0
    });
    h.key_up(KeyCode::KeyS);
    let smith = at(h.world(), MobKind::Smith);
    look_at(&mut h, smith);
    // Only the sky of the client: the villagers stay
    h.world_mut().resource_mut::<ClientTime>().0 = shared::NOON_IN_DAY;
    h.frames(60);
    h.snapshot("villagers");
}
