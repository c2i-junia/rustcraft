use crate::camera::CameraController;
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_pressed;
use crate::mob::Mob;
use crate::network::buffered_client::CurrentFrameInputs;
use crate::portal::shot::ShotFired;
use crate::state::KeyMap;
use crate::ui::hud::{InventoryRoot, UIMode};
use crate::world::{BlockChanged, ClientWorldMap};
use bevy::color::palettes::css::WHITE;
use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;
use shared::messages::NetworkAction;
use shared::players::blocks::{
    aim, held_item, simulate_player_block_interactions, CallerType, PORTAL_GUN_REACH,
};
use shared::players::inventory::{
    simulate_inventory_actions, InventoryAction, InventoryActionOutcome,
};
use shared::players::{Inventory, Player, ViewMode};
use shared::world::{aabb_ray_hit, BlockId, FaceDirection, FaceDirectionExt, ItemId, Portals};

use super::CurrentPlayerMarker;

/// Outlines the block the player aims at and, while a mouse button is held on it, predicts
/// breaking it or placing a block against it, and reports the blocks that changed. A right
/// click on a crafting table, a chest or a furnace opens it instead, unless the player sneaks
/// or holds the portal gun, whose shots it fires (`ShotFired`).
/// Holding an empty bucket, the player aims at the water sources too.
pub fn handle_block_interactions(
    mut player_query: Query<(&mut Player, &Transform), With<CurrentPlayerMarker>>,
    camera_query: Query<&Transform, (With<CameraController>, Without<CurrentPlayerMarker>)>,
    world_map: ResMut<ClientWorldMap>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut pressed_in_ui: Local<Vec<MouseButton>>,
    (keyboard_input, key_map): (Res<ButtonInput<KeyCode>>, Res<KeyMap>),
    mobs: Query<(&Mob, &Transform), (Without<CameraController>, Without<CurrentPlayerMarker>)>,
    mut inventory_root: Query<&mut Visibility, With<InventoryRoot>>,
    ui_mode: Res<UIMode>,
    view_mode: Res<ViewMode>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut gizmos: Gizmos,
    mut block_changes: MessageWriter<BlockChanged>,
    mut shots: MessageWriter<ShotFired>,
) {
    // A button pressed over a UI, to close it for instance, is not for the world: it is ignored
    // until released
    if *ui_mode != UIMode::Closed {
        for button in mouse_input.get_pressed() {
            if !pressed_in_ui.contains(button) {
                pressed_in_ui.push(*button);
            }
        }
        frame_inputs.0.inputs.insert(NetworkAction::LowerBow);
        return;
    }
    pressed_in_ui.retain(|button| mouse_input.pressed(*button));
    let held = |button| mouse_input.pressed(button) && !pressed_in_ui.contains(&button);
    let (Ok((mut player, player_transform)), Ok(camera_transform)) =
        (player_query.single_mut(), camera_query.single())
    else {
        return;
    };
    let world_map = world_map.into_inner();

    // A right click, sneaking, on a donkey or a mule of the player opens the chest on its back
    let sneaking = is_action_pressed(GameAction::FlyDown, &keyboard_input, &key_map);
    if sneaking && mouse_input.just_pressed(MouseButton::Right) && !pressed_in_ui.contains(&MouseButton::Right) {
        let eye = match *view_mode {
            ViewMode::FirstPerson => camera_transform.translation,
            ViewMode::ThirdPerson => player_transform.translation,
        };
        let ray = Ray3d::new(eye, camera_transform.forward());
        let block = aim(
            &world_map.blocks,
            camera_transform,
            &player_transform.translation,
            *view_mode,
            None,
        )
        .map(|target| target.bbox);
        let pack = aimed_pack(ray, block, &player.name, mobs.iter());
        if let Some(id) = pack {
            frame_inputs.0.inventory_actions.push(InventoryAction::OpenPack(id));
            if let Ok(mut visibility) = inventory_root.single_mut() {
                *visibility = Visibility::Visible;
            }
            return;
        }
    }

    let Some(target) = aim(
        &world_map.blocks,
        camera_transform,
        &player_transform.translation,
        *view_mode,
        held_item(&player, &frame_inputs.0),
    ) else {
        // Food is eaten wherever the player looks, and a mob is hit in front of the sky
        for (button, action) in [
            (MouseButton::Left, NetworkAction::LeftClick),
            (MouseButton::Right, NetworkAction::RightClick),
        ] {
            if held(button) {
                frame_inputs.0.inputs.insert(action);
            }
        }
        // The server simulates every input, and so does the prediction: the buttons the
        // simulation remembers, which the portal gun shoots on the press of, stay the same
        let fired = simulate_player_block_interactions(
            &mut player,
            &mut world_map.blocks,
            &frame_inputs.0,
            CallerType::Client,
        )
        .shots;
        let shooter = player.id;
        shots.write_batch(
            fired
                .into_iter()
                .map(|shot| ShotFired::predicted(shooter, shot)),
        );
        return;
    };

    let holding = held_item(&player, &frame_inputs.0);
    if is_outlined(
        target.position,
        target.face,
        player_transform.translation,
        holding,
        world_map.blocks.portals(),
    ) {
        let center = (target.bbox.max + target.bbox.min) / 2.0;
        let size = target.bbox.max - target.bbox.min;
        gizmos.cube(
            Transform::from_translation(center.into()).with_scale(size.into()),
            WHITE,
        );
    }

    if held(MouseButton::Left) {
        frame_inputs.0.inputs.insert(NetworkAction::LeftClick);
    }
    // The simulation must see the sneak key, which decides between opening and placing
    if sneaking {
        frame_inputs.0.inputs.insert(NetworkAction::SneakOrFlyDown);
    }
    let block = world_map
        .blocks
        .get_block(target.position)
        .map(|block| block.id);
    let opens = opened(block, target.position, holding)
        .filter(|_| !sneaking && mouse_input.just_pressed(MouseButton::Right))
        // The click is for the villager in front of it, whom the server has trade
        .filter(|_| {
            let eye = match *view_mode {
                ViewMode::FirstPerson => camera_transform.translation,
                ViewMode::ThirdPerson => player_transform.translation,
            };
            let ray = Ray3d::new(eye, camera_transform.forward());
            !mobs
                .iter()
                .filter(|(mob, _)| mob.kind.is_villager())
                .any(|(mob, at)| is_before(ray, at.translation, mob.kind.size(), target.bbox))
        });
    if let Some(open) = opens {
        frame_inputs.0.inventory_actions.push(open);
        if let Ok(mut visibility) = inventory_root.single_mut() {
            *visibility = Visibility::Visible;
        }
    } else if held(MouseButton::Right) {
        frame_inputs.0.inputs.insert(NetworkAction::RightClick);
    }

    // The aimed block, and the cell a block placed against it takes
    let touched = [target.position, target.position + target.face.to_ivec3()];
    let before = touched.map(|position| world_map.blocks.get_block(position).copied());

    let fired = simulate_player_block_interactions(
        &mut player,
        &mut world_map.blocks,
        &frame_inputs.0,
        CallerType::Client,
    )
    .shots;
    let shooter = player.id;
    shots.write_batch(
        fired
            .into_iter()
            .map(|shot| ShotFired::predicted(shooter, shot)),
    );

    for (position, before) in touched.into_iter().zip(before) {
        let after = world_map.blocks.get_block(position).copied();
        if after != before {
            block_changes.write(BlockChanged {
                position,
                before,
                after,
                by_local_player: true,
            });
        }
    }
}

