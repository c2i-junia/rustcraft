//! What a player holds in its hand. In first person the player sees, in the lower right of the
//! screen, its right arm and the item of its selected hotbar slot (the view model): they bob
//! while it walks, strike while the left button is held, push forward when a block is placed
//! or an item used, and go down and up again when the item in the hand changes. The others, and
//! the player itself in third person, see the item in the right hand of its model, which
//! follows the arm. An enchanted item glints in either (`dropped_items::glint`).
//!
//! The view model is a child of the player's camera, on a layer that only that camera draws
//! (`VIEW_MODEL_LAYER`): no portal shows it. It is drawn small and close to the eye, nearer
//! than any wall can be to a player's eye (half of `PLAYER_WIDTH`), so it never sinks into one.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use super::compass::CompassNeedle;
use bevy::{
    asset::AssetId,
    camera::visibility::RenderLayers,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use shared::players::bow::{full_draw_ms, power_of};
use shared::players::{Held, Inventory, Player, ViewMode};
use shared::world::{ItemId, ItemStack};

use crate::camera::{CameraController, FOV_DEGREES};
use crate::dropped_items::glint::Glints;
use crate::dropped_items::mesh::{ItemLook, ItemLooks};
use crate::portal::VIEW_MODEL_LAYER;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::UIMode;
use crate::world::{BlockChanged, MaterialResource};

use super::model::{box_mesh, BodyPart, PlayerAnimation, LAYER_INFLATE, MODEL_PIXELS};
use super::CurrentPlayerMarker;

/// The materials the arm and the pictures of the view model are drawn with: a copy of each,
/// unlit, dimmed as a whole with the light of the day (`light_the_view_model`). The world's own
/// materials would leave the hand in the dark whenever the sun is ahead of the eye.
#[derive(Resource, Default)]
pub struct ViewModelMaterials(HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>);

impl ViewModelMaterials {
    /// The unlit copy of `material`, made the first time it is asked for.
    fn unlit(
        &mut self,
        material: &Handle<StandardMaterial>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        if let Some(copy) = self.0.get(&material.id()) {
            return copy.clone();
        }
        let Some(lit) = materials.get(material) else {
            return material.clone();
        };
        let copy = materials.add(StandardMaterial {
            unlit: true,
            ..lit.clone()
        });
        self.0.insert(material.id(), copy.clone());
        copy
    }
}

/// The item a player holds: that of its selected hotbar slot. The local player's follows its
/// hotbar (`track_the_local_held_item`), the others' the server (`PlayerUpdateEvent::held`).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HeldItem(pub Option<Held>);

impl HeldItem {
    /// The item held, if any.
    pub fn item(self) -> Option<ItemId> {
        self.0.map(|held| held.item)
    }
}

/// The material of a player's skin, which its model and its arm in first person are drawn with.
#[derive(Component, Debug, Clone)]
pub struct SkinMaterial(pub Handle<StandardMaterial>);

/// Where an item is held, which decides where and how big it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grip {
    /// In the view model, seen from the eye.
    FirstPerson,
    /// In the right hand of a player's model.
    ThirdPerson,
}

/// An entity that draws an item in a hand as its child (`HeldItemMesh`): in the right hand of
/// a model, or in the view model.
#[derive(Component, Debug)]
pub struct ItemHolder {
    pub grip: Grip,
    /// The item to draw, if any: `draw_held_items` follows it.
    pub wanted: Option<ItemId>,
    /// How far the bow in it is drawn, if it is (`bow::draw_stage`), or where the needle of the
    /// compass in it points (`compass::CompassNeedle`).
    pub draw: Option<u8>,
    /// Whether the item is enchanted, which draws its glint (`glint`).
    pub enchanted: bool,
}

impl ItemHolder {
    pub fn new(grip: Grip) -> Self {
        Self {
            grip,
            wanted: None,
            draw: None,
            enchanted: false,
        }
    }
}

/// The mesh of the item drawn by an `ItemHolder`, how far drawn a bow it shows, and whether it
/// glints.
#[derive(Component, Debug)]
pub struct HeldItemMesh(pub ItemId, pub Option<u8>, pub bool);

/// The frame the view model hangs from, the child of the player's camera: it stretches with the
/// field of view (`frame_scale`), so that the hand stays the same on screen while the player
/// sprints or zooms.
#[derive(Component, Debug)]
pub struct ViewModelFrame;

/// The root of the view model, a child of its frame, moved by its `HandMotion`.
#[derive(Component, Debug)]
pub struct ViewModel;

/// The arm of the view model.
#[derive(Component, Debug)]
pub struct ViewModelArm;

