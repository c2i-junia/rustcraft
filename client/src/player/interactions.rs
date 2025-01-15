use crate::camera::*;
use crate::constants::{CUBE_SIZE, INTERACTION_DISTANCE};
use crate::network::api::send_network_action;
use crate::network::api::NetworkAction;
use crate::player::inventory::*;
use crate::player::spawn::Player;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::UIMode;
use crate::world::ClientWorldMap;
use crate::world::WorldRenderRequestUpdateEvent;
use bevy::math::NormedVectorSpace;
use bevy::prelude::*;
use bevy_mod_raycast::prelude::*;
use bevy_renet::renet::RenetClient;
use shared::world::{BlockData, ItemStack, ItemType};

use super::CurrentPlayerMarker;

// Helper function to snap a Vec3 position to the grid
fn snap_to_grid(position: Vec3) -> Vec3 {
    Vec3::new(position.x.round(), position.y.round(), position.z.round())
}

// Function to handle block placement and breaking
pub fn handle_block_interactions(
    queries: (
        Query<&Player, With<CurrentPlayerMarker>>,
        Query<&mut Transform, With<CurrentPlayerMarker>>,
        Query<&RaycastSource<BlockRaycastSet>>,
        Query<&Hotbar>,
    ),
    resources: (
        ResMut<ClientWorldMap>,
        Res<ButtonInput<MouseButton>>,
        Res<UIMode>,
        ResMut<Inventory>,
        ResMut<RenetClient>,
    ),
    mut ev_render: EventWriter<WorldRenderRequestUpdateEvent>,
) {
    let (player_query, mut p_transform, raycast_source, hotbar) = queries;
    let (mut world_map, mouse_input, ui_mode, mut inventory, mut client) = resources;

    let player = player_query.single().clone();

    if *ui_mode == UIMode::Opened {
        return;
    }

    let raycast_source = raycast_source.single();

    // Handle left-click for breaking blocks
    if mouse_input.just_pressed(MouseButton::Left) {
        debug!("Left click : {}", raycast_source.intersections().len());
        // Check if there are any intersections with a block
        if let Some((_, intersection)) = raycast_source.intersections().first() {
            // Check if block is close enough to the player
            if (intersection.position() - p_transform.single_mut().translation).norm()
                < INTERACTION_DISTANCE
            {
                let block_pos = intersection.position() - intersection.normal() / 10.;
                let global_block_coords = IVec3::new(
                    block_pos.x.floor() as i32,
                    block_pos.y.floor() as i32,
                    block_pos.z.floor() as i32,
                );

                // Remove the hit block
                let block = world_map.remove_block_by_coordinates(&global_block_coords);

                if let Some(block) = block {
                    // add the block to the player's inventory

                    // If block has corresponding item, add it to inventory
                    for (item_id, nb) in block.id.get_drops(1) {
                        inventory.add_item_to_inventory(ItemStack {
                            item_id,
                            item_type: item_id.get_default_type(),
                            nb,
                        });
                    }

                    ev_render.send(WorldRenderRequestUpdateEvent::BlockToReload(
                        global_block_coords,
                    ));

                    // Send the bloc to the serveur to delete it
                    send_network_action(
                        &mut client,
                        NetworkAction::BlockInteraction {
                            position: global_block_coords,
                            block_type: None, // None signify suppression
                        },
                    );
                }
            }
        }
    }

    // Handle right-click for placing blocks
    if mouse_input.just_pressed(MouseButton::Right) {
        if let Some((_entity, intersection)) = raycast_source.intersections().first() {
            let block_pos = intersection.position() - intersection.normal() * (CUBE_SIZE / 2.);
            let global_block_coords = IVec3::new(
                block_pos.x.floor() as i32,
                block_pos.y.floor() as i32,
                block_pos.z.floor() as i32,
            );

            // Get the normal of the face where the block will be placed
            let normal = intersection.normal(); // This is already a Vec3, no need to unwrap
                                                // Calculate the block position by adding a small offset to the intersection point
            let mut position = global_block_coords.as_vec3() + normal * 0.51;
            // Snap the position to the grid
            position = snap_to_grid(position);

            // Difference vector between player position and block center
            let distance = position + (Vec3::new(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE) / 2.)
                - p_transform.single_mut().translation;

            // Check if target space is close enough to the player
            if (intersection.position() - p_transform.single_mut().translation).norm()
                <= INTERACTION_DISTANCE
                // Guarantees a block cannot be placed too close to the player (which would be unable to move because of constant collision)
                && (distance.x.abs() > (CUBE_SIZE + player.width) / 2. || distance.z.abs() > (CUBE_SIZE + player.width ) / 2. || distance.y.abs() > (CUBE_SIZE + player.height) / 2.)
            {
                // Try to get item currently selected in player hotbar
                if let Some(&item) = inventory.inner.get(&hotbar.single().selected) {
                    inventory.remove_item_from_stack(hotbar.single().selected, 1);

                    // Check if the item has a block counterpart
                    if let ItemType::Block(block_id) = item.item_type {
                        let block_pos =
                            IVec3::new(position.x as i32, position.y as i32, position.z as i32);
                        let block =
                            BlockData::new(block_id, false, shared::world::BlockDirection::Front);

                        world_map.set_block(&block_pos, block);

                        ev_render.send(WorldRenderRequestUpdateEvent::BlockToReload(block_pos));

                        // Send to server the bloc to add
                        send_network_action(
                            &mut client,
                            NetworkAction::BlockInteraction {
                                position: block_pos,
                                block_type: Some(block), // Some signify adding
                            },
                        );
                    }
                }
            }
        }
    }
}