/// How far from the eye of the player a chest on the back of a donkey is opened, in blocks.
const PACK_REACH: f32 = 3.5;

/// The donkey or mule of the player named `name` carrying a chest that `ray` meets first, within
/// `PACK_REACH` and before `block`.
fn aimed_pack<'a>(
    ray: Ray3d,
    block: Option<Aabb3d>,
    name: &str,
    mobs: impl Iterator<Item = (&'a Mob, &'a Transform)>,
) -> Option<shared::world::MobId> {
    let direction = ray.direction.as_vec3();
    let distance = |hitbox: &Aabb3d| {
        aabb_ray_hit(hitbox, &ray.origin, &direction, &direction.recip())
            .map(|(hit, _)| hit.distance(ray.origin))
    };
    let block = block.as_ref().and_then(distance).unwrap_or(f32::INFINITY);
    mobs.filter_map(|(mob, at)| {
        let hit = distance(&Aabb3d::new(at.translation, mob.kind.size() * mob.scale / 2.0))?;
        Some((mob, hit))
    })
    .filter(|&(_, hit)| hit <= PACK_REACH && hit < block)
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(mob, _)| mob)
    .filter(|mob| mob.chested && mob.owner.as_deref() == Some(name))
    .map(|mob| mob.id)
}

/// Whether `ray` meets the box of `size` centred on `centre` before `block`.
fn is_before(ray: Ray3d, centre: Vec3, size: Vec3, block: Aabb3d) -> bool {
    let direction = ray.direction.as_vec3();
    let distance = |hitbox: &Aabb3d| {
        aabb_ray_hit(hitbox, &ray.origin, &direction, &direction.recip())
            .map(|(hit, _)| hit.distance(ray.origin))
    };
    distance(&Aabb3d::new(centre, size / 2.0))
        .is_some_and(|mob| distance(&block).is_none_or(|block| mob < block))
}

