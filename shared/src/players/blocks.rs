//! Breaking and placing blocks, run by the client (prediction) and by the server (authority).

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{movement::MAX_FRAME_DELTA_MS, Player},
    world::{
        global_block_to_chunk_pos, raycast, BlockData, BlockDirection, BlockId, ChunkMap,
        FaceDirectionExt, ItemType, RaycastResponse,
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

/// How far from the centre of a block a player may break, place or use it.
pub const INTERACTION_DISTANCE: f32 = 5.0;
/// Time the left button must be held on a block for one mining hit, a server tick.
pub const HIT_MS: u64 = 50;
/// Time between two blocks placed while the right button is held, four server ticks.
pub const PLACE_REPEAT_MS: u64 = 200;
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
    /// The aimed block is used by a right click, like a crafting table, unless the player
    /// sneaks.
    UsesBlock(BlockId),
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

/// A block a player broke. Its items are not given to the player: the server drops them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrokenBlock {
    pub position: IVec3,
    pub block: BlockId,
}

/// Breaks and places the blocks `action` asks for, and returns the blocks it broke.
pub fn simulate_player_block_interactions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) -> Vec<BrokenBlock> {
    let mut broken = Vec::new();
    // Both count the time the button is held, so that a block takes as long to break, and
    // blocks are placed as often, at any frame rate.
    let hits = mining_hits(player, action);
    let may_place = place_ready(player, action);
    for network_action in &action.inputs {
        let (verb, outcome) = match network_action {
            NetworkAction::LeftClick if hits == 0 => continue,
            NetworkAction::LeftClick => (
                "break",
                break_block(player, world_map, action, hits).map(|block| broken.extend(block)),
            ),
            NetworkAction::RightClick if !may_place => continue,
            NetworkAction::RightClick => (
                "place",
                place_block(player, world_map, action)
                    .map(|()| player.place_wait_ms = PLACE_REPEAT_MS),
            ),
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
    broken
}

/// Mining hits the time held in `action` adds up to; what is left of it is kept for the next
/// input. Releasing the left button drops the time held.
fn mining_hits(player: &mut Player, action: &PlayerFrameInput) -> u8 {
    if !action.is_pressed(NetworkAction::LeftClick) {
        player.mining_carry_ms = 0;
        return 0;
    }
    player.mining_carry_ms += action.delta_ms.min(MAX_FRAME_DELTA_MS);
    let hits = player.mining_carry_ms / HIT_MS;
    player.mining_carry_ms %= HIT_MS;
    // At most (HIT_MS - 1 + MAX_FRAME_DELTA_MS) / HIT_MS, 2 hits
    hits as u8
}

/// Whether the right button in `action` may place a block: at once when pressed, then once per
/// `PLACE_REPEAT_MS` while held. Releasing it drops the wait.
fn place_ready(player: &mut Player, action: &PlayerFrameInput) -> bool {
    if !action.is_pressed(NetworkAction::RightClick) {
        player.place_wait_ms = 0;
        return false;
    }
    player.place_wait_ms = player
        .place_wait_ms
        .saturating_sub(action.delta_ms.min(MAX_FRAME_DELTA_MS));
    player.place_wait_ms == 0
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

/// `hits` hits on the aimed block: it breaks once it took as many hits as its break time, and is
/// returned then. The hits left over by a broken block are lost, and never reach the block
/// behind it.
fn break_block(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    hits: u8,
) -> Result<Option<BrokenBlock>, Refused> {
    let position = aimed_block(player, world_map, action, |target| target.position)?.position;

    let Some(mut block) = world_map.get_block(position).copied() else {
        return Err(Refused::at(position, Refusal::NoTarget));
    };
    let Some(break_time) = block.id.get_break_time() else {
        return Err(Refused::at(position, Refusal::Unbreakable(block.id)));
    };

    block.breaking_progress = block.breaking_progress.saturating_add(hits);
    if block.breaking_progress < break_time {
        world_map.set_block(position, block);
        return Ok(None);
    }

    world_map.remove_block(position);
    Ok(Some(BrokenBlock {
        position,
        block: block.id,
    }))
}

/// Places the block held in the selected hotbar slot against the aimed face.
fn place_block(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<(), Refused> {
    let new_block_position = |target: &RaycastResponse| target.position + target.face.to_ivec3();
    let target = aimed_block(player, world_map, action, new_block_position)?;
    if let Some(aimed) = world_map.get_block(target.position) {
        if aimed.id == BlockId::CraftingTable && !action.is_pressed(NetworkAction::SneakOrFlyDown) {
            // The client opens the table instead, and predicts no block either
            return Err(Refused {
                reason: Refusal::UsesBlock(aimed.id),
                predicted_at: None,
            });
        }
    }
    let position = new_block_position(&target);
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
    use crate::world::{BlockId, ItemId, ItemStack};
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
            delta_ms: HIT_MS,
            inputs: [NetworkAction::LeftClick].into(),
            camera,
            ..Default::default()
        };
        (player, world, input)
    }

    #[test]
    fn breaking_takes_the_same_time_at_any_frame_rate() {
        let break_ms = u64::from(BlockId::Stone.get_break_time().unwrap()) * HIT_MS;
        for delta_ms in [4, 7, 16, 33, 100] {
            let (mut player, mut world, mut input) = left_click_on(BlockId::Stone);
            input.delta_ms = delta_ms;
            let mut held_ms = 0;
            while world.get_block(TARGET).is_some() {
                assert!(
                    held_ms < break_ms,
                    "{delta_ms} ms: not broken after {held_ms} ms"
                );
                simulate_player_block_interactions(
                    &mut player,
                    &mut world,
                    &input,
                    CallerType::Server,
                );
                held_ms += delta_ms;
            }
            assert!(
                held_ms >= break_ms,
                "{delta_ms} ms: broken after {held_ms} ms"
            );
        }
    }

    #[test]
    fn releasing_the_button_drops_the_time_held() {
        let (mut player, mut world, mut input) = left_click_on(BlockId::Stone);
        input.delta_ms = HIT_MS - 1;
        let mut released = input.clone();
        released.inputs.clear();
        for input in [&input, &released, &input] {
            simulate_player_block_interactions(&mut player, &mut world, input, CallerType::Server);
        }
        assert_eq!(world.get_block(TARGET).unwrap().breaking_progress, 0);
    }

    #[test]
    fn hits_left_over_by_a_broken_block_do_not_reach_the_one_behind() {
        let (mut player, mut world, mut input) = left_click_on(BlockId::Dandelion);
        let behind = TARGET + IVec3::NEG_Z;
        world.fill_box(behind, behind, BlockId::Stone);
        assert_eq!(BlockId::Dandelion.get_break_time(), Some(2));
        // One hit, then two in one long frame: the second one has no block left to hit
        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        input.delta_ms = 2 * HIT_MS;
        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        assert!(world.get_block(TARGET).is_none());
        assert_eq!(world.get_block(behind).unwrap().breaking_progress, 0);
    }

    #[test]
    fn breaking_a_regular_block_takes_its_break_time() {
        let (mut player, mut world, input) = left_click_on(BlockId::Dandelion);
        let steps = BlockId::Dandelion.get_break_time().unwrap();
        for _ in 1..steps {
            let broken = simulate_player_block_interactions(
                &mut player,
                &mut world,
                &input,
                CallerType::Server,
            );
            assert!(broken.is_empty());
            assert!(world.get_block(TARGET).is_some());
        }
        let broken =
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        assert!(world.get_block(TARGET).is_none());
        assert_eq!(world.take_changed_blocks(), vec![TARGET]);
        assert_eq!(
            broken,
            vec![BrokenBlock {
                position: TARGET,
                block: BlockId::Dandelion
            }]
        );
        // The items are dropped by the server, not given to the player
        assert!(player.inventory.inner.is_empty());
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
    fn right_click_on_a_table_places_nothing_unless_sneaking() {
        for (sneaking, placed) in [(false, None), (true, Some(BlockId::Dirt))] {
            let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
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
            world.fill_box(
                IVec3::new(0, 0, -1),
                IVec3::new(0, 0, -1),
                BlockId::CraftingTable,
            );
            let mut input = PlayerFrameInput {
                inputs: [NetworkAction::RightClick].into(),
                camera,
                ..Default::default()
            };
            if sneaking {
                input.inputs.insert(NetworkAction::SneakOrFlyDown);
            }

            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);

            assert_eq!(
                world.get_block(IVec3::ZERO).map(|b| b.id),
                placed,
                "sneaking: {sneaking}"
            );
        }
    }

    #[test]
    fn holding_the_right_button_places_a_block_once_per_repeat_time() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
        let mut player = place_across_a_chunk_border(&mut world);
        player.inventory.inner.insert(
            0,
            ItemStack {
                item_id: ItemId::Dirt,
                item_type: ItemId::Dirt.get_default_type(),
                nb: 2,
            },
        );
        let camera = Transform::from_translation(player.position);
        let mut input = PlayerFrameInput {
            delta_ms: 16,
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };
        let mut released = input.clone();
        released.inputs.clear();
        let hold = |player: &mut Player, world: &mut ChunkMap, input: &PlayerFrameInput| {
            simulate_player_block_interactions(player, world, input, CallerType::Server)
        };
        let next = IVec3::new(0, 0, 1);

        // The first block was placed at once; the next one waits while the button is held
        let mut held_ms = 0;
        while world.get_block(next).is_none() {
            assert!(held_ms <= PLACE_REPEAT_MS, "not placed after {held_ms} ms");
            hold(&mut player, &mut world, &input);
            held_ms += input.delta_ms;
        }
        assert!(held_ms >= PLACE_REPEAT_MS, "placed after {held_ms} ms");

        // Released and pressed again, a block is placed at once
        world.remove_block(next);
        hold(&mut player, &mut world, &released);
        input.delta_ms = 1;
        hold(&mut player, &mut world, &input);
        assert_eq!(world.get_block(next).map(|b| b.id), Some(BlockId::Dirt));
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