/// Seconds the hand takes to go down out of sight, or up again, when the item in it changes.
const EQUIP_SECONDS: f32 = 0.15;
/// Seconds of the push of the hand when a block is placed or an item used.
const USE_SECONDS: f32 = 0.25;

/// What moves the view model from one frame to the next, besides the walk and the strike, which
/// it reads from the player's `PlayerAnimation`.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct HandMotion {
    /// The item in the view model: the one held, once the hand went down to change it.
    pub shown: Option<ItemId>,
    /// Whether the item shown is enchanted: that of the one held, once it is shown.
    pub enchanted: bool,
    /// From 0, the hand raised, to 1, out of sight below the screen.
    pub lowered: f32,
    /// Where the hand is in its push, from 0 to 1, while it pushes.
    pub using: Option<f32>,
}

impl HandMotion {
    /// Moves the hand `delta` seconds forward, the player holding `held` and having just placed
    /// a block or used an item if `used`. Another item lowers the hand, which takes it at the
    /// bottom and comes up with it.
    pub fn advance(&mut self, held: Option<ItemId>, used: bool, delta: f32) {
        self.using = match self.using {
            _ if used => Some(0.0),
            Some(t) if t + delta / USE_SECONDS < 1.0 => Some(t + delta / USE_SECONDS),
            _ => None,
        };
        let step = delta / EQUIP_SECONDS;
        if held != self.shown {
            self.lowered = (self.lowered + step).min(1.0);
            if self.lowered >= 1.0 {
                self.shown = held;
            }
        } else {
            self.lowered = (self.lowered - step).max(0.0);
        }
    }
}

/// A move of the hand from where it rests: an offset in the space of the camera, in metres of the
/// view model before it is shrunk (`VIEW_MODEL_SCALE`), and a turn about the hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandOffset {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl HandOffset {
    pub const NONE: HandOffset = HandOffset {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    /// This move, then `other` from where it leaves the hand.
    pub fn then(self, other: HandOffset) -> HandOffset {
        HandOffset {
            translation: self.translation + other.translation,
            rotation: self.rotation * other.rotation,
        }
    }
}

/// How far the hand sways sideways and dips at each step of a walk at full speed.
const BOB_SWAY: f32 = 0.11;
const BOB_DIP: f32 = 0.15;

/// The bob of the hand of a player whose legs are at `walk_phase` of their swing, `walk_amount`
/// wide (`PlayerAnimation`): it sways from side to side once a stride, and dips at each step.
pub fn bob(walk_phase: f32, walk_amount: f32) -> HandOffset {
    let swing = walk_phase.sin();
    HandOffset {
        translation: Vec3::new(BOB_SWAY * swing, -BOB_DIP * swing.abs(), 0.0) * walk_amount,
        rotation: Quat::IDENTITY,
    }
}

/// The hand at `t`, from 0 to 1, of a strike: thrown forward and to the left, it comes down across
/// the screen and back, quick at first.
pub fn strike(t: f32) -> HandOffset {
    let root = t.sqrt();
    let arc = (root * PI).sin();
    HandOffset {
        translation: Vec3::new(
            -0.45 * arc,
            0.2 * (root * TAU).sin(),
            -0.55 * (t * PI).sin(),
        ),
        rotation: Quat::from_rotation_y(0.4 * (t * t * PI).sin())
            * Quat::from_rotation_z(0.25 * arc)
            * Quat::from_rotation_x(-0.55 * arc),
    }
}

/// The hand at `t`, from 0 to 1, of a use: a short push forward and up, and back.
pub fn push(t: f32) -> HandOffset {
    let arc = (t * PI).sin();
    HandOffset {
        translation: Vec3::new(-0.15, 0.18, -0.6) * arc,
        rotation: Quat::from_rotation_x(0.35 * arc),
    }
}

/// The hand `lowered` of the way down out of sight, from 0 to 1.
pub fn lowering(lowered: f32) -> HandOffset {
    HandOffset {
        translation: Vec3::new(0.0, -1.5 * lowered, 0.0),
        rotation: Quat::from_rotation_x(-0.4 * lowered),
    }
}

/// Where the hand goes as a bow is drawn to `power` of the way: the bow is raised ahead of the eye,
/// and trembles once it is fully drawn.
pub fn draw_bow(power: f32, time: f32) -> HandOffset {
    let raised = (power * 4.0).min(1.0);
    let tremble = if power >= 1.0 {
        (time * 45.0).sin() * 0.008
    } else {
        0.0
    };
    HandOffset {
        translation: Vec3::new(-0.5, 0.45, 0.15) * raised + Vec3::Y * tremble,
        rotation: Quat::from_rotation_z(0.12 * raised),
    }
}

