use crate::constants::{CUBE_SIZE, INTERACTION_DISTANCE};
use crate::mob::{MobMarker, TargetedMob, TargetedMobData};
use crate::network::SendGameMessageExtension;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::UIMode;
use crate::world::{raycast, ClientWorldMap};
use crate::world::{FaceDirectionExt, WorldRenderRequestUpdateEvent};
use bevy::color::palettes::css::{self, WHITE};
use bevy::math::NormedVectorSpace;
use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use shared::messages::ClientToServerMessage;
use shared::players::{Inventory, Player};
use shared::world::{BlockData, ItemStack, ItemType, WorldMap};

use super::{CurrentPlayerMarker, ViewMode};

// Function to handle block placement and breaking
pub fn handle_block_interactions(
    queries: (
        Query<&Player, With<CurrentPlayerMarker>>,
        Query<&mut Transform, With<CurrentPlayerMarker>>,
        Query<&Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
        Query<&Hotbar>,
        Query<&MobMarker>,
    ),
    resources: (
        ResMut<ClientWorldMap>,
        Res<ButtonInput<MouseButton>>,
        Res<UIMode>,
        ResMut<Inventory>,
        ResMut<RenetClient>,
        Res<ViewMode>,
        ResMut<TargetedMob>,
    ),
    mut ev_render: EventWriter<WorldRenderRequestUpdateEvent>,
    mut ray_cast: MeshRayCast,
    mut commands: Commands,
    mut gizmos: Gizmos,
) {
    let (player_query, mut p_transform, camera_query, hotbar, mob_query) = queries;
    let (
        mut world_map,
        mouse_input,
        ui_mode,
        mut inventory,
        mut client,
        view_mode,
        mut targeted_mob,
    ) = resources;

    let player = player_query.single().clone();

    if *ui_mode == UIMode::Opened {
        return;
    }

    let camera_transform = camera_query.single();
    let player_transform = p_transform.single();

    let ray = Ray3d::new(camera_transform.translation, camera_transform.forward());

    let maybe_block = raycast(&world_map, camera_transform, player_transform, *view_mode);

    // bounce_ray(ray, &mut ray_cast);

    // if let Some((entity, _)) = ray_cast.cast_ray(ray, &RayCastSettings::default()).first() {
    //     let mob = mob_query.get(*entity);
    //     if let Ok(mob) = mob {
    //         targeted_mob.target = Some(TargetedMobData {
    //             entity: *entity,
    //             id: mob.id,
    //             name: mob.name.clone(),
    //         });
    //     } else {
    //         targeted_mob.target = None;
    //     }
    // } else {
    //     targeted_mob.target = None;
    // }

    if mouse_input.just_pressed(MouseButton::Left) && targeted_mob.target.is_some() {
        // TODO: Attack the targeted

        let target = targeted_mob.target.as_ref().unwrap();

        commands.entity(target.entity).despawn_recursive();

        targeted_mob.target = None;

        return;
    }

    if let Some(res) = maybe_block {
        // Draw gizmos for the bounding box
        let center = (res.bbox.max + res.bbox.min) / 2.0;
        let hsize = res.bbox.max - res.bbox.min;
        gizmos.cuboid(
            Transform::from_translation(center.into()).with_scale(hsize.into()),
            WHITE,
        );

        // Handle left-click for breaking blocks
        if mouse_input.pressed(MouseButton::Left) {
            let pos = res.position;
            let block_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            // Check if block is close enough to the player
            if (block_pos - p_transform.single_mut().translation).norm() < INTERACTION_DISTANCE {
                // Remove the hit block
                let res = world_map.try_to_break_block(&pos);

                if let Some((block, destroyed)) = res {
                    ev_render.send(WorldRenderRequestUpdateEvent::BlockToReload(pos));

                    if destroyed {
                        // If block has corresponding item, add it to inventory
                        for (item_id, nb) in block.id.get_drops(1) {
                            inventory.add_item_to_inventory(ItemStack {
                                item_id,
                                item_type: item_id.get_default_type(),
                                nb,
                            });
                        }

                        client.send_game_message(ClientToServerMessage::BlockInteraction {
                            position: pos,
                            block_type: None,
                        });
                    }
                }
            }
        }

        // Handle right-click for placing blocks
        if mouse_input.just_pressed(MouseButton::Right) {
            let face_dir = res.face;
            let collision_pos = res.position;

            let face = face_dir.to_ivec3();
            // Check if target space is close enough to the player
            let block_to_create_pos = Vec3::new(
                (collision_pos.x + face.x) as f32,
                (collision_pos.y + face.y) as f32,
                (collision_pos.z + face.z) as f32,
            );

            let unit_cube = Vec3::new(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE);
            let player_position = p_transform.single_mut().translation;

            let target_cube_center = block_to_create_pos + (unit_cube / 2.);

            // Difference vector between player position and block center
            let distance = (target_cube_center - player_position).abs();

            debug!("Distance: {:?}", distance);
            debug!("Block position: {:?}", block_to_create_pos);
            debug!("Player position: {:?}", player_position);
            debug!("Target cube center: {:?}", target_cube_center);

            if (block_to_create_pos - p_transform.single_mut().translation).norm()
                <= INTERACTION_DISTANCE
                // Guarantees a block cannot be placed too close to the player (which would be unable to move because of constant collision)
                && (distance.x> (CUBE_SIZE + player.width) / 2. || distance.z > (CUBE_SIZE + player.width ) / 2. || distance.y > (CUBE_SIZE + player.height) / 2.)
            {
                // Try to get item currently selected in player hotbar
                if let Some(&item) = inventory.inner.get(&hotbar.single().selected) {
                    inventory.remove_item_from_stack(hotbar.single().selected, 1);

                    // Check if the item has a block counterpart
                    if let ItemType::Block(block_id) = item.item_type {
                        let block_pos = IVec3::new(
                            block_to_create_pos.x as i32,
                            block_to_create_pos.y as i32,
                            block_to_create_pos.z as i32,
                        );
                        let block =
                            BlockData::new(block_id, false, shared::world::BlockDirection::Front);

                        world_map.set_block(&block_pos, block);

                        ev_render.send(WorldRenderRequestUpdateEvent::BlockToReload(block_pos));

                        client.send_game_message(ClientToServerMessage::BlockInteraction {
                            position: block_pos,
                            block_type: Some(block),
                        });
                    }
                }
            }
        }
    }
}

const MAX_BOUNCES: usize = 1;

// Bounces a ray off of surfaces `MAX_BOUNCES` times.
fn bounce_ray(mut ray: Ray3d, ray_cast: &mut MeshRayCast) {
    return;
    let color = Color::from(css::GREEN);

    let mut intersections = Vec::with_capacity(MAX_BOUNCES + 1);
    intersections.push((ray.origin, Color::srgb(30.0, 0.0, 0.0)));

    for i in 0..MAX_BOUNCES {
        // Cast the ray and get the first hit
        let Some((_, hit)) = ray_cast.cast_ray(ray, &RayCastSettings::default()).first() else {
            break;
        };

        // debug!("Hit: {:?} {:?}", entity, hit);

        // Draw the point of intersection and add it to the list
        let brightness = 1.0 + 10.0 * (1.0 - i as f32 / MAX_BOUNCES as f32);
        intersections.push((hit.point, Color::BLACK.mix(&color, brightness)));

        // Reflect the ray off of the surface
        ray.direction = Dir3::new(ray.direction.reflect(hit.normal)).unwrap();
        ray.origin = hit.point + ray.direction * 1e-6;
    }
}
