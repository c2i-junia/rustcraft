use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::Player,
    world::{raycast, BlockData, BlockDirection, FaceDirectionExt, ItemStack, ItemType, WorldMap},
};
use bevy::math::{NormedVectorSpace, Vec3};
use bevy_log::info;

#[derive(Debug, Clone, Copy)]
pub enum CallerType {
    Client,
    Server,
}

impl CallerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallerType::Client => "[CLIENT]",
            CallerType::Server => "[SERVER]",
        }
    }
}

const INTERACTION_DISTANCE: f32 = 5.0;
const CUBE_SIZE: f32 = 1.0;

pub fn simulate_player_block_interactions(
    player: &mut Player,
    world_map: &mut impl WorldMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) {
    // TODO: make sure that only one interaction is processed per game tick (instead of per frame like now)
    for network_action in &action.inputs {
        match network_action {
            NetworkAction::LeftClick => {
                handle_block_breaking(player, world_map, action, caller_type);
            }
            NetworkAction::RightClick => {
                handle_block_placement(player, world_map, action, caller_type);
            }
            _ => {}
        }
    }
}

fn handle_block_breaking(
    player: &mut Player,
    world_map: &mut impl WorldMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) {
    let block_position = raycast::raycast(
        world_map,
        &action.camera,
        &player.position,
        action.view_mode,
    );

    log::debug!(
        "{} Player {} is trying to break block is at {:?}",
        caller_type.as_str(),
        player.id,
        block_position,
    );

    if block_position.is_none() {
        log::info!(
            "{} Player {} tried to break a block but no valid block was found",
            caller_type.as_str(),
            player.id,
        );
        return;
    }

    let block_pos = block_position.unwrap().position;

    let distance = (block_pos.as_vec3() + Vec3::splat(0.5) - player.position).norm();
    log::debug!(
        "{} Calculated distance to block center: {:.2} (block pos: {:?}, player pos: {:?})",
        caller_type.as_str(),
        distance,
        block_pos,
        player.position
    );

    // Validate interaction distance
    if distance > INTERACTION_DISTANCE {
        log::warn!(
            "{} Player {} tried to break block at {:?} but it's too far (distance: {:.2})",
            caller_type.as_str(),
            player.id,
            block_pos,
            distance
        );
        return;
    }

    let block = world_map.get_block_mut_by_coordinates(&block_pos);
    if block.is_none() {
        log::info!(
            "{} Player {} tried to break a block at {:?}, but no block was found",
            caller_type.as_str(),
            player.id,
            block_pos
        );
        return;
    }
    let block = block.unwrap();

    // Try to break the block
    block.breaking_progress += 1;

    let destroyed = block.breaking_progress >= block.id.get_break_time();
    let block_id = block.id;
    let breaking_progress = block.breaking_progress;
    let break_time = block.id.get_break_time();

    if destroyed {
        info!(
            "{} Player {} broke block {:?} at position {:?}",
            caller_type.as_str(),
            player.id,
            block_id,
            block_pos
        );

        world_map.remove_block_by_coordinates(&block_pos);
        // Add drops to player inventory
        for (item_id, nb) in block_id.get_drops(1) {
            player.inventory.add_item_to_inventory(ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb,
            });
            info!(
                "{} Player {} received drop {:?} x{} from breaking block {:?}",
                caller_type.as_str(),
                player.id,
                item_id,
                nb,
                block_id
            );
        }
    } else {
        world_map.mark_block_for_update(&block_pos);
        info!(
            "{} Player {} is breaking block {:?} at position {:?} (progress: {}/{})",
            caller_type.as_str(),
            player.id,
            block_id,
            block_pos,
            breaking_progress,
            break_time
        );
    }
}