/// The whole move of the hand: lowered, bobbing, striking (`swing`) and pushing as `motion`
/// and the walk say.
pub fn hand_offset(
    motion: &HandMotion,
    swing: Option<f32>,
    walk_phase: f32,
    walk_amount: f32,
) -> HandOffset {
    let mut offset = HandOffset::NONE
        .then(lowering(motion.lowered))
        .then(bob(walk_phase, walk_amount));
    if let Some(t) = swing {
        offset = offset.then(strike(t));
    }
    if let Some(t) = motion.using {
        offset = offset.then(push(t));
    }
    offset
}

/// How much smaller than the arm of the player the view model is drawn, and as much nearer to
/// the eye: it looks the same, and stays nearer than a wall.
pub const VIEW_MODEL_SCALE: f32 = 0.1;
/// Where the hand rests in the space of the camera, before it is shrunk: in the lower right.
pub const HAND_REST: Vec3 = Vec3::new(1.0, -0.95, -2.2);

/// The place of the view model's root, the hand, moved by `offset`.
pub fn view_model_transform(offset: HandOffset) -> Transform {
    Transform {
        translation: (HAND_REST + offset.translation) * VIEW_MODEL_SCALE,
        rotation: offset.rotation,
        scale: Vec3::splat(VIEW_MODEL_SCALE),
    }
}

/// What the view model is stretched by, across the view, for a camera whose field of view is
/// `fov` radians to show it as it does at `FOV_DEGREES`.
pub fn frame_scale(fov: f32) -> Vec3 {
    let across = (fov / 2.0).tan() / (FOV_DEGREES.to_radians() / 2.0).tan();
    Vec3::new(across, across, 1.0)
}

/// Whether a change of block by the local player, its right button down, is a use of its hand:
/// a block placed or turned, as a door opened, but not one it is breaking with the other button.
fn is_use(change: &BlockChanged) -> bool {
    let progress =
        |block: Option<shared::world::BlockData>| block.map_or(0, |block| block.breaking_progress);
    change.by_local_player
        && change.after.is_some()
        && progress(change.after) <= progress(change.before)
}

/// Whether the stack in the hand went from `before` to `after` by a use: some of it gone, as a
/// food eaten, or another item in its place, as a bucket filled.
fn is_used_up(before: Option<&ItemStack>, after: Option<&ItemStack>) -> bool {
    let Some(before) = before else {
        return false;
    };
    after.is_none_or(|after| after.item_id != before.item_id || after.nb < before.nb)
}

/// The size of a pixel of the skin on a player's model, in metres.
fn pixel() -> f32 {
    shared::players::PLAYER_HEIGHT / MODEL_PIXELS
}

/// The way the arm of the view model goes, from the hand to the shoulder: back toward the eye,
/// down and to the right, out of the corner of the screen.
const ARM_TO_SHOULDER: Vec3 = Vec3::new(0.5, -0.95, 0.55);

/// How much longer than the player's the arm of the view model is, so that its shoulder stays
/// out of the screen wherever a strike takes the hand.
const ARM_STRETCH: f32 = 1.8;

/// The arm of the view model, its hand at the root: the joint of the arm is at the shoulder, and
/// the end of the arm 10 pixels below it.
fn arm_transform() -> Transform {
    let rotation = Quat::from_rotation_arc(Vec3::Y, ARM_TO_SHOULDER.normalize())
        * Quat::from_rotation_y(-FRAC_PI_4);
    let scale = Vec3::new(1.0, ARM_STRETCH, 1.0);
    let hand = Vec3::new(0.0, -10.0, 0.0) * pixel() * scale;
    Transform::from_translation(-(rotation * hand))
        .with_rotation(rotation)
        .with_scale(scale)
}

