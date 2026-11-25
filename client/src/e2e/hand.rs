//! The item in the hand: in first person the player sees its arm and the item of its selected
//! hotbar slot in the lower right of the screen, which strikes with the left button; in third
//! person its model holds the item in its right hand.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use shared::players::Player;
use shared::world::ItemId;

use super::game::{enter_world, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::player::hand::{
    view_model_transform, Grip, HandMotion, HandOffset, HeldItemMesh, ItemHolder, ViewModel,
    ViewModelArm,
};
use crate::player::CurrentPlayerMarker;
use crate::portal::{BODY_LAYER, VIEW_MODEL_LAYER};

fn player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

/// The item the holder of `grip` wants, and the items drawn in it with their layers.
fn holder(w: &World, grip: Grip) -> (Option<ItemId>, Vec<(ItemId, Option<RenderLayers>)>) {
    let mut holders = w.try_query::<(Entity, &ItemHolder)>().unwrap();
    let (entity, holder) = holders
        .iter(w)
        .find(|(_, holder)| holder.grip == grip)
        .unwrap();
    let mut meshes = w
        .try_query::<(&HeldItemMesh, &ChildOf, Option<&RenderLayers>)>()
        .unwrap();
    let drawn = meshes
        .iter(w)
        .filter(|(_, parent, _)| parent.parent() == entity)
        .map(|(mesh, _, layers)| (mesh.0, layers.cloned()))
        .collect();
    (holder.wanted, drawn)
}

fn hand(w: &World) -> (HandMotion, Transform, Visibility) {
    let mut roots = w
        .try_query_filtered::<(&HandMotion, &Transform, &Visibility), With<ViewModel>>()
        .unwrap();
    let (motion, transform, visibility) = roots.single(w).unwrap();
    (motion.clone(), *transform, *visibility)
}

/// Whether the view model holds `item`, raised and still.
fn holds(w: &World, item: Option<ItemId>) -> bool {
    let (motion, _, _) = hand(w);
    let (drawn, meshes) = holder(w, Grip::FirstPerson);
    motion.shown == item
        && motion.lowered == 0.0
        && drawn == item
        && meshes.iter().map(|(item, _)| *item).collect::<Vec<_>>() == Vec::from_iter(item)
}

/// The arm of the view model, and the item held in it.
fn view_model_parts(h: &mut Harness) -> (Entity, Entity) {
    let w = h.world_mut();
    let arm = w
        .query_filtered::<Entity, With<ViewModelArm>>()
        .single(w)
        .unwrap();
    let mut holders = w.query::<(Entity, &ItemHolder)>();
    let item = holders
        .iter(w)
        .find(|(_, holder)| holder.grip == Grip::FirstPerson)
        .unwrap()
        .0;
    (arm, item)
}

/// Shows or hides a part of the view model.
fn show(h: &mut Harness, part: Entity, visibility: Visibility) {
    h.world_mut().entity_mut(part).insert(visibility);
}

/// How many pixels of the lower right quarter of the screen differ between `a` and `b`.
fn differing_in_the_corner(a: &image::RgbImage, b: &image::RgbImage) -> usize {
    let (width, height) = a.dimensions();
    let mut count = 0;
    for y in height / 2..height {
        for x in width * 5 / 8..width {
            let [pa, pb] =
                [a, b].map(|shot| Vec3::from_array(shot.get_pixel(x, y).0.map(f32::from)));
            if (pa - pb).length() > 20.0 {
                count += 1;
            }
        }
    }
    count
}

/// How many pixels of the corner of the screen the item in the hand covers, and how many the arm
/// covers. Each is measured by hiding it and comparing the pictures, the world hidden meanwhile:
/// its grass sways and its clouds move between them.
fn item_and_arm_pixels(h: &mut Harness) -> (usize, usize) {
    let (arm, item) = view_model_parts(h);
    // The entities of the world drawn now, and what they were shown as
    let w = h.world_mut();
    let world: Vec<(Entity, Visibility)> = w
        .query_filtered::<(Entity, &Visibility, Option<&RenderLayers>), With<Mesh3d>>()
        .iter(w)
        .filter(|(_, _, layers)| *layers != Some(&RenderLayers::layer(VIEW_MODEL_LAYER)))
        .map(|(entity, visibility, _)| (entity, *visibility))
        .collect();
    for (entity, _) in &world {
        show(h, *entity, Visibility::Hidden);
    }
    h.frames(10);
    let whole = h.screenshot();
    show(h, item, Visibility::Hidden);
    let without_item = h.screenshot();
    show(h, arm, Visibility::Hidden);
    let bare = h.screenshot();
    show(h, item, Visibility::Inherited);
    show(h, arm, Visibility::Inherited);
    for (entity, visibility) in world {
        if let Ok(mut entity) = h.world_mut().get_entity_mut(entity) {
            entity.insert(visibility);
        }
    }
    h.frames(2);
    (
        differing_in_the_corner(&whole, &without_item),
        differing_in_the_corner(&without_item, &bare),
    )
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_player_sees_its_arm_and_the_item_it_holds_and_the_model_holds_it_too() {
    let folder = GameFolder::new().with_world("Hand").with_items(
        "Hand",
        "Tester",
        &[(0, ItemId::WoodenPickaxe, 1), (1, ItemId::Dirt, 5)],
    );
    let mut h = enter_world(folder, "Hand");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::WoodenPickaxe, 1))
    });
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    // At noon, the light and the sky the same for a while
    h.set_time("day");
    h.frames(30);

    // The pickaxe of the first slot, in the lower right, with the arm
    h.wait_until("the pickaxe in the hand", SERVER, |w| {
        holds(w, Some(ItemId::WoodenPickaxe))
    });
    h.frames(10);
    assert_eq!(hand(h.world()).2, Visibility::Inherited);
    h.snapshot("pickaxe");
    let (pickaxe, arm) = item_and_arm_pixels(&mut h);
    assert!(arm > 2000, "no arm in the corner: {arm}");
    assert!(pickaxe > 1500, "no pickaxe in the hand: {pickaxe}");

    // An empty slot: the hand goes down, and comes up with the arm alone
    h.press(KeyCode::Digit3);
    h.wait_until("the hand going down", SCREEN, |w| hand(w).0.lowered > 0.0);
    h.wait_until("the empty hand", SCREEN, |w| holds(w, None));
    h.frames(10);
    h.snapshot("empty_hand");
    let (nothing, arm) = item_and_arm_pixels(&mut h);
    assert!(nothing < 100, "something in an empty hand: {nothing}");
    assert!(arm > 2000, "no arm in the corner: {arm}");

    // The dirt, as a small block
    h.press(KeyCode::Digit2);
    h.wait_until("the dirt in the hand", SCREEN, |w| {
        holds(w, Some(ItemId::Dirt))
    });
    h.frames(10);
    h.snapshot("dirt");
    let (dirt, _) = item_and_arm_pixels(&mut h);
    assert!(dirt > 1500, "no block in the hand: {dirt}");

    // The left button strikes: the hand leaves its rest
    let rest = view_model_transform(HandOffset::NONE);
    h.button_down(MouseButton::Left);
    h.wait_until("the hand striking", SCREEN, |w| {
        hand(w).1.translation.distance(rest.translation) > 0.03
    });
    h.snapshot("striking");
    h.button_up(MouseButton::Left);
    h.wait_until("the hand back at rest", SCREEN, |w| {
        hand(w).1.translation.distance(rest.translation) < 1e-4
    });

    // Placing the dirt on the ground ahead pushes the hand
    let w = h.world_mut();
    let mut controllers = w.query::<&mut CameraController>();
    controllers.single_mut(w).unwrap().angle_y = 0.7;
    h.frames(5);
    h.button_down(MouseButton::Right);
    h.wait_until("the hand pushing the block placed", SCREEN, |w| {
        hand(w).0.using.is_some()
    });
    h.button_up(MouseButton::Right);
    h.wait_until("the block placed", SERVER, |w| {
        slot(w, 1).is_some_and(|(_, count)| count < 5)
    });
    h.wait_until("the hand back at rest", SCREEN, |w| {
        hand(w).0.using.is_none()
    });

    // Its model holds the dirt too, on the body's layer: seen through the portals only in first
    // person, by the player's camera in third person, where the view model hides
    let (wanted, meshes) = holder(h.world(), Grip::ThirdPerson);
    assert_eq!(wanted, Some(ItemId::Dirt));
    assert_eq!(
        meshes,
        vec![(ItemId::Dirt, Some(RenderLayers::layer(BODY_LAYER)))]
    );
    h.key_down(KeyCode::F5);
    h.frames(1);
    h.key_up(KeyCode::F5);
    h.frames(10);
    assert_eq!(hand(h.world()).2, Visibility::Hidden);
    let (_, meshes) = holder(h.world(), Grip::ThirdPerson);
    assert_eq!(meshes, vec![(ItemId::Dirt, Some(RenderLayers::layer(0)))]);
    h.snapshot("third_person");

    // And the pickaxe, once selected
    h.press(KeyCode::Digit1);
    h.wait_until("the pickaxe in the hand of the model", SCREEN, |w| {
        let (_, meshes) = holder(w, Grip::ThirdPerson);
        meshes == [(ItemId::WoodenPickaxe, Some(RenderLayers::layer(0)))]
    });
    // Pictured from its right side, where the tool is seen whole
    let at = player(h.world()).position;
    let w = h.world_mut();
    let (camera, transform) = w
        .query_filtered::<(Entity, &Transform), With<CameraController>>()
        .single(w)
        .map(|(camera, transform)| (camera, *transform))
        .unwrap();
    let side = at + transform.right() * 2.5 + transform.forward() * 1.5 + Vec3::Y * 0.5;
    w.entity_mut(camera)
        .remove::<CameraController>()
        .insert(Transform::from_translation(side).looking_at(at, Vec3::Y));
    h.frames(10);
    h.snapshot("third_person_pickaxe");
}
