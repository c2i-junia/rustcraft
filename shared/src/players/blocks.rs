use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::Player,
    world::{
        global_block_to_chunk_pos, raycast, BlockData, BlockDirection, ChunkMap, FaceDirectionExt,
        ItemStack, ItemType,
    },
};
use bevy::math::{IVec3, NormedVectorSpace, Vec3};
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
    world_map: &mut ChunkMap,
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

/// Tells the server to re-state `pos` to the clients after it refused an interaction that a
/// client may have predicted there: the prediction is corrected whatever it was. A client
/// has nobody to correct it, so it does nothing.
fn report_refused(world_map: &mut ChunkMap, pos: IVec3, caller_type: CallerType) {
    if matches!(caller_type, CallerType::Server) {
        world_map.touch_block(pos);
    }
}

fn handle_block_breaking(
    player: &mut Player,
    world_map: &mut ChunkMap,
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
            "{} Player {} tried to break a block but no valid block was found | [Camera: {:?}, Player Pos: {:?}, ViewMode: {:?}]",
            caller_type.as_str(),
            player.id,
            action.camera,
            player.position,
            action.view_mode
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
        report_refused(world_map, block_pos, caller_type);
        return;
    }

    let Some(mut block) = world_map.get_block(block_pos).copied() else {
        log::info!(
            "{} Player {} tried to break a block at {:?}, but no block was found",
            caller_type.as_str(),
            player.id,
            block_pos
        );
        report_refused(world_map, block_pos, caller_type);
        return;
    };

    let Some(break_time) = block.id.get_break_time() else {
        log::debug!(
            "{} Player {} tried to break unbreakable block {:?} at {:?}",
            caller_type.as_str(),
            player.id,
            block.id,
            block_pos
        );
        report_refused(world_map, block_pos, caller_type);
        return;
    };

    // Try to break the block
    block.breaking_progress += 1;

    let destroyed = block.breaking_progress >= break_time;
    let block_id = block.id;
    let breaking_progress = block.breaking_progress;

    if destroyed {
        log::debug!(
            "{} Player {} broke block {:?} at position {:?}",
            caller_type.as_str(),
            player.id,
            block_id,
            block_pos
        );

        world_map.remove_block(block_pos);
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
        world_map.set_block(block_pos, block);
        log::debug!(
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
    world_map: &mut ChunkMap,
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
            "{} Player {} tried to place a block but no valid block was found | [Camera: {:?}, Player Pos: {:?}, ViewMode: {:?}]",
            caller_type.as_str(),
            player.id,
            action.camera,
            player.position,
            action.view_mode
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
        report_refused(world_map, block_to_create_pos, caller_type);
        return;
    }

    // Check if there's already a block at that position
    if world_map.get_block(block_to_create_pos).is_some() {
        log::warn!(
            "{} Player {} tried to place block at {:?} but a block already exists there",
            caller_type.as_str(),
            player.id,
            block_to_create_pos
        );
        report_refused(world_map, block_to_create_pos, caller_type);
        return;
    }

    // A block is never placed into terrain that is not loaded, and must not use up the item then
    if !world_map.is_chunk_loaded(global_block_to_chunk_pos(&block_to_create_pos)) {
        log::warn!(
            "{} Player {} tried to place block at {:?} but its chunk is not loaded",
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
        report_refused(world_map, block_to_create_pos, caller_type);
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
        report_refused(world_map, block_to_create_pos, caller_type);
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
            world_map.set_block(block_to_create_pos, block);

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
            report_refused(world_map, block_to_create_pos, caller_type);
        }
    } else {
        log::warn!(
            "{} Player {} tried to place block from empty inventory slot {}",
            caller_type.as_str(),
            player.id,
            inventory_slot
        );
        report_refused(world_map, block_to_create_pos, caller_type);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockId, ItemId};
    use bevy::math::IVec3;
    use bevy::prelude::Transform;

    const TARGET: IVec3 = IVec3::new(0, 0, -3);

    /// A player at the centre of block (0,0,0), looking down -Z at `TARGET`, left-clicking.
    fn left_click_on(id: BlockId) -> (Player, ChunkMap, PlayerFrameInput) {
        let position = Vec3::splat(0.5);
        let camera = Transform::from_translation(position);
        let player = Player::new(1, "tester".to_string(), position, camera);
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        world.fill_box(TARGET, TARGET, id);
        world.take_changed_blocks();
        let input = PlayerFrameInput {
            inputs: [NetworkAction::LeftClick].into(),
            camera,
            ..Default::default()
        };
        (player, world, input)
    }

    #[test]
    fn breaking_a_regular_block_takes_its_break_time() {
        let (mut player, mut world, input) = left_click_on(BlockId::Dandelion);
        let steps = BlockId::Dandelion.get_break_time().unwrap();
        for _ in 1..steps {
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
            assert!(world.get_block(TARGET).is_some());
        }
        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        assert!(world.get_block(TARGET).is_none());
        assert_eq!(world.take_changed_blocks(), vec![TARGET]);
    }

    #[test]
    fn each_hit_stores_the_breaking_progress_and_records_the_change() {
        let (mut player, mut world, input) = left_click_on(BlockId::Stone);
        assert!(BlockId::Stone.get_break_time().unwrap() > 2);
        for expected in 1..=2 {
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
            assert_eq!(world.get_block(TARGET).unwrap().breaking_progress, expected);
            assert_eq!(world.take_changed_blocks(), vec![TARGET]);
        }
    }

    #[test]
    fn mining_bedrock_changes_nothing_and_the_server_restates_it() {
        let (mut player, mut world, input) = left_click_on(BlockId::Bedrock);
        for _ in 0..1000 {
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        }
        assert_eq!(world.get_block(TARGET).unwrap().id, BlockId::Bedrock);
        assert_eq!(world.get_block(TARGET).unwrap().breaking_progress, 0);
        // the server re-states the block to the clients that may have predicted otherwise
        assert_eq!(world.take_changed_blocks(), vec![TARGET]);
    }

    #[test]
    fn a_client_refusing_an_interaction_reports_nothing() {
        let (mut player, mut world, input) = left_click_on(BlockId::Bedrock);
        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Client);
        assert!(world.take_changed_blocks().is_empty());
    }

    #[test]
    fn breaking_a_block_that_is_too_far_reports_it() {
        let (mut player, mut world, input) = left_click_on(BlockId::Stone);
        let far = IVec3::new(0, 0, -8);
        world.fill_box(far, far, BlockId::Stone);
        world.take_changed_blocks();
        world.remove_block(TARGET);
        world.take_changed_blocks();

        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);

        assert_eq!(world.get_block(far).unwrap().breaking_progress, 0);
        assert_eq!(world.take_changed_blocks(), vec![far]);
    }

    /// A player two blocks behind `TARGET`'s neighbour, holding dirt, right-clicking the face of
    /// a block at z = -1 that looks toward block (0,0,0): the new block goes in the next chunk.
    fn place_across_a_chunk_border(world: &mut ChunkMap) -> Player {
        let position = Vec3::new(0.5, 0.5, 2.5);
        let camera = Transform::from_translation(position);
        let mut player = Player::new(1, "tester".to_string(), position, camera);
        player.inventory.inner.insert(
            0,
            ItemStack {
                item_id: ItemId::Dirt,
                item_type: ItemId::Dirt.get_default_type(),
                nb: 1,
            },
        );
        world.fill_box(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1), BlockId::Stone);
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };
        simulate_player_block_interactions(&mut player, world, &input, CallerType::Server);
        player
    }

    #[test]
    fn placing_a_block_across_a_chunk_border_uses_the_item() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
        let player = place_across_a_chunk_border(&mut world);
        assert_eq!(
            world.get_block(IVec3::ZERO).map(|b| b.id),
            Some(BlockId::Dirt)
        );
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn placing_where_the_player_stands_reports_the_refused_position() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
        // the player's feet are inside the cell the block would take
        let position = Vec3::new(0.5, 0.5, 0.5);
        let camera = Transform::from_translation(position + Vec3::new(0.0, 0.0, 2.0))
            .looking_at(Vec3::new(0.5, 0.5, -0.5), Vec3::Y);
        let mut player = Player::new(1, "tester".to_string(), position, camera);
        player.inventory.inner.insert(
            0,
            ItemStack {
                item_id: ItemId::Dirt,
                item_type: ItemId::Dirt.get_default_type(),
                nb: 1,
            },
        );
        world.fill_box(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1), BlockId::Stone);
        world.take_changed_blocks();
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };

        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);

        assert_eq!(world.get_block(IVec3::ZERO), None);
        assert_eq!(player.inventory.inner[&0].nb, 1);
        assert_eq!(world.take_changed_blocks(), vec![IVec3::ZERO]);
    }

    #[test]
    fn placing_a_block_into_an_unloaded_chunk_keeps_the_item() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1));
        let player = place_across_a_chunk_border(&mut world);
        assert_eq!(world.get_block(IVec3::ZERO), None);
        assert_eq!(player.inventory.inner[&0].nb, 1);
    }
}