/// How an item is placed in a hand held by `grip`, from the hand, as `look` draws it: a block
/// as a small cube, anything else as its picture, held by the bottom left of it, where the
/// pictures of the tools have their handle.
fn grip_transform(grip: Grip, item: ItemId, look: &ItemLook) -> Transform {
    // The point of a picture one wide where the hand holds it
    const HANDLE: Vec3 = Vec3::new(-0.3, -0.3, 0.0);
    let held_by_the_handle = |rotation: Quat, size: f32, hand: Vec3| {
        Transform::from_translation(hand - rotation * (HANDLE * size))
            .with_rotation(rotation)
            .with_scale(Vec3::splat(size))
    };
    if item == ItemId::Bow {
        // Held by the middle of its stave, which its picture has ahead, to the right
        let stave = Vec3::new(0.2, 0.0, 0.0);
        let (rotation, size) = match grip {
            // Upright, its string toward the eye, turned a little to be seen
            Grip::FirstPerson => (Quat::from_rotation_y(2.0), 1.0),
            // Its stave at the end of the arm, upright when the arm is raised ahead
            Grip::ThirdPerson => (
                Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X)),
                0.8,
            ),
        };
        return Transform::from_translation(-(rotation * (stave * size)))
            .with_rotation(rotation)
            .with_scale(Vec3::splat(size));
    }
    match (grip, look) {
        (Grip::FirstPerson, ItemLook::Cube(..)) => {
            Transform::from_translation(Vec3::new(-0.05, 0.12, -0.12))
                .with_rotation(Quat::from_rotation_y(FRAC_PI_4) * Quat::from_rotation_x(0.2))
                .with_scale(Vec3::splat(0.4))
        }
        // Turned to the eye, its back seen, the head of a tool up and to the left
        (Grip::FirstPerson, ItemLook::Sprite(..)) => held_by_the_handle(
            Quat::from_rotation_y(PI * 0.8) * Quat::from_rotation_z(-0.15),
            0.8,
            Vec3::new(0.0, 0.05, -0.05),
        ),
        (Grip::ThirdPerson, ItemLook::Cube(..)) => {
            Transform::from_translation(Vec3::new(0.0, -0.06, -0.14)).with_scale(Vec3::splat(0.25))
        }
        // On its side, pointing forward from the hand
        (Grip::ThirdPerson, ItemLook::Sprite(..)) => held_by_the_handle(
            Quat::from_rotation_x(-FRAC_PI_4) * Quat::from_rotation_y(FRAC_PI_2),
            0.5,
            Vec3::ZERO,
        ),
    }
}

/// The local player holds the item of its selected hotbar slot, as its inventory predicts it.
pub fn track_the_local_held_item(
    inventory: Res<Inventory>,
    hotbar: Query<&Hotbar>,
    mut players: Query<&mut HeldItem, With<CurrentPlayerMarker>>,
) {
    let Ok(hotbar) = hotbar.single() else {
        return;
    };
    let held = HeldItem(inventory.in_hand(hotbar.selected));
    for mut item in &mut players {
        item.set_if_neq(held);
    }
}

/// How far `player` draws the bow or loads the crossbow it holds in `held`, if it does
/// (`bow::draw_stage_of`).
fn draw_stage_of(player: &Player, held: Option<ItemId>) -> Option<u8> {
    shared::players::bow::draw_stage_of(held?, player.bow_draw_ms)
}

/// How far `player` draws what it holds in `held`, from 0 to 1.
fn power(player: &Player, held: Option<ItemId>) -> f32 {
    let full = held.and_then(full_draw_ms).unwrap_or(1000);
    power_of(player.bow_draw_ms, full)
}

/// Puts the item each player holds in the right hand of its model.
pub fn hold_items_in_hands(
    players: Query<(&Player, &HeldItem, &Children)>,
    children: Query<&Children>,
    mut holders: Query<&mut ItemHolder>,
) {
    for (player, held, player_children) in &players {
        let wanted = (
            held.item(),
            draw_stage_of(player, held.item()),
            held.0.is_some_and(|held| held.enchanted),
        );
        for child in player_children {
            for descendant in children.iter_descendants(*child) {
                if let Ok(mut holder) = holders.get_mut(descendant) {
                    if holder.grip == Grip::ThirdPerson
                        && (holder.wanted, holder.draw, holder.enchanted) != wanted
                    {
                        (holder.wanted, holder.draw, holder.enchanted) = wanted;
                    }
                }
            }
        }
    }
}

/// Gives the player's camera its view model, once the local player, whose skin the arm is cut
/// from, is there.
pub fn spawn_view_model(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut unlit: ResMut<ViewModelMaterials>,
    existing: Query<(), With<ViewModel>>,
    camera: Query<Entity, With<CameraController>>,
    player: Query<&SkinMaterial, With<CurrentPlayerMarker>>,
) {
    if !existing.is_empty() {
        return;
    }
    let (Ok(camera), Ok(skin)) = (camera.single(), player.single()) else {
        return;
    };
    // Those of the session before, whose skin is gone
    unlit.0.clear();
    let skin = unlit.unlit(&skin.0, &mut materials);
    let layers = RenderLayers::layer(VIEW_MODEL_LAYER);
    let (first, second) = BodyPart::RightArm.skin_origins();
    let size = BodyPart::RightArm.size();
    let centre = Transform::from_translation(Vec3::new(0.0, -4.0, 0.0) * pixel());
    let boxes = [
        meshes.add(box_mesh(first, size, 0.0, pixel())),
        meshes.add(box_mesh(second, size, LAYER_INFLATE, pixel())),
    ];
    let frame = commands
        .spawn((
            ViewModelFrame,
            Transform::from_scale(frame_scale(FOV_DEGREES.to_radians())),
            Visibility::default(),
            layers.clone(),
        ))
        .id();
    commands.entity(camera).add_child(frame);
    commands.entity(frame).with_children(|frame| {
        frame
            .spawn((
                ViewModel,
                HandMotion::default(),
                view_model_transform(lowering(1.0)),
                Visibility::Hidden,
                layers.clone(),
            ))
            .with_children(|root| {
                root.spawn((
                    ViewModelArm,
                    arm_transform(),
                    Visibility::default(),
                    layers.clone(),
                ))
                .with_children(|arm| {
                    for mesh in boxes {
                        arm.spawn((
                            Mesh3d(mesh),
                            MeshMaterial3d(skin.clone()),
                            centre,
                            layers.clone(),
                            NotShadowCaster,
                            NotShadowReceiver,
                        ));
                    }
                });
                root.spawn((
                    ItemHolder::new(Grip::FirstPerson),
                    Transform::default(),
                    Visibility::default(),
                    layers.clone(),
                ));
            });
    });
}

