//! Breaking and placing blocks, run by the client (prediction) and by the server (authority).

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::Player,
    world::{
        global_block_to_chunk_pos, raycast, BlockData, BlockDirection, BlockId, ChunkMap,
        FaceDirectionExt, ItemStack, ItemType, RaycastResponse,
    },
    HALF_BLOCK, MAX_INVENTORY_SLOTS,
};
use bevy::math::{IVec3, Vec3};

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

/// Why an interaction changed nothing.
#[derive(Debug, PartialEq)]
enum Refusal {
    /// The player aims at no block.
    NoTarget,
    TooFar {
        distance: f32,
    },
    Unbreakable(BlockId),
    /// The cell the new block would take already holds one.
    Occupied,
    /// The cell the new block would take is in terrain that is not loaded.
    ChunkNotLoaded,
    /// The new block would overlap the player.
    InsidePlayer,
    InvalidSlot(u32),
    EmptySlot(u32),
    NotABlock(ItemType),
}

/// A refused interaction, and the block a client may have predicted a change at.
#[derive(Debug, PartialEq)]
struct Refused {
    reason: Refusal,
    predicted_at: Option<IVec3>,
}

impl Refused {
    fn at(position: IVec3, reason: Refusal) -> Self {
        Self {
            reason,
            predicted_at: Some(position),
        }
    }
}

pub fn simulate_player_block_interactions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) {
    // Interactions are processed once per input, so once per rendered frame of the client
    // and not once per tick: a faster client breaks blocks faster.
    for network_action in &action.inputs {
        let (verb, outcome) = match network_action {
            NetworkAction::LeftClick => ("break", break_block(player, world_map, action)),
            NetworkAction::RightClick => ("place", place_block(player, world_map, action)),
            _ => continue,
        };
        let Err(refused) = outcome else {
            continue;
        };
        log::debug!(
            "{} Player {} could not {verb} a block: {:?}",
            caller_type.as_str(),
            player.id,
            refused.reason
        );
        // The server re-states the block to the clients, which corrects whatever one of them
        // predicted there. A client has nobody to correct, so it does nothing.
        if let (Some(position), CallerType::Server) = (refused.predicted_at, caller_type) {
            world_map.touch_block(position);
        }
    }
}

/// The block the player aims at, if it is within reach. An out of reach refusal names
/// `predicted_at(target)` as the block to re-state.
fn aimed_block(
    player: &Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
    predicted_at: impl Fn(&RaycastResponse) -> IVec3,
) -> Result<RaycastResponse, Refused> {
    let target = raycast::raycast(
        world_map,
        &action.camera,
        &player.position,
        action.view_mode,
    )
    .ok_or(Refused {
        reason: Refusal::NoTarget,
        predicted_at: None,
    })?;

    let distance = (target.position.as_vec3() + HALF_BLOCK).distance(player.position);
    if distance > INTERACTION_DISTANCE {
        return Err(Refused::at(
            predicted_at(&target),
            Refusal::TooFar { distance },
        ));
    }
    Ok(target)
}

/// One hit on the aimed block: it breaks once it took as many hits as its break time.
fn break_block(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<(), Refused> {
    let position = aimed_block(player, world_map, action, |target| target.position)?.position;

    let Some(mut block) = world_map.get_block(position).copied() else {
        return Err(Refused::at(position, Refusal::NoTarget));
    };
    let Some(break_time) = block.id.get_break_time() else {
        return Err(Refused::at(position, Refusal::Unbreakable(block.id)));
    };

    block.breaking_progress = block.breaking_progress.saturating_add(1);
    if block.breaking_progress < break_time {
        world_map.set_block(position, block);
        return Ok(());
    }

    world_map.remove_block(position);
    for (item_id, nb) in block.id.get_drops(1) {
        let lost = player.inventory.add_item_to_inventory(ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        });
        if lost > 0 {
            log::info!(
                "Player {} has no room for {lost} of the {nb} {item_id:?} dropped by {:?}",
                player.id,
                block.id
            );
        }
    }
    Ok(())
}

/// Places the block held in the selected hotbar slot against the aimed face.
fn place_block(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<(), Refused> {
    let new_block_position = |target: &RaycastResponse| target.position + target.face.to_ivec3();
    let position = new_block_position(&aimed_block(player, world_map, action, new_block_position)?);
    let refused = |reason| Err(Refused::at(position, reason));

    if world_map
        .get_block(position)
        .is_some_and(|block| !block.id.is_replaceable())
    {
        return refused(Refusal::Occupied);
    }
    // A block is never placed into terrain that is not loaded, and must not use up the item
    // then. There is no block to re-state either: neither side has this chunk.
    if !world_map.is_chunk_loaded(global_block_to_chunk_pos(&position)) {
        return Err(Refused {
            reason: Refusal::ChunkNotLoaded,
            predicted_at: None,
        });
    }
    if block_overlaps_player(position, player) {
        return refused(Refusal::InsidePlayer);
    }

    let slot = action.hotbar_slot;
    if slot >= MAX_INVENTORY_SLOTS {
        return refused(Refusal::InvalidSlot(slot));
    }
    let Some(&item) = player.inventory.inner.get(&slot) else {
        return refused(Refusal::EmptySlot(slot));
    };
    let ItemType::Block(block_id) = item.item_type else {
        return refused(Refusal::NotABlock(item.item_type));
    };

    player.inventory.remove_item_from_stack(slot, 1);
    world_map.set_block(position, BlockData::new(block_id, BlockDirection::Front));
    Ok(())
}

/// Whether a block at `block_position` would overlap the player's box, whose `position` is
/// its centre.
fn block_overlaps_player(block_position: IVec3, player: &Player) -> bool {
    let distance = (player.position - (block_position.as_vec3() + HALF_BLOCK)).abs();
    let reach =
        (Vec3::splat(CUBE_SIZE) + Vec3::new(player.width, player.height, player.width)) / 2.;
    distance.x <= reach.x && distance.y <= reach.y && distance.z <= reach.z
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

    #[test]
    fn a_block_placed_into_water_replaces_it() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
        world.fill_box(IVec3::ZERO, IVec3::ZERO, BlockId::Water);
        let player = place_across_a_chunk_border(&mut world);
        assert_eq!(
            world.get_block(IVec3::ZERO).map(|b| b.id),
            Some(BlockId::Dirt)
        );
        assert!(player.inventory.inner.is_empty());
    }
}
