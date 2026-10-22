//! The lamp of the example mod Beacon, a block of a mod in the game: its texture, its light,
//! brighter than a torch's, its recipe, and the lamp it drops when mined.
//!
//! The blocks of the mods are read once, as the game starts (`load_content`), and the tests of
//! this binary share the base mod's. So the test runs again in a process of its own, which reads
//! them with Beacon's first.

use std::process::Command;

use bevy::{
    input::{mouse::MouseButtonInput, ButtonState},
    prelude::*,
    window::{PrimaryWindow, WindowEvent},
};
use shared::world::{content, load_content, match_recipe, BlockId, ItemId};
use shared::CHUNK_SIZE;

use super::game::{aimed_cells, enter_world, hold_click, is_block, slot};
use super::harness::{SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::dropped_items::DroppedItemRoot;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;
use crate::world::lighting::{LightVolume, TerrainPrior};
use crate::world::{ClientWorldMap, MaterialResource};

/// The folder of Beacon in the repository.
fn beacon() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/mods/beacon")
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_lamp_of_beacon_lights_more_than_a_torch_and_drops_itself() {
    let name = concat!(module_path!(), "::in_its_own_process");
    // the path of the test in its binary, without the crate
    let name = name.split_once("::").map_or(name, |(_, path)| path);
    let status = Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--ignored", "--nocapture"])
        .status()
        .unwrap();
    assert!(status.success(), "the lamp's test failed in its process");
}

/// The test, in the process `the_lamp_of_beacon_lights_more_than_a_torch_and_drops_itself`
/// starts.
#[test]
#[ignore = "run by the_lamp_of_beacon_lights_more_than_a_torch_and_drops_itself"]
fn in_its_own_process() {
    let folder = GameFolder::new()
        .with_world("Lamp")
        .with_mod_copy("beacon", &beacon());
    load_content(Some(folder.path())).unwrap();
    let lamp = content().by_name("beacon:lamp").expect("Beacon's lamp");
    let item = ItemId::Block(lamp);

    // Its recipe: four torches around a coal, cobblestone in the corners
    let (c, t, k) = (
        Some(ItemId::Cobblestone),
        Some(ItemId::Torch),
        Some(ItemId::Coal),
    );
    let crafted = match_recipe(&[c, t, c, t, k, t, c, t, c]).unwrap();
    assert_eq!((crafted.item_id, crafted.nb), (item, 1));

    let folder = folder.with_items("Lamp", "Tester", &[(0, item, 1)]);
    let mut h = enter_world(folder, "Lamp");
    h.wait_until("the saved inventory", SERVER, |w| slot(w, 0) == Some((item, 1)));
    h.wait_until("the lamp's texture", SCREEN, |w| {
        w.resource::<MaterialResource>()
            .blocks
            .as_ref()
            .is_some_and(|blocks| blocks.tiles.contains_key("beacon:lamp"))
    });

    // Look down, at the ground ahead of the player
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
    // Settled on the ground
    h.frames(30);

    h.snapshot("held");

    // Place the lamp, and aim at it
    let (_, target) = aimed_cells(h.world(), camera, player).unwrap();
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 6);
    h.wait_until("the placed lamp", SERVER, |w| {
        let map = &w.resource::<ClientWorldMap>().blocks;
        map.get_block(target).is_some_and(|block| block.id == lamp) && slot(w, 0).is_none()
    });
    assert_eq!(aimed_cells(h.world(), camera, player).unwrap().0, target);
    h.frames(10);
    h.snapshot("placed");

    // Its light: 15 in its cell, one less a cell away, where a torch's would be 13
    let map = &h.world().resource::<ClientWorldMap>().blocks;
    let chunk = target.div_euclid(IVec3::splat(CHUNK_SIZE));
    let light = LightVolume::of_chunks(map, &[chunk], &mut TerrainPrior::new(None));
    let torch = content().get(BlockId::Torch).unwrap().light;
    assert_eq!(light.light(target).block, 15);
    assert_eq!(light.light(target + IVec3::Y).block, 14);
    assert!(light.light(target + IVec3::Y).block > torch - 1);

    // Mine it: the block goes, and drops a lamp, which the player picks up
    let center = h.center_of(&L::marker::<Hotbar>());
    h.move_cursor(center);
    h.frames(1);
    let window = h
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap();
    let click = |state| MouseButtonInput {
        button: MouseButton::Left,
        state,
        window,
    };
    let pressed = click(ButtonState::Pressed);
    h.world_mut().write_message(pressed);
    h.world_mut()
        .write_message(WindowEvent::MouseButtonInput(pressed));
    h.wait_until("the mined lamp to break", SERVER, |w| !is_block(w, target));
    let released = click(ButtonState::Released);
    h.world_mut().write_message(released);
    h.world_mut()
        .write_message(WindowEvent::MouseButtonInput(released));
    h.wait_until("the dropped lamp", SERVER, |w| {
        w.try_query::<&DroppedItemRoot>()
            .is_some_and(|mut items| items.iter(w).count() == 1)
    });
    h.key_down(KeyCode::KeyW);
    h.wait_until("the picked up lamp", SERVER, |w| slot(w, 0) == Some((item, 1)));
    h.key_up(KeyCode::KeyW);
}