/// Moves the view model: down and up to change the item, with the walk, the strike of the arm
/// and the push of a use. It is seen in first person only, while the player lives.
#[allow(clippy::too_many_arguments)]
pub fn move_the_hand(
    time: Res<Time>,
    player: Query<(&Player, &PlayerAnimation, &HeldItem), With<CurrentPlayerMarker>>,
    (inventory, hotbar, needle): (Res<Inventory>, Query<&Hotbar>, Res<CompassNeedle>),
    mut last_stack: Local<Option<(u32, Option<ItemStack>)>>,
    mouse: Res<ButtonInput<MouseButton>>,
    ui_mode: Res<UIMode>,
    view_mode: Res<ViewMode>,
    mut block_changes: MessageReader<BlockChanged>,
    mut roots: Query<
        (&mut HandMotion, &mut Transform, &mut Visibility, &Children),
        With<ViewModel>,
    >,
    mut holders: Query<&mut ItemHolder>,
) {
    let mut used = block_changes.read().any(is_use);
    if let Ok(hotbar) = hotbar.single() {
        let stack = inventory.inner.get(&hotbar.selected);
        if let Some((slot, before)) = &*last_stack {
            used |= *slot == hotbar.selected && is_used_up(before.as_ref(), stack);
        }
        *last_stack = Some((hotbar.selected, stack.cloned()));
    }
    let Ok((player, animation, held)) = player.single() else {
        return;
    };
    let alive = !player.is_dead();
    used &= alive && *ui_mode == UIMode::Closed && mouse.pressed(MouseButton::Right);
    for (mut motion, mut transform, mut visibility, children) in &mut roots {
        visibility.set_if_neq(if alive && *view_mode == ViewMode::FirstPerson {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        motion.advance(held.item(), used, time.delta_secs());
        if let Some(held) = held.0.filter(|held| Some(held.item) == motion.shown) {
            motion.enchanted = held.enchanted;
        }
        let offset = hand_offset(
            &motion,
            animation.swing,
            animation.walk_phase,
            animation.walk_amount,
        );
        let draw = draw_stage_of(player, motion.shown);
        let offset = match draw {
            Some(_) => offset.then(draw_bow(
                power(player, motion.shown),
                time.elapsed_secs(),
            )),
            None => offset,
        };
        let draw = match motion.shown {
            Some(ItemId::DepthsCompass) => Some(needle.0),
            _ => draw,
        };
        *transform = view_model_transform(offset);
        let mut holders = holders.iter_many_mut(children);
        while let Some(mut holder) = holders.fetch_next() {
            let wanted = (motion.shown, draw, motion.enchanted);
            if (holder.wanted, holder.draw, holder.enchanted) != wanted {
                (holder.wanted, holder.draw, holder.enchanted) = wanted;
            }
        }
    }
}

/// Keeps the view model the same on screen as the field of view of the camera changes.
pub fn stretch_the_view_model(
    camera: Query<&Projection, With<CameraController>>,
    mut frames: Query<&mut Transform, With<ViewModelFrame>>,
) {
    let Ok(Projection::Perspective(perspective)) = camera.single() else {
        return;
    };
    for mut transform in &mut frames {
        let scale = frame_scale(perspective.fov);
        if transform.scale != scale {
            transform.scale = scale;
        }
    }
}

/// The ambient light of a clear day, at which the hand has its full colour, and how bright it
/// stays in the dark.
const FULL_AMBIENT: f32 = 400.0;
const DARKEST: f32 = 0.35;

/// How bright the unlit hand is drawn under an ambient light of `brightness`. Stepped, so that
/// its materials change a few times at dusk rather than on every frame.
fn hand_brightness(brightness: f32) -> f32 {
    let light = (brightness / FULL_AMBIENT).clamp(0.0, 1.0);
    let light = DARKEST + (1.0 - DARKEST) * light;
    (light * 64.0).round() / 64.0
}

/// Dims the unlit materials of the view model with the light of the day.
pub fn light_the_view_model(
    ambient: Res<GlobalAmbientLight>,
    unlit: Res<ViewModelMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let light = hand_brightness(ambient.brightness);
    let color = Color::linear_rgb(light, light, light);
    for handle in unlit.0.values() {
        if materials
            .get(handle)
            .is_some_and(|material| material.base_color != color)
        {
            if let Some(mut material) = materials.get_mut(handle) {
                material.base_color = color;
            }
        }
    }
}

/// Draws in each `ItemHolder` the item it wants, on its layers, once the atlases the looks of
/// the items are made from are there.
#[allow(clippy::too_many_arguments)]
pub fn draw_held_items(
    mut commands: Commands,
    holders: Query<(
        Entity,
        &ItemHolder,
        Option<&RenderLayers>,
        Option<&Children>,
    )>,
    drawn: Query<&HeldItemMesh>,
    mut looks: ResMut<ItemLooks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut unlit: ResMut<ViewModelMaterials>,
    mut glints: ResMut<Glints>,
    materials: Res<MaterialResource>,
    images: Res<Assets<Image>>,
) {
    for (entity, holder, layers, children) in &holders {
        let shown = children
            .into_iter()
            .flatten()
            .find_map(|child| drawn.get(*child).ok())
            .map(|mesh| (mesh.0, mesh.1, mesh.2));
        let wanted = holder
            .wanted
            .map(|item| (item, holder.draw, holder.enchanted));
        if shown == wanted {
            continue;
        }
        for child in children.into_iter().flatten() {
            if drawn.contains(*child) {
                commands.entity(*child).despawn();
            }
        }
        // A look that is not there yet is tried again on the next frame
        let Some((shown, look)) = wanted.and_then(|shown| {
            let look = looks.get_drawn((shown.0, shown.1), &materials, &mut meshes, &images)?;
            Some((shown, look))
        }) else {
            continue;
        };
        let layers = layers.cloned().unwrap_or_default();
        let transform = grip_transform(holder.grip, shown.0, &look);
        let glint = shown
            .2
            .then(|| glints.of(&look, layers.clone(), &mut standard_materials))
            .flatten();
        let mut mesh = match look {
            ItemLook::Cube(mesh, material) => {
                commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), transform, layers))
            }
            ItemLook::Sprite(mesh, material) => {
                let material = match holder.grip {
                    Grip::FirstPerson => unlit.unlit(&material, &mut standard_materials),
                    Grip::ThirdPerson => material,
                };
                commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), transform, layers))
            }
        };
        mesh.insert((
            HeldItemMesh(shown.0, shown.1, shown.2),
            Name::new("Held item"),
        ));
        if let Some(glint) = glint {
            mesh.with_child(glint);
        }
        if holder.grip == Grip::FirstPerson {
            mesh.insert((NotShadowCaster, NotShadowReceiver));
        }
        let mesh = mesh.id();
        commands.entity(entity).add_child(mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;

    fn run(motion: &mut HandMotion, held: Option<ItemId>, seconds: f32) {
        for _ in 0..(seconds / FRAME).round() as u32 {
            motion.advance(held, false, FRAME);
        }
    }

    fn close(a: HandOffset, b: HandOffset) -> bool {
        (a.translation - b.translation).length() < 1e-4
            && a.rotation.angle_between(b.rotation) < 1e-3
    }

    #[test]
    fn standing_still_the_hand_does_not_bob_and_walking_it_dips_at_each_step() {
        for phase in [0.0, 1.0, 2.5, 4.0] {
            assert_eq!(bob(phase, 0.0), HandOffset::NONE);
        }
        let mut lowest: f32 = 0.0;
        for step in 0..=64 {
            let offset = bob(step as f32 / 64.0 * TAU, 1.0).translation;
            assert!(
                offset.y <= 0.0,
                "the hand never rises above its rest: {offset}"
            );
            lowest = lowest.min(offset.y);
        }
        assert!((lowest + BOB_DIP).abs() < 1e-3, "{lowest}");
        // Back up between the steps, at either end of the stride, and to either side midway
        assert!(bob(0.0, 1.0).translation.length() < 1e-6);
        assert!(bob(PI, 1.0).translation.length() < 1e-6);
        assert!(bob(FRAC_PI_2, 1.0).translation.x > 0.0);
        assert!(bob(3.0 * FRAC_PI_2, 1.0).translation.x < 0.0);
    }

    #[test]
    fn a_strike_goes_forward_and_comes_back_to_rest() {
        assert!(close(strike(0.0), HandOffset::NONE));
        assert!(close(strike(1.0), HandOffset::NONE));
        let middle = strike(0.5);
        assert!(middle.translation.z < -0.5, "forward: {middle:?}");
        assert!(middle.translation.x < 0.0, "across the screen: {middle:?}");
        assert!(middle.rotation.angle_between(Quat::IDENTITY) > 0.5);
        // Quick at first: a quarter of the way in, the hand is already well across
        assert!(strike(0.25).translation.x < -0.35);
    }

    #[test]
    fn a_use_pushes_the_hand_forward_and_back() {
        assert!(close(push(0.0), HandOffset::NONE));
        assert!(close(push(1.0), HandOffset::NONE));
        assert!(push(0.5).translation.z < -0.5);
        assert!(push(0.25).translation.z > push(0.5).translation.z);
    }

    #[test]
    fn a_new_item_lowers_the_hand_which_takes_it_at_the_bottom_and_comes_back_up() {
        let mut motion = HandMotion::default();
        run(&mut motion, None, 0.5);
        assert_eq!(motion, HandMotion::default(), "an empty hand stays up");

        let pickaxe = Some(ItemId::WoodenPickaxe);
        run(&mut motion, pickaxe, EQUIP_SECONDS / 2.0);
        assert!(motion.lowered > 0.3 && motion.lowered < 0.7, "{motion:?}");
        assert_eq!(motion.shown, None, "the old item until the hand is down");
        run(&mut motion, pickaxe, EQUIP_SECONDS / 2.0 + FRAME);
        assert_eq!(motion.shown, pickaxe);
        run(&mut motion, pickaxe, EQUIP_SECONDS + FRAME);
        assert_eq!(motion.lowered, 0.0, "up again with it");

        // Changing back halfway down only goes down and up again
        run(&mut motion, Some(ItemId::Dirt), EQUIP_SECONDS / 2.0);
        run(&mut motion, pickaxe, EQUIP_SECONDS);
        assert_eq!(motion.shown, pickaxe);
        assert_eq!(motion.lowered, 0.0);
        // Lowered, the hand is out of sight
        assert!(
            (HAND_REST + lowering(1.0).translation).y < -2.0,
            "{:?}",
            lowering(1.0)
        );
    }

    #[test]
    fn a_use_runs_to_its_end_and_another_starts_it_again() {
        let mut motion = HandMotion::default();
        motion.advance(None, true, FRAME);
        assert_eq!(motion.using, Some(0.0));
        run(&mut motion, None, USE_SECONDS / 2.0);
        let halfway = motion.using.unwrap();
        assert!((halfway - 0.5).abs() < 0.05, "{halfway}");
        motion.advance(None, true, FRAME);
        assert_eq!(motion.using, Some(0.0), "a new use starts over");
        run(&mut motion, None, USE_SECONDS + FRAME);
        assert_eq!(motion.using, None);
    }

    #[test]
    fn placing_a_block_or_using_up_an_item_is_a_use_and_mining_is_not() {
        use shared::world::{BlockData, BlockDirection, BlockId};
        let block = BlockData::new(BlockId::Dirt, BlockDirection::Front);
        let mut mined = block;
        mined.breaking_progress = 2;
        let change = |before, after, by_local_player| BlockChanged {
            position: IVec3::ZERO,
            before,
            after,
            by_local_player,
        };
        assert!(is_use(&change(None, Some(block), true)));
        assert!(!is_use(&change(None, Some(block), false)), "another player");
        assert!(!is_use(&change(Some(block), Some(mined), true)), "mining");
        assert!(!is_use(&change(Some(mined), None, true)), "broken");

        let stack = |item_id: ItemId, nb| ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        let (one, two) = (stack(ItemId::Dirt, 1), stack(ItemId::Dirt, 2));
        assert!(is_used_up(Some(&two), Some(&one)));
        assert!(is_used_up(Some(&one), None));
        assert!(is_used_up(Some(&one), Some(&stack(ItemId::Stick, 1))));
        assert!(!is_used_up(Some(&one), Some(&two)), "picked up");
        assert!(!is_used_up(None, Some(&one)), "picked up");
    }

    #[test]
    fn the_hand_keeps_its_place_on_screen_whatever_the_field_of_view() {
        let on_screen = |fov: f32| {
            let point = frame_scale(fov) * (HAND_REST * VIEW_MODEL_SCALE);
            point.xy() / (-point.z * (fov / 2.0).tan())
        };
        let usual = on_screen(FOV_DEGREES.to_radians());
        assert_eq!(frame_scale(FOV_DEGREES.to_radians()), Vec3::ONE);
        for fov in [0.25, 0.5, 1.15, 110.0 / 60.0 * 1.1] {
            let there = on_screen(FOV_DEGREES.to_radians() * fov);
            assert!((there - usual).length() < 1e-5, "{there} {usual}");
        }
    }

    #[test]
    fn the_hand_dims_in_the_dark_and_never_goes_black() {
        assert_eq!(hand_brightness(FULL_AMBIENT), 1.0);
        assert_eq!(hand_brightness(10.0 * FULL_AMBIENT), 1.0);
        let night = hand_brightness(60.0);
        assert!(night > 0.3 && night < 0.6, "{night}");
    }

    #[test]
    fn the_moves_of_the_hand_add_up() {
        let motion = HandMotion {
            lowered: 0.5,
            using: Some(0.5),
            ..default()
        };
        let offset = hand_offset(&motion, Some(0.5), FRAC_PI_2, 1.0);
        let expected = lowering(0.5).translation
            + bob(FRAC_PI_2, 1.0).translation
            + strike(0.5).translation
            + push(0.5).translation;
        assert!((offset.translation - expected).length() < 1e-6);
        assert_eq!(
            hand_offset(&HandMotion::default(), None, 0.0, 0.0),
            HandOffset::NONE
        );
    }

    #[test]
    fn the_view_model_stays_nearer_to_the_eye_than_a_wall_can_be() {
        // A wall is at least half the width of a player from its eye
        let wall = shared::players::PLAYER_WIDTH / 2.0;
        // The hand, and the shoulder end of the arm, wherever the moves take them
        let arm = arm_transform();
        let shoulder = arm.transform_point(Vec3::new(0.0, 2.0, 0.0) * pixel());
        for t in [0.0, 0.25, 0.5, 0.75] {
            for offset in [strike(t), push(t), bob(t * TAU, 1.0)] {
                let root = view_model_transform(offset);
                for point in [Vec3::ZERO, shoulder, Vec3::new(-0.4, 0.5, -0.3)] {
                    let seen = root.transform_point(point);
                    assert!(seen.length() < wall, "{t} {offset:?}: {seen}");
                }
            }
        }
    }

    #[test]
    fn the_shoulder_never_comes_on_screen() {
        let tan = (FOV_DEGREES.to_radians() / 2.0).tan();
        let arm = arm_transform();
        for step in 0..=40 {
            let t = step as f32 / 40.0;
            for offset in [strike(t), push(t), bob(t * TAU, 1.0)] {
                let root = view_model_transform(offset);
                for (x, z) in [(-2.0, -2.0), (-2.0, 2.0), (2.0, -2.0), (2.0, 2.0)] {
                    let corner = arm.transform_point(Vec3::new(x, 2.0, z) * pixel());
                    let seen = root.transform_point(corner);
                    // Below the screen, or to the right of it, 16 wide by 9 high
                    let below = seen.y < tan * seen.z;
                    let right = seen.x > -tan * seen.z * 16.0 / 9.0;
                    assert!(below || right, "{t} {offset:?}: {seen}");
                }
            }
        }
    }

    #[test]
    fn no_portal_camera_draws_the_view_model() {
        use crate::portal::{camera_layers, OwnView};
        let view_model = RenderLayers::layer(VIEW_MODEL_LAYER);
        for own in [OwnView::Deeper(1), OwnView::Colour, OwnView::Hidden] {
            for slot in 0..4 {
                assert!(!camera_layers(slot, own, 0..4).intersects(&view_model));
            }
        }
    }

    #[test]
    fn a_tool_is_held_by_its_handle_and_a_block_as_a_small_cube() {
        let sprite = ItemLook::Sprite(Handle::default(), Handle::default());
        for grip in [Grip::FirstPerson, Grip::ThirdPerson] {
            let transform = grip_transform(grip, ItemId::Stick, &sprite);
            let handle = transform.transform_point(Vec3::new(-0.3, -0.3, 0.0));
            let hand = match grip {
                Grip::FirstPerson => Vec3::new(0.0, 0.05, -0.05),
                Grip::ThirdPerson => Vec3::ZERO,
            };
            assert!((handle - hand).length() < 1e-5, "{grip:?}: {handle}");
        }
        // In the hand of a model, a tool points forward
        let transform = grip_transform(Grip::ThirdPerson, ItemId::Stick, &sprite);
        let head = transform.transform_point(Vec3::new(0.4, 0.4, 0.0));
        assert!(head.z < -0.3 && head.y.abs() < 0.05, "{head}");

        let cube = ItemLook::Cube(Handle::default(), Handle::default());
        assert!(
            grip_transform(Grip::ThirdPerson, ItemId::Stick, &cube)
                .scale
                .x
                < 0.3
        );
    }
}