fn handle_block_placement(
    player: &mut Player,
    world_map: &mut impl WorldMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) {
    let raycast_response = raycast::raycast(
        world_map,
        &action.camera,
        &player.position,
        action.view_mode,
    );

    log::debug!(
        "{} Player {} is trying to place block is at {:?}",
        caller_type.as_str(),
        player.id,
        raycast_response,
    );

    if raycast_response.is_none() {
        log::info!(
            "{} Player {} tried to place a block but no valid block was found",
            caller_type.as_str(),
            player.id,
        );
        return;
    }

    let raycast_response = raycast_response.unwrap();

    let collision_pos = raycast_response.position;
    let face_direction = raycast_response.face;

    log::debug!(
        "{} Player {} is trying to place block at {:?} on face {:?}",
        caller_type.as_str(),
        player.id,
        collision_pos,
        face_direction
    );

    let face = raycast_response.face.to_ivec3();

    let block_to_create_pos = collision_pos + face;

    let block_to_create_pos_vec3 = Vec3::new(
        (collision_pos.x + face.x) as f32,
        (collision_pos.y + face.y) as f32,
        (collision_pos.z + face.z) as f32,
    );

    let unit_cube = Vec3::new(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE);

    let target_cube_center = block_to_create_pos_vec3 + (unit_cube / 2.);

    let distance = (collision_pos.as_vec3() + Vec3::splat(0.5) - player.position).norm();

    // Validate interaction distance
    if distance > INTERACTION_DISTANCE {
        log::warn!(
            "{} Player {} tried to place block at {:?} but it's too far (distance: {:.2})",
            caller_type.as_str(),
            player.id,
            collision_pos,
            distance
        );
        return;
    }

    // Check if there's already a block at that position
    if world_map
        .get_block_by_coordinates(&block_to_create_pos)
        .is_some()
    {
        log::warn!(
            "{} Player {} tried to place block at {:?} but a block already exists there",
            caller_type.as_str(),
            player.id,
            block_to_create_pos
        );
        return;
    }

    let delta = player.position - target_cube_center;
    let distance = delta.abs();

    log::debug!(
        "{} Calculated distance to target cube center: {:?} (block pos: {:?}, player pos: {:?})",
        caller_type.as_str(),
        distance,
        block_to_create_pos,
        player.position
    );

    // Validate that block placement won't collide with player
    if !(distance.x > (CUBE_SIZE + player.width) / 2.
        || distance.z > (CUBE_SIZE + player.width) / 2.
        || distance.y > (CUBE_SIZE + player.height) / 2.)
    {
        log::warn!(
            "{} Player {} tried to place block at {:?} but it would collide with player",
            caller_type.as_str(),
            player.id,
            block_to_create_pos
        );
        return;
    }

    let inventory_slot = action.hotbar_slot;

    // Validate hotbar slot
    if inventory_slot >= crate::MAX_INVENTORY_SLOTS {
        log::warn!(
            "{} Player {} tried to place block from invalid inventory slot {}",
            caller_type.as_str(),
            player.id,
            inventory_slot
        );
        return;
    }

    // Try to get item from player's inventory
    if let Some(&item) = player.inventory.inner.get(&inventory_slot) {
        // Check if the item has a block counterpart
        if let ItemType::Block(block_id) = item.item_type {
            // Remove item from inventory
            player.inventory.remove_item_from_stack(inventory_slot, 1);

            // Place the block
            let block = BlockData::new(block_id, BlockDirection::Front);
            world_map.set_block(&block_to_create_pos, block);

            log::info!(
                "{} Player {} placed block {:?} at position {:?}",
                caller_type.as_str(),
                player.id,
                block_id,
                block_to_create_pos
            );
        } else {
            log::warn!(
                "{} Player {} tried to place item {:?} but it's not a block",
                caller_type.as_str(),
                player.id,
                item.item_type
            );
        }
    } else {
        log::warn!(
            "{} Player {} tried to place block from empty inventory slot {}",
            caller_type.as_str(),
            player.id,
            inventory_slot
        );
    }
}