/// What a right click on `block`, at `position`, opens, the player holding `held`: a crafting
/// table, a chest or a furnace. Nothing with the portal gun held: the click shoots the orange
/// portal, at a container as at any block.
fn opened(
    block: Option<BlockId>,
    position: IVec3,
    held: Option<ItemId>,
) -> Option<InventoryAction> {
    if held == Some(ItemId::PortalGun) {
        return None;
    }
    match block {
        Some(BlockId::CraftingTable) => Some(InventoryAction::OpenCraftingTable(position)),
        Some(BlockId::EngravingTable) => Some(InventoryAction::OpenEngravingTable(position)),
        Some(BlockId::Anvil) => Some(InventoryAction::OpenAnvil(position)),
        Some(BlockId::Chest | BlockId::Barrel) => Some(InventoryAction::OpenChest(position)),
        Some(BlockId::Furnace | BlockId::LitFurnace) => {
            Some(InventoryAction::OpenFurnace(position))
        }
        _ => None,
    }
}

/// How far from the player an aimed block is outlined, but with the portal gun: as far as the
/// rays of every item but the gun go.
const OUTLINE_DISTANCE: f32 = 20.0;

/// Whether the block at `position`, aimed at on its face `face` by the player whose centre is
/// `player`, is outlined: within `OUTLINE_DISTANCE` of it, or the reach of the portal gun while
/// it is held, and not on the face of an open portal, where the view through it is seen.
/// Distances go from the centre of the player to that of the block, as the reaches do.
fn is_outlined(
    position: IVec3,
    face: FaceDirection,
    player: Vec3,
    held: Option<ItemId>,
    portals: &Portals,
) -> bool {
    let distance = if held == Some(ItemId::PortalGun) {
        PORTAL_GUN_REACH
    } else {
        OUTLINE_DISTANCE
    };
    (position.as_vec3() + 0.5).distance(player) <= distance && !portals.opens(position, face)
}

/// What one inventory action of the local player did, as predicted when the player made it.
/// Replays of the reconciliation are not reported: each action is reported once, which the
/// sounds and the shake of a refused slot rely on.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct InventoryActionPredicted(pub InventoryActionOutcome);

