//! Breaking and placing blocks, run by the client (prediction) and by the server (authority).

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{movement::MAX_FRAME_DELTA_MS, Player, ViewMode},
    world::{
        global_block_to_chunk_pos, liquid::is_source, raycast, BlockData, BlockDirection, BlockId,
        BlockTransparency, ChunkMap, Container, FaceDirectionExt, Furnace, ItemId, ItemStack,
        ItemType, MobKind, RaycastResponse, ServerMob, Tool,
    },
    HALF_BLOCK, MAX_INVENTORY_SLOTS,
};
use bevy::{
    math::{IVec3, Vec3},
    prelude::Transform,
};

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
/// The wait after a bucket was used: none ends but the release of the button, so that a bucket
/// acts once per press and never pours back at once what it just took.
const UNTIL_RELEASED_MS: u64 = u64::MAX;
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
    /// A vine hangs on the side of a block, never on its top or bottom.
    NotASide,
    /// An empty bucket fills only from a water source.
    NotASource(BlockId),
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
#[derive(Debug, Clone, PartialEq)]
pub struct BrokenBlock {
    pub position: IVec3,
    pub block: BlockId,
    /// What a broken chest held, dropped with it. A client only knows the contents of the
    /// chest its player has open.
    pub contents: Option<Container>,
    /// What a broken furnace held, dropped with it, as `contents`.
    pub furnace: Option<Furnace>,
    /// Whether the block drops its items: a block that needs a tool, like stone, drops nothing
    /// when broken without it. What a chest or a furnace held drops in any case.
    pub harvested: bool,
}

/// A mob a player made appear with a spawn egg. The server adds it to the world.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnedMob {
    pub kind: MobKind,
    /// The middle of its hitbox.
    pub position: Vec3,
}

/// What the block interactions of one input did that the server acts upon.
#[derive(Debug, Default, PartialEq)]
pub struct BlockInteractions {
    pub broken: Vec<BrokenBlock>,
    pub spawned: Vec<SpawnedMob>,
    /// The water bucket a player filled and had no room for: the server drops it.
    pub dropped: Vec<ItemStack>,
}

