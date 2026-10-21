//! The bucket, through the integrated server: water poured on the ground and taken back like a
//! player does, with the right button.

use bevy::prelude::*;
use shared::world::{BlockData, BlockId, ItemId};

use super::game::{aimed_cells, enter_world, hold_click, slot};
use super::harness::SERVER;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;
use crate::world::ClientWorldMap;

/// The water at `position`, as the client knows it.
fn water(w: &World, position: IVec3) -> Option<BlockData> {
    w.resource::<ClientWorldMap>()
        .blocks
        .get_block(position)
        .copied()
        .filter(|block| block.id == BlockId::Water)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_player_pours_water_from_a_bucket_and_takes_it_back() {
    let folder = GameFolder::new().with_world("Bucket").with_items(
        "Bucket",
        "Tester",
        &[(0, ItemId::WaterBucket, 1)],
    );
    let mut h = enter_world(folder, "Bucket");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::WaterBucket, 1))
    });

    // Look down at the ground ahead, once settled on it
    let world = h.world_mut();
    let camera = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    let player = world
        .query_filtered::<Entity, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap();
    world.get_mut::<CameraController>(camera).unwrap().angle_y = 0.9;
    h.wait_until("the ground within reach", SERVER, |w| {
        aimed_cells(w, camera, player).is_some_and(|(target, _)| {
            let centre = w.get::<Transform>(player).unwrap().translation;
            target.as_vec3().distance(centre) < 3.
        })
    });
    h.frames(30);
    let (_, cell) = aimed_cells(h.world(), camera, player).unwrap();

    // A right click pours a source there, which spreads, and leaves an empty bucket
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_until("the source and the empty bucket", SERVER, |w| {
        water(w, cell).is_some_and(|block| block.level == 0)
            && slot(w, 0) == Some((ItemId::Bucket, 1))
    });
    h.wait_until("the water spreading", SERVER, |w| {
        [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y]
            .into_iter()
            .any(|side| water(w, cell + side).is_some())
    });
    h.frames(20);
    h.snapshot("bucket_poured");

    // Another right click, aimed at the source, takes it back; what flowed from it drains
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_until("the water bucket", SERVER, |w| {
        slot(w, 0) == Some((ItemId::WaterBucket, 1))
            && water(w, cell).is_none_or(|block| block.level != 0)
    });
    h.wait_until("the water drained", SERVER, |w| {
        [IVec3::ZERO, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
            .into_iter()
            .all(|side| water(w, cell + side).is_none())
    });
    h.snapshot("bucket_taken_back");
}