/// Predicts the inventory actions of the frame on the local player, shows the result, and
/// reports what each action did.
pub fn predict_inventory_system(
    mut player: Query<&mut Player, With<CurrentPlayerMarker>>,
    frame_inputs: Res<CurrentFrameInputs>,
    mut world_map: ResMut<ClientWorldMap>,
    mut inventory: ResMut<Inventory>,
    mut predicted: MessageWriter<InventoryActionPredicted>,
) {
    let Ok(mut player) = player.single_mut() else {
        return;
    };
    if frame_inputs.0.inventory_actions.is_empty() {
        return;
    }
    let outcomes = simulate_inventory_actions(
        &mut player,
        &mut world_map.blocks,
        &frame_inputs.0,
        CallerType::Client,
    );
    inventory.clone_from(&player.inventory);
    predicted.write_batch(outcomes.into_iter().map(InventoryActionPredicted));
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::inventory::{InventoryActionOutcome, InventoryRefusal};
    use shared::world::{ItemId, ItemStack, Portal, PortalColor};

    #[test]
    fn the_prediction_reports_each_inventory_action_of_the_frame() {
        let mut app = App::new();
        app.add_message::<InventoryActionPredicted>()
            .init_resource::<ClientWorldMap>()
            .init_resource::<CurrentFrameInputs>()
            .insert_resource(Inventory::new())
            .add_systems(Update, predict_inventory_system);
        let dirt = ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb: 5,
        };
        let mut player = Player::default();
        player.inventory.inner.insert(0, dirt);
        app.world_mut().spawn((player, CurrentPlayerMarker {}));
        let actions = &mut app
            .world_mut()
            .resource_mut::<CurrentFrameInputs>()
            .into_inner()
            .0
            .inventory_actions;
        actions.extend([
            InventoryAction::LeftClickSlot(0),
            InventoryAction::TakeCraftResult,
        ]);

        app.update();

        let reported: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<InventoryActionPredicted>>()
            .drain()
            .map(|predicted| predicted.0)
            .collect();
        assert_eq!(
            reported,
            [
                InventoryActionOutcome {
                    action: InventoryAction::LeftClickSlot(0),
                    held_before: None,
                    held_after: Some(dirt),
                    result: Ok(()),
                    thrown: None,
                    smelted: None,
                    dropped: Vec::new(),
                    crafted: None,
                },
                InventoryActionOutcome {
                    action: InventoryAction::TakeCraftResult,
                    held_before: Some(dirt),
                    held_after: Some(dirt),
                    result: Err(InventoryRefusal::NoRecipe),
                    thrown: None,
                    smelted: None,
                    dropped: Vec::new(),
                    crafted: None,
                },
            ]
        );
    }

    #[test]
    fn a_right_click_opens_a_container_but_with_the_portal_gun_held() {
        let at = IVec3::new(3, 64, -2);
        for (block, open) in [
            (
                BlockId::CraftingTable,
                InventoryAction::OpenCraftingTable(at),
            ),
            (BlockId::Chest, InventoryAction::OpenChest(at)),
            (BlockId::Furnace, InventoryAction::OpenFurnace(at)),
            (BlockId::LitFurnace, InventoryAction::OpenFurnace(at)),
        ] {
            for held in [None, Some(ItemId::Dirt), Some(ItemId::WaterBucket)] {
                assert_eq!(opened(Some(block), at, held), Some(open), "{block:?}");
            }
            // The gun shoots the orange portal instead
            assert_eq!(
                opened(Some(block), at, Some(ItemId::PortalGun)),
                None,
                "{block:?}"
            );
        }
        assert_eq!(opened(Some(BlockId::Stone), at, None), None);
    }

    #[test]
    fn the_aimed_block_is_outlined_within_20_blocks_or_the_reach_of_a_held_gun() {
        let player = Vec3::new(0.5, 0.5, 0.5);
        let none = Portals::default();
        let outlined = |distance: i32, held| {
            is_outlined(
                IVec3::new(0, 0, -distance),
                FaceDirection::PlusZ,
                player,
                held,
                &none,
            )
        };
        for held in [None, Some(ItemId::Dirt)] {
            assert!(outlined(19, held), "{held:?}");
            assert!(!outlined(21, held), "{held:?}");
            assert!(!outlined(31, held), "{held:?}");
        }
        let gun = Some(ItemId::PortalGun);
        assert!(outlined(31, gun));
        assert!(!outlined(33, gun));
    }

    #[test]
    fn the_face_of_an_open_portal_is_not_outlined() {
        let player = Vec3::new(0.5, 0.5, 5.5);
        let wall = IVec3::new(0, 0, 0);
        let mut portals = Portals::default();
        for color in [PortalColor::Blue, PortalColor::Orange] {
            portals.insert(Portal {
                owner: 1,
                color,
                anchor: IVec3::new(0, 0, if color == PortalColor::Blue { 0 } else { -10 }),
                face: FaceDirection::PlusZ,
                up: FaceDirection::PlusY,
            });
        }
        // The view through it is seen there, not the wall
        assert!(!is_outlined(
            wall,
            FaceDirection::PlusZ,
            player,
            None,
            &portals
        ));
        // Another face of the same block
        assert!(is_outlined(
            wall,
            FaceDirection::PlusX,
            player,
            None,
            &portals
        ));
        // A lone portal is a wall
        portals.remove(1, PortalColor::Orange);
        assert!(is_outlined(
            wall,
            FaceDirection::PlusZ,
            player,
            None,
            &portals
        ));
    }
}