/// Breaks and places the blocks `action` asks for, or uses a spawn egg or a bucket, and returns
/// the blocks it broke, the mobs it made appear and what the player had no room for.
pub fn simulate_player_block_interactions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) -> BlockInteractions {
    let mut broken = Vec::new();
    let mut spawned = Vec::new();
    let mut dropped = Vec::new();
    // Both count the time the button is held, so that a block takes as long to break, and
    // blocks are placed as often, at any frame rate.
    let hits = mining_hits(player, action);
    let may_place = place_ready(player, action);
    for network_action in &action.inputs {
        let (verb, outcome) = match network_action {
            NetworkAction::LeftClick if hits == 0 => continue,
            NetworkAction::LeftClick => (
                "break a block",
                break_block(player, world_map, action, hits).map(|block| broken.extend(block)),
            ),
            NetworkAction::RightClick if !may_place => continue,
            NetworkAction::RightClick => match held_item(player, action) {
                Some(ItemId::Bucket) => (
                    "fill a bucket",
                    fill_bucket(player, world_map, action).map(|left| {
                        dropped.extend(left);
                        player.place_wait_ms = UNTIL_RELEASED_MS
                    }),
                ),
                Some(ItemId::WaterBucket) => (
                    "pour a bucket",
                    pour_water(player, world_map, action)
                        .map(|()| player.place_wait_ms = UNTIL_RELEASED_MS),
                ),
                _ => (
                    "place a block",
                    place_block(player, world_map, action).map(|mob| {
                        spawned.extend(mob);
                        player.place_wait_ms = PLACE_REPEAT_MS
                    }),
                ),
            },
            _ => continue,
        };
        let Err(refused) = outcome else {
            continue;
        };
        log::debug!(
            "{} Player {} could not {verb}: {:?}",
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
    BlockInteractions {
        broken,
        spawned,
        dropped,
    }
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

/// The block the player aims at through `camera`, holding `held`: an empty bucket aims at the
/// water sources too, any other item through water.
pub fn aim(
    world_map: &ChunkMap,
    camera: &Transform,
    player_position: &Vec3,
    view_mode: ViewMode,
    held: Option<ItemId>,
) -> Option<RaycastResponse> {
    if held == Some(ItemId::Bucket) {
        raycast::raycast_to_water_source(world_map, camera, player_position, view_mode)
    } else {
        raycast::raycast(world_map, camera, player_position, view_mode)
    }
}

/// The block the player aims at with the item it holds, if it is within reach. An out of reach
/// refusal names `predicted_at(target)` as the block to re-state.
fn aimed_block(
    player: &Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
    predicted_at: impl Fn(&RaycastResponse) -> IVec3,
) -> Result<RaycastResponse, Refused> {
    let target = aim(
        world_map,
        &action.camera,
        &player.position,
        action.view_mode,
        held_item(player, action),
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

/// The item in the selected hotbar slot.
fn held_item(player: &Player, action: &PlayerFrameInput) -> Option<ItemId> {
    if action.hotbar_slot >= MAX_INVENTORY_SLOTS {
        return None;
    }
    Some(player.inventory.inner.get(&action.hotbar_slot)?.item_id)
}

/// The tool in the selected hotbar slot, if it holds one.
fn held_tool(player: &Player, action: &PlayerFrameInput) -> Option<Tool> {
    player
        .inventory
        .inner
        .get(&action.hotbar_slot)?
        .item_id
        .tool()
}

/// `hits` hits on the aimed block: it breaks once it took as many hits as its break time, and is
/// returned then. A held tool made for the block counts each hit as its speed, and wears out by
/// breaking blocks. The hits left over by a broken block are lost, and never reach the block
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

    let tool = held_tool(player, action);
    let speed = tool
        .filter(|tool| block.id.is_mined_faster_by(tool.kind))
        .map_or(1, |tool| tool.speed);
    block.breaking_progress = block
        .breaking_progress
        .saturating_add(hits.saturating_mul(speed));
    if block.breaking_progress < break_time {
        world_map.set_block(position, block);
        return Ok(None);
    }

    let harvested = block.id.is_harvested_by(tool);
    // Like in Minecraft, the plants that break at once do not wear a tool out
    if let Some(tool) = tool {
        if block.id.get_visibility() != BlockTransparency::Decoration {
            player
                .inventory
                .wear_tool(action.hotbar_slot, tool.wear_per_block);
        }
    }
    let contents = world_map.take_container(position);
    let furnace = world_map.take_furnace(position);
    world_map.remove_block(position);
    Ok(Some(BrokenBlock {
        position,
        block: block.id,
        contents,
        furnace,
        harvested,
    }))
}

/// Refuses a right click on a block that it uses, like a chest, unless the player sneaks.
fn refuse_used_block(
    world_map: &ChunkMap,
    target: &RaycastResponse,
    action: &PlayerFrameInput,
) -> Result<(), Refused> {
    match world_map.get_block(target.position) {
        Some(aimed)
            if aimed.id.is_usable() && !action.is_pressed(NetworkAction::SneakOrFlyDown) =>
        {
            // The client opens the block instead, and predicts no change either
            Err(Refused {
                reason: Refusal::UsesBlock(aimed.id),
                predicted_at: None,
            })
        }
        _ => Ok(()),
    }
}

/// The aimed block and the cell against its aimed face, where a block can be placed: empty or
/// replaceable, in a loaded chunk.
fn cell_against_aimed_face(
    player: &Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
) -> Result<(RaycastResponse, IVec3), Refused> {
    let new_block_position = |target: &RaycastResponse| target.position + target.face.to_ivec3();
    let target = aimed_block(player, world_map, action, new_block_position)?;
    refuse_used_block(world_map, &target, action)?;
    let position = new_block_position(&target);

    if world_map
        .get_block(position)
        .is_some_and(|block| !block.id.is_replaceable())
    {
        return Err(Refused::at(position, Refusal::Occupied));
    }
    // A block is never placed into terrain that is not loaded, and must not use up the item
    // then. There is no block to re-state either: neither side has this chunk.
    if !world_map.is_chunk_loaded(global_block_to_chunk_pos(&position)) {
        return Err(Refused {
            reason: Refusal::ChunkNotLoaded,
            predicted_at: None,
        });
    }
    Ok((target, position))
}

/// Places the block held in the selected hotbar slot against the aimed face. A spawn egg makes
/// its mob appear there instead, standing on the bottom of the cell, and is returned.
fn place_block(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<Option<SpawnedMob>, Refused> {
    let (target, position) = cell_against_aimed_face(player, world_map, action)?;
    let refused = |reason| Err(Refused::at(position, reason));
    let slot = action.hotbar_slot;
    if slot >= MAX_INVENTORY_SLOTS {
        return refused(Refusal::InvalidSlot(slot));
    }
    let Some(&item) = player.inventory.inner.get(&slot) else {
        return refused(Refusal::EmptySlot(slot));
    };
    if let Some(kind) = item.item_id.spawned_mob() {
        player.inventory.remove_item_from_stack(slot, 1);
        let cell = position.as_vec3();
        return Ok(Some(SpawnedMob {
            kind,
            position: ServerMob::standing_at(kind, cell.x + 0.5, cell.y, cell.z + 0.5),
        }));
    }
    if block_overlaps_player(position, player) {
        return refused(Refusal::InsidePlayer);
    }
    let ItemType::Block(block_id) = item.item_type else {
        return refused(Refusal::NotABlock(item.item_type));
    };

    // A vine is turned toward the block it is placed against, a furnace toward the player
    let direction = match block_id {
        BlockId::Vine => {
            let Some(toward) = BlockDirection::toward(-target.face.to_ivec3()) else {
                return refused(Refusal::NotASide);
            };
            toward
        }
        BlockId::Furnace => facing(&action.camera),
        _ => BlockDirection::Front,
    };

    player.inventory.remove_item_from_stack(slot, 1);
    world_map.set_block(position, BlockData::new(block_id, direction));
    if block_id == BlockId::Chest {
        world_map.insert_container(position, Container::default());
    }
    if block_id == BlockId::Furnace {
        world_map.insert_furnace(position, Furnace::default());
    }
    Ok(None)
}

/// Fills the bucket held in the selected hotbar slot from the water source the player aims at,
/// which is taken away. A single bucket becomes the water bucket; from a stack, the water bucket
/// goes into the inventory, and is returned when there is no room for it.
fn fill_bucket(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<Option<ItemStack>, Refused> {
    let target = aimed_block(player, world_map, action, |target| target.position)?;
    refuse_used_block(world_map, &target, action)?;
    if !is_source(&target.block) {
        return Err(Refused::at(
            target.position,
            Refusal::NotASource(target.block.id),
        ));
    }

    world_map.remove_block(target.position);
    let water_bucket = ItemStack {
        item_id: ItemId::WaterBucket,
        item_type: ItemId::WaterBucket.get_default_type(),
        nb: 1,
    };
    let slot = action.hotbar_slot;
    if player
        .inventory
        .inner
        .get(&slot)
        .is_some_and(|held| held.nb == 1)
    {
        player.inventory.inner.insert(slot, water_bucket);
        return Ok(None);
    }
    player.inventory.remove_item_from_stack(slot, 1);
    let left = player.inventory.add_item_to_inventory(water_bucket);
    Ok((left > 0).then_some(water_bucket))
}

/// Pours the water of the water bucket held in the selected hotbar slot against the aimed face,
/// where a block would be placed: a source, which the flow of the server then spreads. The water
/// bucket becomes a bucket. Water stops nobody, so it may be poured where the player stands;
/// into a source, it would change nothing, and the water bucket is kept.
fn pour_water(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<(), Refused> {
    let (_, position) = cell_against_aimed_face(player, world_map, action)?;
    if world_map.get_block(position).is_some_and(is_source) {
        return Err(Refused::at(position, Refusal::Occupied));
    }

    world_map.set_block(
        position,
        BlockData::new(BlockId::Water, BlockDirection::Front),
    );
    player.inventory.inner.insert(
        action.hotbar_slot,
        ItemStack {
            item_id: ItemId::Bucket,
            item_type: ItemId::Bucket.get_default_type(),
            nb: 1,
        },
    );
    Ok(())
}

/// The direction that turns the front of a block toward a player looking through `camera`:
/// against the horizontal axis it looks the most along.
fn facing(camera: &Transform) -> BlockDirection {
    let look = camera.forward();
    let step = if look.x.abs() > look.z.abs() {
        IVec3::new(-look.x.signum() as i32, 0, 0)
    } else {
        IVec3::new(0, 0, -look.z.signum() as i32)
    };
    BlockDirection::toward(step).unwrap_or(BlockDirection::Front)
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
    use crate::players::game_mode::GameMode;
    use crate::world::{BlockId, Furnace, ItemId, ItemStack};
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
            let done = simulate_player_block_interactions(
                &mut player,
                &mut world,
                &input,
                CallerType::Server,
            );
            assert!(done.broken.is_empty());
            assert!(world.get_block(TARGET).is_some());
        }
        let broken =
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server)
                .broken;
        assert!(world.get_block(TARGET).is_none());
        assert_eq!(world.take_changed_blocks(), vec![TARGET]);
        assert_eq!(
            broken,
            vec![BrokenBlock {
                position: TARGET,
                block: BlockId::Dandelion,
                contents: None,
                furnace: None,
                harvested: true,
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
        place_across_a_chunk_border_holding(world, ItemId::Dirt)
    }

    /// `place_across_a_chunk_border`, holding `item` in place of the dirt.
    fn place_across_a_chunk_border_holding(world: &mut ChunkMap, item: ItemId) -> Player {
        let position = Vec3::new(0.5, 0.5, 2.5);
        let camera = Transform::from_translation(position);
        let mut player = Player::new(1, "tester".to_string(), position, camera);
        player.inventory.inner.insert(
            0,
            ItemStack {
                item_id: item,
                item_type: item.get_default_type(),
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
    fn right_click_on_a_table_a_chest_or_a_furnace_places_nothing_unless_sneaking() {
        let cases = [false, true].into_iter().flat_map(|sneaking| {
            let placed = sneaking.then_some(BlockId::Dirt);
            [
                BlockId::CraftingTable,
                BlockId::Chest,
                BlockId::Furnace,
                BlockId::LitFurnace,
            ]
            .map(|used| (used, sneaking, placed))
        });
        for (used, sneaking, placed) in cases {
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
            world.fill_box(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1), used);
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
                "{used:?}, sneaking: {sneaking}"
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

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    /// Hits the chest at `TARGET` until it breaks, and returns what breaking it gave.
    fn break_chest(
        player: &mut Player,
        world: &mut ChunkMap,
        input: &PlayerFrameInput,
    ) -> Vec<BrokenBlock> {
        let steps = BlockId::Chest.get_break_time().unwrap();
        (0..steps)
            .flat_map(|_| {
                simulate_player_block_interactions(player, world, input, CallerType::Server).broken
            })
            .collect()
    }

    #[test]
    fn placing_a_chest_gives_it_empty_contents() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
        let position = Vec3::new(0.5, 0.5, 2.5);
        let camera = Transform::from_translation(position);
        let mut player = Player::new(1, "tester".to_string(), position, camera);
        player.inventory.inner.insert(0, stack(ItemId::Chest, 1));
        world.fill_box(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1), BlockId::Stone);
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };

        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);

        assert_eq!(
            world.get_block(IVec3::ZERO).map(|b| b.id),
            Some(BlockId::Chest)
        );
        assert_eq!(world.container(IVec3::ZERO), Some(&Container::default()));
    }

    #[test]
    fn placing_a_furnace_gives_it_an_empty_furnace() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
        let position = Vec3::new(0.5, 0.5, 2.5);
        let camera = Transform::from_translation(position);
        let mut player = Player::new(1, "tester".to_string(), position, camera);
        player.inventory.inner.insert(0, stack(ItemId::Furnace, 1));
        world.fill_box(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1), BlockId::Stone);
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };

        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);

        assert_eq!(
            world.get_block(IVec3::ZERO).map(|b| b.id),
            Some(BlockId::Furnace)
        );
        assert_eq!(world.furnace(IVec3::ZERO), Some(&Furnace::default()));
        assert_eq!(world.container(IVec3::ZERO), None);
    }

    #[test]
    fn a_spawn_egg_makes_its_mob_appear_on_the_aimed_face_and_is_used_up() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, 0));
        let position = Vec3::new(0.5, 0.5, 2.5);
        let camera = Transform::from_translation(position);
        let mut player = Player::new(1, "tester".to_string(), position, camera);
        player
            .inventory
            .inner
            .insert(0, stack(ItemId::SheepSpawnEgg, 2));
        world.fill_box(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1), BlockId::Stone);
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };

        let done =
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);

        // standing on the bottom of the cell in front of the stone, which stays empty
        assert_eq!(
            done.spawned,
            vec![SpawnedMob {
                kind: MobKind::Sheep,
                position: Vec3::new(0.5, MobKind::Sheep.size().y / 2.0, 0.5),
            }]
        );
        assert_eq!(world.get_block(IVec3::ZERO), None);
        assert_eq!(player.inventory.inner[&0].nb, 1);
    }

    /// A player holding a vine two blocks away from the stone at `stone` on the side `side`,
    /// right-clicking the face of the stone toward it. Returns what the world and the player
    /// become.
    fn place_a_vine_against(stone: IVec3, side: IVec3) -> (ChunkMap, Player) {
        place_against(ItemId::Vine, stone, side)
    }

    /// `place_a_vine_against`, with `item` in place of the vine.
    fn place_against(item: ItemId, stone: IVec3, side: IVec3) -> (ChunkMap, Player) {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        world.fill_box(stone, stone, BlockId::Stone);
        world.take_changed_blocks();
        let stone_centre = stone.as_vec3() + HALF_BLOCK;
        let position = stone_centre + side.as_vec3() * 3.0;
        let up = if side.y == 0 { Vec3::Y } else { Vec3::X };
        let camera = Transform::from_translation(position).looking_at(stone_centre, up);
        let mut player = Player::new(1, "tester".to_string(), position, camera);
        player.inventory.inner.insert(0, stack(item, 1));
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };
        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        (world, player)
    }

    #[test]
    fn a_vine_placed_against_a_side_hangs_toward_that_block() {
        let stone = IVec3::new(0, 0, -3);
        for (side, toward_the_stone) in [
            (IVec3::Z, BlockDirection::Front),
            (IVec3::NEG_Z, BlockDirection::Back),
            (IVec3::X, BlockDirection::Left),
            (IVec3::NEG_X, BlockDirection::Right),
        ] {
            let (world, player) = place_a_vine_against(stone, side);
            assert_eq!(
                world.get_block(stone + side).copied(),
                Some(BlockData::new(BlockId::Vine, toward_the_stone)),
                "{side}"
            );
            assert!(player.inventory.inner.is_empty(), "{side}");
        }
    }

    #[test]
    fn a_furnace_faces_the_player_who_placed_it_and_other_blocks_the_north() {
        let stone = IVec3::new(0, 0, -3);
        for (side, toward_the_player) in [
            (IVec3::Z, BlockDirection::Back),
            (IVec3::NEG_Z, BlockDirection::Front),
            (IVec3::X, BlockDirection::Right),
            (IVec3::NEG_X, BlockDirection::Left),
        ] {
            let (world, _) = place_against(ItemId::Furnace, stone, side);
            assert_eq!(
                world.get_block(stone + side).copied(),
                Some(BlockData::new(BlockId::Furnace, toward_the_player)),
                "{side}"
            );
            let (world, _) = place_against(ItemId::Chest, stone, side);
            assert_eq!(
                world.get_block(stone + side).copied(),
                Some(BlockData::new(BlockId::Chest, BlockDirection::Front)),
                "{side}"
            );
        }
    }

    #[test]
    fn a_vine_aimed_at_a_top_or_bottom_face_is_refused_and_kept() {
        let stone = IVec3::new(0, 0, -3);
        for side in [IVec3::Y, IVec3::NEG_Y] {
            let (mut world, player) = place_a_vine_against(stone, side);
            assert_eq!(world.get_block(stone + side), None, "{side}");
            assert_eq!(player.inventory.inner[&0].nb, 1, "{side}");
            // the server re-states the cell a client may have predicted a vine in
            assert_eq!(world.take_changed_blocks(), vec![stone + side], "{side}");
        }
    }

    #[test]
    fn a_broken_chest_hands_its_contents_over_with_it() {
        let (mut player, mut world, input) = left_click_on(BlockId::Chest);
        let mut contents = Container::default();
        contents.slots[4] = Some(stack(ItemId::Stone, 10));
        world.insert_container(TARGET, contents.clone());

        let broken = break_chest(&mut player, &mut world, &input);

        assert!(world.get_block(TARGET).is_none());
        assert_eq!(world.container(TARGET), None);
        assert_eq!(
            broken,
            vec![BrokenBlock {
                position: TARGET,
                block: BlockId::Chest,
                contents: Some(contents),
                furnace: None,
                harvested: true,
            }]
        );
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn a_broken_furnace_hands_its_stacks_over_with_it() {
        for id in [BlockId::Furnace, BlockId::LitFurnace] {
            let (mut player, mut world, input) = left_click_on(id);
            let furnace = Furnace {
                input: Some(stack(ItemId::Sand, 3)),
                fuel: Some(stack(ItemId::Coal, 2)),
                output: Some(stack(ItemId::Glass, 1)),
                burn_left: 40,
                ..Default::default()
            };
            world.insert_furnace(TARGET, furnace.clone());

            let broken: Vec<BrokenBlock> = (0..id.get_break_time().unwrap())
                .flat_map(|_| {
                    simulate_player_block_interactions(
                        &mut player,
                        &mut world,
                        &input,
                        CallerType::Server,
                    )
                    .broken
                })
                .collect();

            assert_eq!(world.furnace(TARGET), None, "{id:?}");
            assert_eq!(
                broken,
                vec![BrokenBlock {
                    position: TARGET,
                    block: id,
                    contents: None,
                    furnace: Some(furnace),
                    // without a pickaxe, the furnace itself drops nothing
                    harvested: false,
                }],
                "{id:?}"
            );
        }
    }

    #[test]
    fn a_chest_whose_contents_are_unknown_breaks_like_any_block() {
        let (mut player, mut world, input) = left_click_on(BlockId::Chest);
        let broken = break_chest(&mut player, &mut world, &input);
        assert!(world.get_block(TARGET).is_none());
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].contents, None);
    }

    /// Hits the block at `TARGET` until it breaks, holding `held` in the selected slot, and
    /// returns the hits it took and what breaking it gave.
    fn break_holding(id: BlockId, held: Option<ItemStack>) -> (u32, Player, BrokenBlock) {
        let (mut player, mut world, input) = left_click_on(id);
        if let Some(held) = held {
            player.inventory.inner.insert(input.hotbar_slot, held);
        }
        for hits in 1..=u32::from(id.get_break_time().unwrap()) {
            let mut done = simulate_player_block_interactions(
                &mut player,
                &mut world,
                &input,
                CallerType::Server,
            );
            if let Some(broken) = done.broken.pop() {
                return (hits, player, broken);
            }
        }
        panic!("{id:?} not broken by {held:?}");
    }

    fn tool(item_id: ItemId) -> Option<ItemStack> {
        Some(stack(item_id, 1))
    }

    #[test]
    fn a_tool_breaks_the_blocks_it_is_made_for_faster() {
        for (block, item) in [
            (BlockId::Stone, ItemId::WoodenPickaxe),
            (BlockId::OakLog, ItemId::WoodenAxe),
            (BlockId::Dirt, ItemId::WoodenShovel),
            (BlockId::OakLeaves, ItemId::WoodenHoe),
            (BlockId::SpruceLeaves, ItemId::WoodenSword),
        ] {
            let (by_hand, ..) = break_holding(block, None);
            let (with_tool, ..) = break_holding(block, tool(item));
            let speed = u32::from(item.tool().unwrap().speed);
            assert_eq!(by_hand, u32::from(block.get_break_time().unwrap()));
            assert_eq!(
                with_tool,
                by_hand.div_ceil(speed),
                "{block:?} with {item:?}"
            );
        }
    }

    #[test]
    fn a_tool_breaks_other_blocks_like_a_hand() {
        for (block, item) in [
            (BlockId::Dirt, ItemId::WoodenPickaxe),
            (BlockId::Stone, ItemId::WoodenAxe),
            (BlockId::Glass, ItemId::WoodenShovel),
        ] {
            let (by_hand, ..) = break_holding(block, None);
            let (with_tool, ..) = break_holding(block, tool(item));
            assert_eq!(with_tool, by_hand, "{block:?} with {item:?}");
        }
    }

    #[test]
    fn stone_drops_its_items_only_when_broken_with_a_pickaxe() {
        for (held, harvested) in [
            (None, false),
            (Some(stack(ItemId::Dirt, 5)), false),
            (tool(ItemId::WoodenAxe), false),
            (tool(ItemId::WoodenPickaxe), true),
        ] {
            for block in [BlockId::Stone, BlockId::Cobblestone, BlockId::Furnace] {
                let (.., broken) = break_holding(block, held);
                assert_eq!(broken.harvested, harvested, "{block:?} with {held:?}");
            }
        }
        // A block that needs no tool always drops its items
        assert!(break_holding(BlockId::Dirt, None).2.harvested);
        assert!(
            break_holding(BlockId::Dirt, tool(ItemId::WoodenPickaxe))
                .2
                .harvested
        );
    }

    #[test]
    fn a_better_tier_breaks_stone_faster() {
        // Stone takes 20 hits by hand
        for (item, hits) in [
            (ItemId::WoodenPickaxe, 10),
            (ItemId::StonePickaxe, 5),
            (ItemId::IronPickaxe, 4),
            (ItemId::DiamondPickaxe, 3),
            (ItemId::GoldenPickaxe, 2),
        ] {
            let (with_tool, ..) = break_holding(BlockId::Stone, tool(item));
            assert_eq!(with_tool, hits, "{item:?}");
        }
    }

    #[test]
    fn an_ore_drops_its_items_only_when_broken_with_a_pickaxe_of_its_level() {
        for (block, item, harvested) in [
            (BlockId::IronOre, ItemId::WoodenPickaxe, false),
            (BlockId::IronOre, ItemId::GoldenPickaxe, false),
            (BlockId::IronOre, ItemId::StonePickaxe, true),
            (BlockId::DiamondOre, ItemId::StonePickaxe, false),
            (BlockId::DiamondOre, ItemId::IronPickaxe, true),
        ] {
            let (.., broken) = break_holding(block, tool(item));
            assert_eq!(broken.harvested, harvested, "{block:?} with {item:?}");
        }
    }

    #[test]
    fn each_broken_block_wears_the_held_tool() {
        for (block, item, wear) in [
            (BlockId::Stone, ItemId::WoodenPickaxe, 1),
            // A tool wears out on any block, not only on those it is made for
            (BlockId::Stone, ItemId::WoodenShovel, 1),
            (BlockId::OakLeaves, ItemId::WoodenSword, 2),
            // but not on the plants that break at once
            (BlockId::Poppy, ItemId::WoodenAxe, 0),
        ] {
            let (_, player, _) = break_holding(block, tool(item));
            assert_eq!(
                player.inventory.inner[&0].item_type,
                ItemType::Tool { wear },
                "{block:?} with {item:?}"
            );
        }
    }

    #[test]
    fn a_tool_breaking_its_last_block_breaks_too() {
        let durability = ItemId::WoodenPickaxe.tool().unwrap().durability;
        let worn = ItemStack {
            item_type: ItemType::Tool {
                wear: durability - 1,
            },
            ..stack(ItemId::WoodenPickaxe, 1)
        };
        let (hits, player, broken) = break_holding(BlockId::Stone, Some(worn));
        // The last block still gets the tool's speed and its drops
        assert_eq!(
            hits,
            u32::from(BlockId::Stone.get_break_time().unwrap()) / 2
        );
        assert!(broken.harvested);
        assert!(player.inventory.inner.is_empty());
    }

    fn water(level: u8) -> BlockData {
        BlockData {
            level,
            ..BlockData::new(BlockId::Water, BlockDirection::Front)
        }
    }

    /// A player at the centre of block (0,0,0), looking down -Z, holding `held` in its first
    /// hotbar slot, before stone at `TARGET` and `blocks` placed in front of it. Returns them
    /// with the right click it sends.
    fn bucket_in_front_of(
        blocks: &[(IVec3, BlockData)],
        held: ItemStack,
    ) -> (Player, ChunkMap, PlayerFrameInput) {
        let (mut player, mut world, mut input) = left_click_on(BlockId::Stone);
        for &(position, block) in blocks {
            world.set_block(position, block);
        }
        world.take_changed_blocks();
        player.inventory.inner.insert(0, held);
        input.inputs = [NetworkAction::RightClick].into();
        (player, world, input)
    }

    fn right_click(
        player: &mut Player,
        world: &mut ChunkMap,
        input: &PlayerFrameInput,
    ) -> Vec<ItemStack> {
        simulate_player_block_interactions(player, world, input, CallerType::Server).dropped
    }

    /// What the player holds in slot `slot`.
    fn held(player: &Player, slot: u32) -> Option<(ItemId, u32)> {
        let stack = player.inventory.inner.get(&slot)?;
        Some((stack.item_id, stack.nb))
    }

    const IN_FRONT: IVec3 = IVec3::new(0, 0, -2);

    #[test]
    fn a_bucket_takes_the_aimed_water_source_and_becomes_a_water_bucket_in_any_mode() {
        for mode in [GameMode::Survival, GameMode::Creative] {
            let (mut player, mut world, input) =
                bucket_in_front_of(&[(IN_FRONT, water(0))], stack(ItemId::Bucket, 1));
            player.game_mode = mode;

            let dropped = right_click(&mut player, &mut world, &input);

            assert_eq!(world.get_block(IN_FRONT), None, "{mode:?}");
            assert_eq!(world.take_changed_blocks(), vec![IN_FRONT], "{mode:?}");
            assert_eq!(held(&player, 0), Some((ItemId::WaterBucket, 1)), "{mode:?}");
            assert!(dropped.is_empty());
        }
    }

    #[test]
    fn a_bucket_of_a_stack_fills_into_the_inventory_or_is_handed_to_the_server() {
        let (mut player, mut world, input) =
            bucket_in_front_of(&[(IN_FRONT, water(0))], stack(ItemId::Bucket, 3));
        // The hand keeps the other buckets: the first empty slot takes the water bucket
        player.inventory.inner.insert(1, stack(ItemId::Dirt, 64));
        assert!(right_click(&mut player, &mut world, &input).is_empty());
        assert_eq!(held(&player, 0), Some((ItemId::Bucket, 2)));
        assert_eq!(held(&player, 2), Some((ItemId::WaterBucket, 1)));

        // With no room left, the water bucket is handed to the server to drop
        let (mut player, mut world, input) =
            bucket_in_front_of(&[(IN_FRONT, water(0))], stack(ItemId::Bucket, 3));
        for slot in 1..MAX_INVENTORY_SLOTS {
            player.inventory.inner.insert(slot, stack(ItemId::Dirt, 64));
        }
        let dropped = right_click(&mut player, &mut world, &input);
        assert_eq!(dropped, vec![stack(ItemId::WaterBucket, 1)]);
        assert_eq!(held(&player, 0), Some((ItemId::Bucket, 2)));
        assert_eq!(world.get_block(IN_FRONT), None);
    }

    #[test]
    fn a_bucket_aims_through_flowing_water_at_the_source_behind_it() {
        let near = IVec3::new(0, 0, -1);
        let (mut player, mut world, input) = bucket_in_front_of(
            &[(near, water(1)), (IN_FRONT, water(0))],
            stack(ItemId::Bucket, 1),
        );
        right_click(&mut player, &mut world, &input);
        assert_eq!(world.get_block(near), Some(&water(1)));
        assert_eq!(world.get_block(IN_FRONT), None);
        assert_eq!(held(&player, 0), Some((ItemId::WaterBucket, 1)));
    }

    #[test]
    fn a_bucket_aimed_at_flowing_water_or_a_block_takes_nothing() {
        for blocks in [vec![(IN_FRONT, water(3))], vec![]] {
            let (mut player, mut world, input) =
                bucket_in_front_of(&blocks, stack(ItemId::Bucket, 1));

            right_click(&mut player, &mut world, &input);

            assert_eq!(world.get_block(IN_FRONT), blocks.first().map(|(_, b)| b));
            assert_eq!(held(&player, 0), Some((ItemId::Bucket, 1)), "{blocks:?}");
            // the server re-states the aimed stone
            assert_eq!(world.take_changed_blocks(), vec![TARGET], "{blocks:?}");
        }
    }

    #[test]
    fn a_bucket_takes_no_source_out_of_reach() {
        let far = IVec3::new(0, 0, -8);
        let (mut player, mut world, input) =
            bucket_in_front_of(&[(far, water(0))], stack(ItemId::Bucket, 1));
        world.remove_block(TARGET);
        world.take_changed_blocks();

        right_click(&mut player, &mut world, &input);

        assert_eq!(world.get_block(far), Some(&water(0)));
        assert_eq!(held(&player, 0), Some((ItemId::Bucket, 1)));
        assert_eq!(world.take_changed_blocks(), vec![far]);
    }

    #[test]
    fn a_water_bucket_pours_a_source_against_the_aimed_face_and_becomes_a_bucket() {
        for (blocks, mode) in [
            (vec![], GameMode::Survival),
            (vec![], GameMode::Creative),
            // Flowing water there becomes a source
            (vec![(IN_FRONT, water(3))], GameMode::Survival),
        ] {
            let (mut player, mut world, input) =
                bucket_in_front_of(&blocks, stack(ItemId::WaterBucket, 1));
            player.game_mode = mode;

            right_click(&mut player, &mut world, &input);

            assert_eq!(world.get_block(IN_FRONT), Some(&water(0)), "{blocks:?}");
            assert_eq!(world.take_changed_blocks(), vec![IN_FRONT], "{blocks:?}");
            assert_eq!(held(&player, 0), Some((ItemId::Bucket, 1)), "{blocks:?}");
        }
    }

    #[test]
    fn water_poured_into_a_source_or_a_decoration_is_refused_and_the_water_bucket_kept() {
        let torch = BlockData::new(BlockId::Torch, BlockDirection::Front);
        for block in [water(0), torch] {
            let (mut player, mut world, mut input) =
                bucket_in_front_of(&[(IN_FRONT, block)], stack(ItemId::WaterBucket, 1));
            // Over the torch, at the stone
            let eyes = Vec3::new(0.5, 0.9, 0.5);
            player.position = eyes;
            input.camera = Transform::from_translation(eyes);

            right_click(&mut player, &mut world, &input);

            assert_eq!(world.get_block(IN_FRONT), Some(&block));
            assert_eq!(held(&player, 0), Some((ItemId::WaterBucket, 1)));
            // the server re-states the cell a client may have predicted water in
            assert_eq!(world.take_changed_blocks(), vec![IN_FRONT]);
        }
    }

    #[test]
    fn water_poured_into_an_unloaded_chunk_is_refused() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(0, 0, -1), IVec3::new(0, 0, -1));
        let player = place_across_a_chunk_border_holding(&mut world, ItemId::WaterBucket);
        assert_eq!(held(&player, 0), Some((ItemId::WaterBucket, 1)));
    }

    #[test]
    fn water_is_poured_where_the_player_stands_unlike_a_block() {
        for (item, poured) in [(ItemId::WaterBucket, Some(water(0))), (ItemId::Dirt, None)] {
            let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
            let floor = IVec3::new(0, -1, 0);
            world.fill_box(floor, floor, BlockId::Stone);
            let position = Vec3::splat(0.5);
            let camera = Transform::from_translation(position).looking_to(Vec3::NEG_Y, Vec3::Z);
            let mut player = Player::new(1, "tester".to_string(), position, camera);
            player.inventory.inner.insert(0, stack(item, 1));
            let input = PlayerFrameInput {
                inputs: [NetworkAction::RightClick].into(),
                camera,
                ..Default::default()
            };

            right_click(&mut player, &mut world, &input);

            assert_eq!(world.get_block(IVec3::ZERO).copied(), poured, "{item:?}");
        }
    }

    #[test]
    fn a_bucket_aimed_at_a_chest_opens_it_unless_sneaking() {
        for sneaking in [false, true] {
            let chest = BlockData::new(BlockId::Chest, BlockDirection::Front);
            let (mut player, mut world, mut input) =
                bucket_in_front_of(&[(TARGET, chest)], stack(ItemId::WaterBucket, 1));
            world.insert_container(TARGET, Container::default());
            if sneaking {
                input.inputs.insert(NetworkAction::SneakOrFlyDown);
            }

            right_click(&mut player, &mut world, &input);

            let poured = sneaking.then_some(water(0));
            assert_eq!(
                world.get_block(IN_FRONT).copied(),
                poured,
                "sneaking: {sneaking}"
            );
        }
    }

    #[test]
    fn a_bucket_acts_once_per_press_of_the_button() {
        let (mut player, mut world, mut input) =
            bucket_in_front_of(&[], stack(ItemId::WaterBucket, 1));
        input.delta_ms = 16;
        let mut released = input.clone();
        released.inputs.clear();

        // Poured; held a long while, the bucket does not take the water back
        for _ in 0..100 {
            right_click(&mut player, &mut world, &input);
        }
        assert_eq!(world.get_block(IN_FRONT), Some(&water(0)));
        assert_eq!(held(&player, 0), Some((ItemId::Bucket, 1)));

        // Released and pressed again, it does
        right_click(&mut player, &mut world, &released);
        right_click(&mut player, &mut world, &input);
        assert_eq!(world.get_block(IN_FRONT), None);
        assert_eq!(held(&player, 0), Some((ItemId::WaterBucket, 1)));
    }

    #[test]
    fn a_bucket_aimed_at_a_source_breaks_nothing_behind_it() {
        let (mut player, mut world, mut input) =
            bucket_in_front_of(&[(IN_FRONT, water(0))], stack(ItemId::Bucket, 1));
        input.inputs = [NetworkAction::LeftClick].into();
        for _ in 0..100 {
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        }
        assert_eq!(world.get_block(IN_FRONT), Some(&water(0)));
        assert_eq!(world.get_block(TARGET).unwrap().breaking_progress, 0);
    }
}
