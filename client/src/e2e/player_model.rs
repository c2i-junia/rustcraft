//! The model of the player: seen in third person, it is a body cut from its skin, which swings
//! its legs as it walks.

use bevy::prelude::*;
use shared::players::Player;

use super::game::enter_world;
use super::harness::{Harness, SCREEN, SERVER};
use super::seed::GameFolder;
use crate::player::model::{BodyPart, PlayerModelRoot};
use crate::player::{CurrentPlayerMarker, PlayerBody};

fn player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

/// The pitch of the local player's right leg.
fn right_leg(w: &World) -> f32 {
    let mut parts = w.try_query::<(&BodyPart, &Transform)>().unwrap();
    let (_, transform) = parts
        .iter(w)
        .find(|(part, _)| **part == BodyPart::RightLeg)
        .unwrap();
    transform.rotation.to_euler(EulerRot::YXZ).1
}

/// The mean colour of the pixels in a square of `size` about the centre of `shot`.
fn centre_color(shot: &image::RgbImage, size: u32) -> Vec3 {
    let (cx, cy) = (shot.width() / 2, shot.height() / 2);
    let mut sum = Vec3::ZERO;
    for y in cy - size / 2..cy + size / 2 {
        for x in cx - size / 2..cx + size / 2 {
            sum += Vec3::from_array(shot.get_pixel(x, y).0.map(f32::from));
        }
    }
    sum / (size * size) as f32
}

fn show_body(h: &mut Harness, visibility: Visibility) {
    let w = h.world_mut();
    let bodies: Vec<Entity> = w
        .query_filtered::<Entity, With<PlayerBody>>()
        .iter(w)
        .collect();
    for body in bodies {
        w.entity_mut(body).insert(visibility);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn in_third_person_the_player_is_a_body_that_walks() {
    let mut h = enter_world(GameFolder::new().with_world("Model"), "Model");
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);

    // Six parts, each with its two layers, under one root
    let w = h.world_mut();
    assert_eq!(w.query::<&PlayerModelRoot>().iter(w).count(), 1);
    assert_eq!(w.query::<&BodyPart>().iter(w).count(), 6);

    h.key_down(KeyCode::F5);
    h.frames(1);
    h.key_up(KeyCode::F5);
    h.frames(20);
    h.snapshot("standing");

    // The camera looks at the player: the middle of the picture is its body, not what is
    // behind it
    let with_body = h.screenshot();
    show_body(&mut h, Visibility::Hidden);
    let without_body = h.screenshot();
    show_body(&mut h, Visibility::Inherited);
    let difference = (centre_color(&with_body, 8) - centre_color(&without_body, 8)).length();
    assert!(difference > 20.0, "no body in the middle: {difference}");

    // Walking, the legs swing
    h.key_down(KeyCode::KeyW);
    h.wait_until("a leg swung forward", SCREEN, |w| right_leg(w) > 0.3);
    h.wait_until("a leg swung back", SCREEN, |w| right_leg(w) < -0.3);
    h.snapshot("walking");
    h.key_up(KeyCode::KeyW);
    h.wait_until("the legs together", SCREEN, |w| right_leg(w).abs() < 0.05);
}
