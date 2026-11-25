//! The armour: worn by the player, its bar shows above the hearts, its pieces in the armour
//! slots of the inventory, and its boxes on the model.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use shared::players::armour::ArmourPiece;
use shared::players::{Inventory, Player};
use shared::world::ItemId;

use super::game::enter_world;
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::player::armour::WornMesh;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::armour::{armour_label, ArmourBar};

fn player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

/// The layers of the boxes of armour drawn.
fn boxes(w: &World) -> Vec<Option<RenderLayers>> {
    let mut meshes = w
        .try_query_filtered::<Option<&RenderLayers>, With<WornMesh>>()
        .unwrap();
    meshes.iter(w).map(|layers| layers.cloned()).collect()
}

/// Pictures the player from `offset`, with the camera freed of it.
fn picture(h: &mut Harness, camera: Entity, offset: Vec3, name: &str) {
    let at = player(h.world()).position;
    h.world_mut()
        .entity_mut(camera)
        .remove::<CameraController>()
        .insert(Transform::from_translation(at + offset).looking_at(at, Vec3::Y));
    h.frames(10);
    h.snapshot(name);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn worn_armour_shows_in_its_slots_in_its_bar_and_on_the_model() {
    let [helmet, chestplate, leggings, boots] = ArmourPiece::ALL.map(ArmourPiece::slot);
    let folder = GameFolder::new().with_world("Armour").with_items(
        "Armour",
        "Tester",
        &[
            (0, ItemId::DiamondHelmet, 1),
            (9, ItemId::LeatherHelmet, 1),
            (10, ItemId::BronzeChestplate, 1),
            (chestplate, ItemId::IronChestplate, 1),
            (leggings, ItemId::BronzeLeggings, 1),
            (boots, ItemId::LeatherBoots, 1),
        ],
    );
    let mut h = enter_world(folder, "Armour");
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    h.set_time("day");
    h.wait_until("the armour on the model", SERVER, |w| boxes(w).len() == 8);
    let bar = h.get(&L::marker::<ArmourBar>());
    let label = h.world().get::<AccessibleLabel>(bar).unwrap();
    assert_eq!(label.0, armour_label(10));
    h.frames(30);
    h.snapshot("bar");

    // The helmet is put on by a quick move
    h.press(KeyCode::KeyE);
    h.wait_for(&L::cell(helmet), SCREEN);
    h.snapshot("inventory_bare_head");
    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::cell(0));
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the helmet worn", SERVER, |w| {
        w.resource::<Inventory>().worn().0[0] == Some(ItemId::DiamondHelmet)
    });
    h.wait_until("the helmet on the model", SCREEN, |w| boxes(w).len() == 9);
    h.snapshot("inventory");
    h.press(KeyCode::KeyE);

    // In third person the player sees its armour
    h.key_down(KeyCode::F5);
    h.frames(1);
    h.key_up(KeyCode::F5);
    h.frames(10);
    assert!(boxes(h.world())
        .iter()
        .all(|layers| *layers == Some(RenderLayers::layer(0))));
    h.snapshot("third_person");
    let w = h.world_mut();
    let mut cameras = w.query_filtered::<Entity, With<CameraController>>();
    let camera = cameras.single(w).unwrap();
    picture(&mut h, camera, Vec3::new(1.2, 0.6, -2.0), "front");
    picture(&mut h, camera, Vec3::new(-1.2, 0.8, 2.0), "back");
}
