//! Breaking and placing blocks, run by the client (prediction) and by the server (authority).

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{hunger::MAX_FOOD, movement::MAX_FRAME_DELTA_MS, sitting, Player, ViewMode},
    world::{
        fire::fire_can_stay,
        growth::{is_sapling, next_stage},
        fit_portal,
        furnace::smelting_result,
        global_block_to_chunk_pos,
        liquid::{is_source, Liquid, LAVA, WATER},
        raycast, BlockData, BlockDirection, BlockHitbox, BlockId, BlockTransparency, ChunkMap,
        decor::painting_right, BannerPatterns, Container, DecorKind, FaceDirectionExt, Form,
        Furnace, ItemId, ItemStack, ItemType, MobKind,
        PortalColor, RaycastResponse, ServerMob, Tool, ToolKind,
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
/// How far from the centre of the aimed block the portal gun shoots.
pub const PORTAL_GUN_REACH: f32 = 32.0;
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
    /// The player has all its food, and eats nothing.
    NotHungry,
    /// The block only stands on a soil that is not there.
    NoSoil(BlockId),
    /// An empty bucket fills only from the source of a liquid.
    NotASource(BlockId),
    /// No portal fits on the wall around the aimed cell.
    NoRoom,
    /// A fire is lit only in the air, on a whole block or by a block that burns.
    FireWouldNotStay,
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

/// A shot of the portal gun, placed or refused, which the players see fly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortalShot {
    /// Where the aim ray starts: the camera in first person, the player in third.
    pub from: Vec3,
    /// Where the ray meets the face of the aimed block, or, with no block aimed at, the point
    /// `PORTAL_GUN_REACH` along it.
    pub to: Vec3,
    pub color: PortalColor,
    /// Whether the shot put its portal there.
    pub placed: bool,
}

/// What the block interactions of one input did that the server acts upon.
#[derive(Debug, Default, PartialEq)]
pub struct BlockInteractions {
    pub broken: Vec<BrokenBlock>,
    pub spawned: Vec<SpawnedMob>,
    /// The full bucket a player filled and had no room for: the server drops it.
    pub dropped: Vec<ItemStack>,
    /// The signs a player reads, and the bedrolls it lies on.
    pub signs: Vec<IVec3>,
    pub bedrolls: Vec<IVec3>,
    /// The frames of maps the player clicked, to put the held map on or take the one there.
    pub frames: Vec<IVec3>,
    /// The item frames and the flower pots the player clicked, to put the held item in, turn
    /// the item shown or take the plant back (`decor`).
    pub displays: Vec<IVec3>,
    /// The banners the player placed with patterns dyed on them, which the server keeps.
    pub banners: Vec<(IVec3, BannerPatterns)>,
    /// The hotbar slots of the empty maps the player used, which the server draws.
    pub drawn_maps: Vec<u32>,
    /// The saplings given compost, which the server may grow into their tree at once.
    pub fertilized: Vec<IVec3>,
    /// What the player placed, from its hand.
    pub placed: Vec<ItemId>,
    /// The shots of the portal gun, which the server tells the other players of.
    pub shots: Vec<PortalShot>,
}

/// Breaks and places the blocks `action` asks for, or uses a spawn egg or a bucket, and returns
/// the blocks it broke, the mobs it made appear and what the player had no room for. With the
/// portal gun in the selected slot, it shoots portals instead.
pub fn simulate_player_block_interactions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) -> BlockInteractions {
    let shots = portal_shots(player, action);
    if held_item(player, action) == Some(ItemId::PortalGun) {
        let mut fired = Vec::new();
        for color in shots {
            let (shot, refusal) = shoot_portal(player, world_map, action, color);
            fired.push(shot);
            let Some(reason) = refusal else {
                continue;
            };
            log::debug!(
                "{} Player {} could not shoot a {color:?} portal: {reason:?}",
                caller_type.as_str(),
                player.id,
            );
            // As for a block, the server sends the portals again to correct a prediction
            if let CallerType::Server = caller_type {
                world_map.portals_mut().touch();
            }
        }
        return BlockInteractions {
            shots: fired,
            ..Default::default()
        };
    }

    let mut broken = Vec::new();
    let mut spawned = Vec::new();
    let mut dropped = Vec::new();
    let mut placed = Vec::new();
    let (mut signs, mut bedrolls, mut fertilized) = (Vec::new(), Vec::new(), Vec::new());
    let (mut frames, mut drawn_maps) = (Vec::new(), Vec::new());
    let (mut displays, mut banners) = (Vec::new(), Vec::new());
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
            // A bow being drawn uses nothing it is swept over
            NetworkAction::RightClick if player.bow_draw_ms > 0 || player.shield_ms > 0 => {
                continue
            }
            NetworkAction::RightClick
                if use_block(
                    player,
                    world_map,
                    action,
                    &mut dropped,
                    &mut signs,
                    &mut bedrolls,
                    &mut frames,
                    &mut displays,
                    &mut fertilized,
                ) =>
            {
                player.place_wait_ms = UNTIL_RELEASED_MS;
                continue;
            }
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
                    pour_liquid(player, world_map, action, WATER)
                        .map(|()| player.place_wait_ms = UNTIL_RELEASED_MS),
                ),
                Some(ItemId::LavaBucket) => (
                    "pour a bucket",
                    pour_liquid(player, world_map, action, LAVA)
                        .map(|()| player.place_wait_ms = UNTIL_RELEASED_MS),
                ),
                Some(food) if food.food_value().is_some() => (
                    "eat",
                    eat(player, action).map(|()| player.place_wait_ms = UNTIL_RELEASED_MS),
                ),
                // `bow::draw_bow` draws them, `shield::raise_shield` raises it; the server casts
                // and draws in a fishing rod (`server::mob::fishing`)
                Some(
                    ItemId::Bow
                    | ItemId::Crossbow
                    | ItemId::ThrowingAxe
                    | ItemId::Shield
                    | ItemId::FishingRod,
                ) => continue,
                Some(ItemId::MilkBucket) => (
                    "drink milk",
                    drink_milk(player, action).map(|()| player.place_wait_ms = UNTIL_RELEASED_MS),
                ),
                // The server draws it, as it gives it an id
                Some(ItemId::EmptyMap) => {
                    drawn_maps.push(action.hotbar_slot);
                    player.place_wait_ms = UNTIL_RELEASED_MS;
                    continue;
                }
                Some(ItemId::FlintAndSteel) => (
                    "light a fire",
                    light_fire(player, world_map, action)
                        .map(|()| player.place_wait_ms = UNTIL_RELEASED_MS),
                ),
                held => {
                    let held_type = player
                        .inventory
                        .inner
                        .get(&action.hotbar_slot)
                        .map(|stack| stack.item_type);
                    (
                        "place a block",
                        place_block(player, world_map, action).map(|(mob, at)| {
                            placed.extend(held);
                            spawned.extend(mob);
                            if let Some(at) = at {
                                // A sign just placed is written on at once
                                if world_map.get_block(at).is_some_and(|b| b.id == BlockId::Sign) {
                                    signs.push(at);
                                }
                                // The patterns of a banner stay with it
                                if let Some(ItemType::Banner { patterns }) = held_type {
                                    if !patterns.is_empty() {
                                        banners.push((at, patterns));
                                    }
                                }
                            }
                            player.place_wait_ms = PLACE_REPEAT_MS
                        }),
                    )
                }
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
        signs,
        bedrolls,
        frames,
        displays,
        banners,
        drawn_maps,
        fertilized,
        placed,
        shots: Vec::new(),
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
/// sources of the liquids too, any other item through them. The portal gun aims further than
/// the rest.
pub fn aim(
    world_map: &ChunkMap,
    camera: &Transform,
    player_position: &Vec3,
    view_mode: ViewMode,
    held: Option<ItemId>,
) -> Option<RaycastResponse> {
    match held {
        Some(ItemId::Bucket | ItemId::Boat) => {
            raycast::raycast_to_source(world_map, camera, player_position, view_mode)
        }
        Some(ItemId::PortalGun) => {
            raycast::raycast_far(world_map, camera, player_position, view_mode)
        }
        _ => raycast::raycast(world_map, camera, player_position, view_mode),
    }
}

/// The colours the portal gun shoots in `action`, whatever the player holds: blue when the left
/// button is down and was not in the previous input, orange likewise for the right one.
fn portal_shots(player: &mut Player, action: &PlayerFrameInput) -> Vec<PortalColor> {
    let held = [NetworkAction::LeftClick, NetworkAction::RightClick]
        .map(|button| action.is_pressed(button));
    let shots = [PortalColor::Blue, PortalColor::Orange]
        .into_iter()
        .zip(held.into_iter().zip(player.portal_triggers_held))
        .filter(|(_, (now, before))| *now && !before)
        .map(|(color, _)| color)
        .collect();
    player.portal_triggers_held = held;
    shots
}

/// Puts the portal of colour `color` of the player where its shot hits a block, on a wall, a
/// floor or a ceiling, `fit_portal`. Returns the shot, and why it put no portal if it did not.
fn shoot_portal(
    player: &Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    color: PortalColor,
) -> (PortalShot, Option<Refusal>) {
    let (origin, direction) = raycast::aim_ray(&action.camera, &player.position, action.view_mode);
    let mut shot = PortalShot {
        from: origin,
        to: origin + direction * PORTAL_GUN_REACH,
        color,
        placed: false,
    };
    let Some(target) = aim(
        world_map,
        &action.camera,
        &player.position,
        action.view_mode,
        held_item(player, action),
    ) else {
        return (shot, Some(Refusal::NoTarget));
    };
    // The point where the ray meets the plane of the aimed face, which a wall block fills
    let normal = target.face.to_ivec3();
    let axis = normal.abs().max_position();
    let plane = (target.position[axis] + normal[axis].max(0)) as f32;
    let hit = origin + direction * (plane - origin[axis]) / direction[axis];
    shot.to = hit;

    // Measured as the reach of a block (`aimed_block`)
    let distance = (target.position.as_vec3() + HALF_BLOCK).distance(player.position);
    if distance > PORTAL_GUN_REACH {
        return (shot, Some(Refusal::TooFar { distance }));
    }
    let Some(portal) = fit_portal(
        world_map,
        world_map.portals(),
        player.id,
        color,
        target.position,
        target.face,
        hit,
        direction,
    ) else {
        return (shot, Some(Refusal::NoRoom));
    };
    world_map.portals_mut().insert(portal);
    shot.placed = true;
    (shot, None)
}

/// The block the player aims at with the item it holds, if it is within `reach` of its centre.
/// An out of reach refusal names `predicted_at(target)` as the block to re-state.
fn aimed_block(
    player: &Player,
    world_map: &ChunkMap,
    action: &PlayerFrameInput,
    reach: f32,
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
    if distance > reach {
        return Err(Refused::at(
            predicted_at(&target),
            Refusal::TooFar { distance },
        ));
    }
    Ok(target)
}

/// The item in the selected hotbar slot.
pub fn held_item(player: &Player, action: &PlayerFrameInput) -> Option<ItemId> {
    if action.hotbar_slot >= MAX_INVENTORY_SLOTS {
        return None;
    }
    Some(player.inventory.inner.get(&action.hotbar_slot)?.item_id)
}

/// The tool in the selected hotbar slot, if it holds one.
fn held_tool(player: &Player, action: &PlayerFrameInput) -> Option<Tool> {
    held_item(player, action)?.tool()
}

/// The speed of the `tool` in the selected hotbar slot, its enchantments included.
fn held_speed(player: &Player, action: &PlayerFrameInput, tool: Tool) -> u8 {
    let stack = player.inventory.inner.get(&action.hotbar_slot);
    stack.map_or(tool.speed, |stack| stack.mining_speed(tool.speed))
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
    let position = aimed_block(player, world_map, action, INTERACTION_DISTANCE, |target| {
        target.position
    })?
    .position;

    let Some(mut block) = world_map.get_block(position).copied() else {
        return Err(Refused::at(position, Refusal::NoTarget));
    };
    let Some(break_time) = block.id.get_break_time() else {
        return Err(Refused::at(position, Refusal::Unbreakable(block.id)));
    };

    let tool = held_tool(player, action);
    let speed = tool
        .filter(|tool| block.id.is_mined_faster_by(tool.kind))
        .map_or(1, |tool| held_speed(player, action, tool));
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
    // A plant of the sea leaves the water it grew in
    if block.id.is_submerged() {
        world_map.set_block(
            position,
            BlockData::new(BlockId::Water, BlockDirection::Front),
        );
    }
    // The other half of a canvas of two blocks goes with it
    if let Some((other_half, step)) = block.id.other_painting_half(block.direction) {
        if world_map
            .get_block(position + step)
            .is_some_and(|half| half.id == other_half)
        {
            world_map.remove_block(position + step);
        }
    }
    // The other half of a door goes with it
    if let Some((_, step)) = block.id.door() {
        let other = position + step;
        if world_map
            .get_block(other)
            .is_some_and(|half| half.id.door().is_some())
        {
            world_map.remove_block(other);
        }
    }
    Ok(Some(BrokenBlock {
        position,
        block: block.id,
        contents,
        furnace,
        harvested,
    }))
}

/// Uses the aimed block, if a right click with the held item changes it: a door, a bush of ripe
/// berries, a soil and a hoe, a tilled soil and a carrot or a potato planted on it, a cistern
/// and a bucket, a campfire and what it cooks or what lights it; or if it is a sign or a bedroll,
/// which the server acts upon. Returns whether it did.
fn use_block(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    dropped: &mut Vec<ItemStack>,
    signs: &mut Vec<IVec3>,
    bedrolls: &mut Vec<IVec3>,
    frames: &mut Vec<IVec3>,
    displays: &mut Vec<IVec3>,
    fertilized: &mut Vec<IVec3>,
) -> bool {
    if action.is_pressed(NetworkAction::SneakOrFlyDown) {
        return false;
    }
    let Ok(target) = aimed_block(player, world_map, action, INTERACTION_DISTANCE, |target| {
        target.position
    }) else {
        return false;
    };
    let (pos, block) = (target.position, target.block);
    let become_block = |world_map: &mut ChunkMap, pos: IVec3, id: BlockId| {
        world_map.set_block(pos, BlockData::new(id, block.direction))
    };
    let slot = action.hotbar_slot;
    let held = held_item(player, action);
    let mut give = |player: &mut Player, item_id: ItemId, nb: u32| {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        let left = player.inventory.add_item_to_inventory(stack);
        if left > 0 {
            dropped.push(ItemStack { nb: left, ..stack });
        }
    };
    if let Some((turned, step)) = block.id.door() {
        become_block(world_map, pos, turned);
        let other = pos + step;
        let half = world_map.get_block(other).and_then(|half| half.id.door());
        if let Some((other_turned, _)) = half {
            become_block(world_map, other, other_turned);
        }
        return true;
    }
    if let Some(turned) = block.id.toggled() {
        become_block(world_map, pos, turned);
        return true;
    }
    // A canvas shows the next of its size, both halves of one of two blocks
    if let Some((next, next_right)) = block.id.next_canvas() {
        let right = painting_right(block.direction);
        let is_right = matches!(
            block.id.decor_kind(),
            Some(DecorKind::Painting { right: true, .. })
        );
        let left = if is_right { pos - right } else { pos };
        become_block(world_map, left, next);
        if let Some(next_right) = next_right {
            become_block(world_map, left + right, next_right);
        }
        return true;
    }
    match (block.id, held) {
        (BlockId::Sign, _) => signs.push(pos),
        (id, _) if id.is_bedroll() => bedrolls.push(pos),
        (id, _) if id.decor_kind() == Some(DecorKind::Chair) => {
            if !sitting::sit(player, pos, block.direction) {
                return false;
            }
        }
        (BlockId::FlowerPot | BlockId::ItemFrame, _) => displays.push(pos),
        (BlockId::MapFrame, _) => frames.push(pos),
        (BlockId::RipeBerryBush, _) => {
            become_block(world_map, pos, BlockId::BerryBush);
            give(player, ItemId::Berries, 2);
        }
        (BlockId::Dirt | BlockId::Grass, Some(item))
            if item.tool().is_some_and(|tool| tool.kind == ToolKind::Hoe)
                && world_map.get_block(pos + IVec3::Y).is_none() =>
        {
            become_block(world_map, pos, BlockId::TilledSoil);
            player.inventory.wear_tool(slot, 1);
        }
        // A carrot or a potato is planted rather than eaten
        (BlockId::TilledSoil, Some(seed @ (ItemId::Carrot | ItemId::Potato)))
            if world_map.get_block(pos + IVec3::Y).is_none()
                && world_map.is_chunk_loaded(global_block_to_chunk_pos(&(pos + IVec3::Y))) =>
        {
            let ItemType::Block(sprout) = seed.get_default_type() else {
                return false;
            };
            player.inventory.remove_item_from_stack(slot, 1);
            world_map.set_block(
                pos + IVec3::Y,
                BlockData::new(sprout, BlockDirection::Front),
            );
        }
        // Compost grows a crop by a stage, ripens a bush, and may grow a sapling at once,
        // which only the server can (`fertilized`)
        (id, Some(ItemId::Compost))
            if next_stage(id).is_some() || id == BlockId::BerryBush || is_sapling(id) =>
        {
            player.inventory.remove_item_from_stack(slot, 1);
            if let Some(grown) = next_stage(id) {
                become_block(world_map, pos, grown);
            } else if id == BlockId::BerryBush {
                become_block(world_map, pos, BlockId::RipeBerryBush);
            } else {
                fertilized.push(pos);
            }
        }
        (BlockId::FullRainCistern, Some(ItemId::Bucket)) => {
            become_block(world_map, pos, BlockId::RainCistern);
            player.inventory.remove_item_from_stack(slot, 1);
            give(player, ItemId::WaterBucket, 1);
        }
        (BlockId::RainCistern, Some(ItemId::WaterBucket)) => {
            become_block(world_map, pos, BlockId::FullRainCistern);
            player.inventory.remove_item_from_stack(slot, 1);
            give(player, ItemId::Bucket, 1);
        }
        (BlockId::UnlitCampfire, Some(ItemId::Torch | ItemId::Flint | ItemId::FlintAndSteel)) => {
            become_block(world_map, pos, BlockId::Campfire);
        }
        // A campfire cooks at once what the furnace cooks into a food
        (BlockId::Campfire, Some(raw))
            if smelting_result(raw).is_some_and(|cooked| cooked.food_value().is_some()) =>
        {
            player.inventory.remove_item_from_stack(slot, 1);
            if let Some(roast) = smelting_result(raw) {
                give(player, roast, 1);
            }
        }
        _ => return false,
    }
    true
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
    let target = aimed_block(
        player,
        world_map,
        action,
        INTERACTION_DISTANCE,
        new_block_position,
    )?;
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

/// Places the block held in the selected hotbar slot against the aimed face, and returns where
/// it placed it. A spawn egg makes its mob appear there instead, standing on the bottom of the
/// cell, and is returned.
fn place_block(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<(Option<SpawnedMob>, Option<IVec3>), Refused> {
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
        let mob = SpawnedMob {
            kind,
            position: ServerMob::standing_at(kind, cell.x + 0.5, cell.y, cell.z + 0.5),
        };
        return Ok((Some(mob), None));
    }
    if block_overlaps_player(position, player) {
        return refused(Refusal::InsidePlayer);
    }
    let block_id = match item.item_type {
        ItemType::Block(block_id) => block_id,
        // a banner with patterns places its block all the same
        ItemType::Banner { .. } => match item.item_id.decor_block() {
            Some(block_id) => block_id,
            None => return refused(Refusal::NotABlock(item.item_type)),
        },
        _ => return refused(Refusal::NotABlock(item.item_type)),
    };
    // A carpet lies on a block
    if block_id.decor_kind() == Some(DecorKind::Carpet)
        && !world_map
            .get_block(position - IVec3::Y)
            .is_some_and(|below| !below.id.is_replaceable())
    {
        return refused(Refusal::NoSoil(block_id));
    }

    if let Some(soils) = block_id.soil() {
        let below = world_map.get_block(position - IVec3::Y);
        if !below.is_some_and(|below| soils.contains(&below.id)) {
            return refused(Refusal::NoSoil(block_id));
        }
    }
    // A plant of the sea grows only in the water, a hanging vine only under a block
    let in_water = world_map
        .get_block(position)
        .is_some_and(|block| block.id == BlockId::Water);
    if block_id.is_submerged() && !in_water {
        return refused(Refusal::NoSoil(block_id));
    }
    if block_id.hangs()
        && !world_map
            .get_block(position + IVec3::Y)
            .is_some_and(|above| {
                above.id == block_id || matches!(above.get_hitbox(), BlockHitbox::FullBlock)
            })
    {
        return refused(Refusal::NoSoil(block_id));
    }
    if let Some(top) = block_id.door_top() {
        let above = position + IVec3::Y;
        if world_map.get_block(above).is_some()
            || !world_map.is_chunk_loaded(global_block_to_chunk_pos(&above))
        {
            return refused(Refusal::Occupied);
        }
        let top = BlockData::new(top, facing(&action.camera));
        world_map.set_block(above, top);
    }
    // A lantern placed under a block hangs from it
    let hangs = (block_id == BlockId::Lantern || block_id.hanging_lantern().is_some())
        && target.face.to_ivec3() == IVec3::NEG_Y;
    let block_id = if hangs {
        block_id.hanging_lantern().unwrap_or(BlockId::HangingLantern)
    } else {
        block_id
    };

    // A vine is turned toward the block it is placed against, a furnace toward the player
    let direction = match block_id {
        BlockId::Vine | BlockId::Ladder => {
            let Some(toward) = BlockDirection::toward(-target.face.to_ivec3()) else {
                return refused(Refusal::NotASide);
            };
            toward
        }
        BlockId::Furnace | BlockId::Door | BlockId::Bedroll | BlockId::Sign | BlockId::MapFrame => {
            facing(&action.camera)
        }
        id if id.form().is_some_and(Form::faces_placer) => facing(&action.camera),
        // A frame, a canvas or a banner hangs on the wall it is placed against
        id if id.decor_kind().is_some_and(DecorKind::on_wall) => {
            let Some(toward) = BlockDirection::toward(-target.face.to_ivec3()) else {
                return refused(Refusal::NotASide);
            };
            toward
        }
        id if id.decor_kind().is_some_and(DecorKind::faces_placer) => facing(&action.camera),
        _ => BlockDirection::Front,
    };
    // A canvas of two blocks takes the cell on its right too
    if let Some((right_half, step)) = block_id.other_painting_half(direction) {
        let right = position + step;
        let free = world_map
            .get_block(right)
            .is_none_or(|block| block.id.is_replaceable());
        if !free
            || !world_map.is_chunk_loaded(global_block_to_chunk_pos(&right))
            || block_overlaps_player(right, player)
        {
            return refused(Refusal::Occupied);
        }
        world_map.set_block(right, BlockData::new(right_half, direction));
    }

    player.inventory.remove_item_from_stack(slot, 1);
    world_map.set_block(position, BlockData::new(block_id, direction));
    if block_id.is_container() {
        world_map.insert_container(position, Container::default());
    }
    if block_id == BlockId::Furnace {
        world_map.insert_furnace(position, Furnace::default());
    }
    Ok((None, Some(position)))
}

/// Eats one of the food held in the selected hotbar slot, which gives food and saturation. It is
/// refused to a player who has all its food, so that no food is lost.
fn eat(player: &mut Player, action: &PlayerFrameInput) -> Result<(), Refused> {
    let slot = action.hotbar_slot;
    let refused = |reason| Refused {
        reason,
        predicted_at: None,
    };
    let (food, saturation) = held_item(player, action)
        .and_then(|item| item.food_value())
        .ok_or(refused(Refusal::EmptySlot(slot)))?;
    let item = held_item(player, action).ok_or(refused(Refusal::EmptySlot(slot)))?;
    // A healing food is eaten for its effect, even by a player who has all its food
    if player.food >= MAX_FOOD && item.food_effect().is_none() {
        return Err(refused(Refusal::NotHungry));
    }
    player.inventory.remove_item_from_stack(slot, 1);
    player.eat(food, saturation);
    if let Some((kind, level, ticks)) = item.food_effect() {
        player.effects.give(kind, level, ticks);
    }
    // The bowl of a soup is left in the hand
    if let Some(remains) = item.eaten_remains() {
        let bowl = ItemStack {
            item_id: remains,
            item_type: remains.get_default_type(),
            nb: 1,
        };
        if player.inventory.inner.contains_key(&slot) {
            player.inventory.add_item_to_inventory(bowl);
        } else {
            player.inventory.inner.insert(slot, bowl);
        }
    }
    Ok(())
}

/// Drinks the bucket of milk held in the selected hotbar slot, which takes every status effect
/// away and leaves the empty bucket in its place. It is refused to a player under none, so that
/// no milk is lost.
fn drink_milk(player: &mut Player, action: &PlayerFrameInput) -> Result<(), Refused> {
    let slot = action.hotbar_slot;
    let refused = |reason| Refused {
        reason,
        predicted_at: None,
    };
    if held_item(player, action) != Some(ItemId::MilkBucket) {
        return Err(refused(Refusal::EmptySlot(slot)));
    }
    if player.effects.is_empty() {
        return Err(refused(Refusal::NotHungry));
    }
    player.effects.clear();
    player.slowness = 0;
    player.inventory.inner.insert(
        slot,
        ItemStack {
            item_id: ItemId::Bucket,
            item_type: ItemId::Bucket.get_default_type(),
            nb: 1,
        },
    );
    Ok(())
}

/// Fills the bucket held in the selected hotbar slot from the source the player aims at, water
/// or lava, which is taken away. A single bucket becomes the full bucket; from a stack, the
/// full bucket goes into the inventory, and is returned when there is no room for it.
fn fill_bucket(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<Option<ItemStack>, Refused> {
    let target = aimed_block(player, world_map, action, INTERACTION_DISTANCE, |target| {
        target.position
    })?;
    refuse_used_block(world_map, &target, action)?;
    if !is_source(&target.block) {
        return Err(Refused::at(
            target.position,
            Refusal::NotASource(target.block.id),
        ));
    }

    let full = if target.block.id == BlockId::Lava {
        ItemId::LavaBucket
    } else {
        ItemId::WaterBucket
    };
    world_map.remove_block(target.position);
    let water_bucket = ItemStack {
        item_id: full,
        item_type: full.get_default_type(),
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

/// Pours the `liquid` of the full bucket held in the selected hotbar slot against the aimed
/// face, where a block would be placed: a source, which the flow of the server then spreads.
/// The full bucket becomes a bucket. A liquid stops nobody, so it may be poured where the
/// player stands; into a source, it would change nothing, and the full bucket is kept.
fn pour_liquid(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    liquid: Liquid,
) -> Result<(), Refused> {
    let (_, position) = cell_against_aimed_face(player, world_map, action)?;
    if world_map.get_block(position).is_some_and(is_source) {
        return Err(Refused::at(position, Refusal::Occupied));
    }

    world_map.set_block(position, liquid.at(0));
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

/// Lights a fire with the flint and steel held in the selected hotbar slot, in the air cell
/// against the aimed face, where a fire stays (`fire_can_stay`). The flint and steel wears by
/// one. A fire stops nobody: it may be lit where the player stands.
fn light_fire(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
) -> Result<(), Refused> {
    let (_, position) = cell_against_aimed_face(player, world_map, action)?;
    if world_map.get_block(position).is_some() {
        return Err(Refused::at(position, Refusal::Occupied));
    }
    if !fire_can_stay(world_map, position) {
        return Err(Refused::at(position, Refusal::FireWouldNotStay));
    }
    world_map.set_block(
        position,
        BlockData::new(BlockId::Fire, BlockDirection::Front),
    );
    player.inventory.wear_tool(action.hotbar_slot, 1);
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
    use crate::world::FaceDirection;
    use crate::world::{BlockId, Furnace, ItemId, ItemStack, PortalColor};
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
    fn a_wide_painting_hangs_on_two_cells_of_the_wall_and_a_click_shows_the_next_canvas() {
        // seen from the south, looking north at the wall: the right half to the east
        let stone = IVec3::new(0, 0, -3);
        let (mut world, mut player) = place_against(ItemId::WidePainting, stone, IVec3::Z);
        let left = world.get_block(stone + IVec3::Z).copied().unwrap();
        let right = world.get_block(stone + IVec3::Z + IVec3::X).copied().unwrap();
        assert_eq!(left.id, BlockId::ValleyPainting);
        assert_eq!(right.id, BlockId::ValleyPaintingRight);
        assert_eq!(left.direction, BlockDirection::Front);
        assert!(player.inventory.inner.is_empty());
        // a click on the right half
        player.place_wait_ms = 0;
        let camera = Transform::from_translation(player.position)
            .looking_at(stone.as_vec3() + Vec3::new(1.5, 0.5, 1.1), Vec3::Y);
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera,
            ..Default::default()
        };
        simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        assert_eq!(world.get_block(stone + IVec3::Z).unwrap().id, BlockId::SeascapePainting);
        assert_eq!(
            world.get_block(stone + IVec3::Z + IVec3::X).unwrap().id,
            BlockId::SeascapePaintingRight
        );
    }

    #[test]
    fn an_item_frame_is_hung_on_a_side_only_and_a_click_on_it_is_told_to_the_server() {
        let stone = IVec3::new(0, 0, -3);
        let (world, _) = place_against(ItemId::ItemFrame, stone, IVec3::Y);
        assert!(world.get_block(stone + IVec3::Y).is_none());
        let (mut world, mut player) = place_against(ItemId::ItemFrame, stone, IVec3::Z);
        assert_eq!(world.get_block(stone + IVec3::Z).unwrap().id, BlockId::ItemFrame);
        player.place_wait_ms = 0;
        let input = PlayerFrameInput {
            inputs: [NetworkAction::RightClick].into(),
            camera: Transform::from_translation(player.position)
                .looking_at(stone.as_vec3() + Vec3::new(0.5, 0.5, 1.0), Vec3::Y),
            ..Default::default()
        };
        let done =
            simulate_player_block_interactions(&mut player, &mut world, &input, CallerType::Server);
        assert_eq!(done.displays, vec![stone + IVec3::Z]);
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
    fn a_bucket_takes_a_lava_source_into_a_lava_bucket_which_pours_it_back() {
        let lava = LAVA.at(0);
        let (mut player, mut world, input) =
            bucket_in_front_of(&[(IN_FRONT, lava)], stack(ItemId::Bucket, 1));
        right_click(&mut player, &mut world, &input);
        assert_eq!(world.get_block(IN_FRONT), None);
        assert_eq!(held(&player, 0), Some((ItemId::LavaBucket, 1)));

        // flowing lava is not taken
        let (mut player, mut world, input) =
            bucket_in_front_of(&[(IN_FRONT, LAVA.at(2))], stack(ItemId::Bucket, 1));
        right_click(&mut player, &mut world, &input);
        assert_eq!(held(&player, 0), Some((ItemId::Bucket, 1)));

        let (mut player, mut world, input) = bucket_in_front_of(&[], stack(ItemId::LavaBucket, 1));
        right_click(&mut player, &mut world, &input);
        assert_eq!(world.get_block(IN_FRONT), Some(&lava));
        assert_eq!(held(&player, 0), Some((ItemId::Bucket, 1)));
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
    fn a_bucket_aims_as_on_main_reaching_five_blocks_and_seeing_twenty() {
        // A lone source straight ahead, `distance` blocks from the centre of the player
        let bucket_toward = |distance: i32| {
            let (mut player, _, input) = bucket_in_front_of(&[], stack(ItemId::Bucket, 1));
            let mut world =
                ChunkMap::with_loaded_region(IVec3::new(-16, -16, -48), IVec3::splat(15));
            let source = IVec3::new(0, 0, -distance);
            world.set_block(source, water(0));
            world.take_changed_blocks();
            let aimed = aim(
                &world,
                &input.camera,
                &player.position,
                input.view_mode,
                Some(ItemId::Bucket),
            )
            .map(|target| target.position);
            right_click(&mut player, &mut world, &input);
            (
                aimed,
                world.get_block(source).copied(),
                world.take_changed_blocks(),
            )
        };
        // Within reach, it fills
        let source = IVec3::new(0, 0, -5);
        assert_eq!(bucket_toward(5), (Some(source), None, vec![source]));
        // Beyond, the source is aimed at, kept and re-stated to the client
        let source = IVec3::new(0, 0, -6);
        assert_eq!(
            bucket_toward(6),
            (Some(source), Some(water(0)), vec![source])
        );
        // Beyond the 20 blocks rays go, but the gun's, it is not even aimed at
        assert_eq!(bucket_toward(25), (None, Some(water(0)), vec![]));
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

    /// The flint and steel held in the first slot, worn by `wear`.
    fn wear_of_lighter(player: &Player) -> Option<u32> {
        match player.inventory.inner.get(&0)? {
            ItemStack {
                item_id: ItemId::FlintAndSteel,
                item_type: ItemType::Tool { wear },
                ..
            } => Some(*wear),
            _ => None,
        }
    }

    fn lighter(wear: u32) -> ItemStack {
        ItemStack {
            item_type: ItemType::Tool { wear },
            ..stack(ItemId::FlintAndSteel, 1)
        }
    }

    const UNDER_IN_FRONT: IVec3 = IVec3::new(0, -1, -2);

    #[test]
    fn a_flint_and_steel_lights_a_fire_against_the_aimed_face_and_wears_once_per_press() {
        let stone = BlockData::new(BlockId::Stone, BlockDirection::Front);
        let (mut player, mut world, input) =
            bucket_in_front_of(&[(UNDER_IN_FRONT, stone)], lighter(0));

        right_click(&mut player, &mut world, &input);

        let fire = BlockData::new(BlockId::Fire, BlockDirection::Front);
        assert_eq!(world.get_block(IN_FRONT), Some(&fire));
        assert_eq!(world.take_changed_blocks(), vec![IN_FRONT]);
        assert_eq!(wear_of_lighter(&player), Some(1));
        // held down, it lights nothing more, even once the fire is gone
        world.remove_block(IN_FRONT);
        right_click(&mut player, &mut world, &input);
        assert_eq!(world.get_block(IN_FRONT), None);
        assert_eq!(wear_of_lighter(&player), Some(1));
    }

    #[test]
    fn a_flint_and_steel_lights_nothing_where_a_fire_would_not_stay_nor_in_water() {
        let stone = BlockData::new(BlockId::Stone, BlockDirection::Front);
        for blocks in [vec![], vec![(UNDER_IN_FRONT, stone), (IN_FRONT, water(0))]] {
            let (mut player, mut world, input) = bucket_in_front_of(&blocks, lighter(0));

            right_click(&mut player, &mut world, &input);

            let fire = world
                .get_block(IN_FRONT)
                .is_some_and(|block| block.id == BlockId::Fire);
            assert!(!fire, "{blocks:?}");
            assert_eq!(wear_of_lighter(&player), Some(0), "{blocks:?}");
            assert_eq!(world.take_changed_blocks(), vec![IN_FRONT], "{blocks:?}");
        }
    }

    #[test]
    fn a_flint_and_steel_breaks_at_its_last_fire() {
        let stone = BlockData::new(BlockId::Stone, BlockDirection::Front);
        let durability = ItemId::FlintAndSteel.durability().unwrap();
        let (mut player, mut world, input) =
            bucket_in_front_of(&[(UNDER_IN_FRONT, stone)], lighter(durability - 1));
        right_click(&mut player, &mut world, &input);
        assert_eq!(world.get_block(IN_FRONT).map(|b| b.id), Some(BlockId::Fire));
        assert_eq!(player.inventory.inner.get(&0), None);
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

    /// A stone wall facing +Z, 8 wide and 4 high: x in 0..=7, y in 10..=13, z = 0.
    fn gun_range() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(-8, 0, -8), IVec3::new(16, 40, 48));
        world.fill_box(IVec3::new(0, 10, 0), IVec3::new(7, 13, 0), BlockId::Stone);
        world.take_changed_blocks();
        world.portals_mut().take_changed();
        world
    }

    /// A player holding the portal gun, its eyes `distance` blocks in front of the wall at
    /// `(x, y)`, looking straight at it.
    fn gunner(x: f32, y: f32, distance: f32) -> Player {
        let position = Vec3::new(x, y, 1.0 + distance);
        let camera = Transform::from_translation(position).looking_to(Vec3::NEG_Z, Vec3::Y);
        let mut player = Player::new(1, "gunner".to_string(), position, camera);
        player
            .inventory
            .inner
            .insert(0, stack(ItemId::PortalGun, 1));
        player
    }

    /// One input of `player`, pressing `buttons`, aiming as its camera does.
    fn shoot(
        player: &mut Player,
        world: &mut ChunkMap,
        buttons: &[NetworkAction],
        caller_type: CallerType,
    ) {
        let input = PlayerFrameInput {
            delta_ms: 16,
            inputs: buttons.iter().cloned().collect(),
            camera: player.camera_transform,
            ..Default::default()
        };
        simulate_player_block_interactions(player, world, &input, caller_type);
    }

    fn anchor(world: &ChunkMap, color: PortalColor) -> Option<IVec3> {
        world.portals().get(1, color).map(|portal| portal.anchor)
    }

    #[test]
    fn the_gun_shoots_blue_on_a_left_click_and_orange_on_a_right_click() {
        let mut world = gun_range();
        let mut player = gunner(1.9, 11.9, 5.0);
        shoot(
            &mut player,
            &mut world,
            &[NetworkAction::LeftClick],
            CallerType::Server,
        );
        assert_eq!(
            anchor(&world, PortalColor::Blue),
            Some(IVec3::new(1, 10, 0))
        );
        assert_eq!(anchor(&world, PortalColor::Orange), None);
        assert!(world.portals_mut().take_changed());

        let mut player = gunner(5.9, 11.9, 5.0);
        shoot(
            &mut player,
            &mut world,
            &[NetworkAction::RightClick],
            CallerType::Server,
        );
        assert_eq!(
            anchor(&world, PortalColor::Orange),
            Some(IVec3::new(5, 10, 0))
        );
        let blue = world.portals().get(1, PortalColor::Blue).unwrap();
        assert!(world.portals().twin(blue).is_some());

        // The gun neither mines the wall nor places a block in front of it
        assert!(world.take_changed_blocks().is_empty());
        assert_eq!(world.get_block(IVec3::new(5, 11, 1)), None);
        assert_eq!(player.inventory.inner[&0].nb, 1);
    }

    #[test]
    fn the_gun_shoots_once_per_press() {
        let mut world = gun_range();
        let mut player = gunner(1.9, 11.9, 5.0);
        let left = [NetworkAction::LeftClick];
        shoot(&mut player, &mut world, &left, CallerType::Server);
        world.portals_mut().remove(1, PortalColor::Blue);
        for _ in 0..20 {
            shoot(&mut player, &mut world, &left, CallerType::Server);
        }
        assert_eq!(anchor(&world, PortalColor::Blue), None);
        shoot(&mut player, &mut world, &[], CallerType::Server);
        shoot(&mut player, &mut world, &left, CallerType::Server);
        assert_eq!(
            anchor(&world, PortalColor::Blue),
            Some(IVec3::new(1, 10, 0))
        );
    }

    #[test]
    fn taking_the_gun_while_a_button_is_held_shoots_nothing() {
        let mut world = gun_range();
        let mut player = gunner(1.9, 11.9, 5.0);
        let gun = player.inventory.inner.remove(&0).unwrap();
        let left = [NetworkAction::LeftClick];
        shoot(&mut player, &mut world, &left, CallerType::Server);
        player.inventory.inner.insert(0, gun);
        shoot(&mut player, &mut world, &left, CallerType::Server);
        assert_eq!(anchor(&world, PortalColor::Blue), None);
    }

    #[test]
    fn shooting_a_colour_again_moves_its_portal() {
        let mut world = gun_range();
        let left = [NetworkAction::LeftClick];
        shoot(
            &mut gunner(1.9, 11.9, 5.0),
            &mut world,
            &left,
            CallerType::Server,
        );
        shoot(
            &mut gunner(5.9, 11.9, 5.0),
            &mut world,
            &left,
            CallerType::Server,
        );
        assert_eq!(
            anchor(&world, PortalColor::Blue),
            Some(IVec3::new(5, 10, 0))
        );
        assert_eq!(world.portals().iter().count(), 1);
    }

    #[test]
    fn the_gun_reaches_32_blocks() {
        let left = [NetworkAction::LeftClick];
        // From the centre of the player to that of the aimed block, half a block deeper and a
        // little aside: about 31.9 and 32.1 blocks
        for (distance, placed) in [(31.4, true), (31.6, false)] {
            let mut world = gun_range();
            shoot(
                &mut gunner(1.9, 11.9, distance),
                &mut world,
                &left,
                CallerType::Server,
            );
            assert_eq!(
                anchor(&world, PortalColor::Blue).is_some(),
                placed,
                "{distance}"
            );
        }
    }

    #[test]
    fn the_gun_places_portals_on_a_floor_and_a_ceiling_along_its_facing() {
        // A stone floor at y = 10 and a ceiling at y = 20, x in 0..=7, z in -6..=6
        let mut world = gun_range();
        world.fill_box(IVec3::new(0, 10, -6), IVec3::new(7, 10, 6), BlockId::Stone);
        world.fill_box(IVec3::new(0, 20, -6), IVec3::new(7, 20, 6), BlockId::Stone);
        let mut player = gunner(0.0, 0.0, 0.0);
        player.position = Vec3::new(3.5, 12.0, 3.5);
        let eyes = player.position + Vec3::Y * 0.8;
        // Looking down to the north-west, mostly north, then up to the east
        player.camera_transform =
            Transform::from_translation(eyes).looking_to(Vec3::new(-0.3, -1.0, -0.8), Vec3::Y);
        let left = [NetworkAction::LeftClick];
        shoot(&mut player, &mut world, &left, CallerType::Server);
        player.camera_transform =
            Transform::from_translation(eyes).looking_to(Vec3::new(0.3, 1.0, 0.1), Vec3::Y);
        shoot(&mut player, &mut world, &[], CallerType::Server);
        shoot(
            &mut player,
            &mut world,
            &[NetworkAction::RightClick],
            CallerType::Server,
        );

        let blue = *world.portals().get(1, PortalColor::Blue).unwrap();
        assert_eq!(
            (blue.face, blue.up),
            (FaceDirection::PlusY, FaceDirection::MinusZ)
        );
        assert!(blue.cells().all(|cell| cell.y == 10));
        let orange = *world.portals().get(1, PortalColor::Orange).unwrap();
        assert_eq!(
            (orange.face, orange.up),
            (FaceDirection::MinusY, FaceDirection::PlusX)
        );
        assert!(orange.cells().all(|cell| cell.y == 20));
        assert!(world.portals().twin(&blue).is_some());
    }

    #[test]
    fn a_shot_refused_by_the_server_sends_the_portals_again_and_a_client_does_nothing() {
        for (caller_type, sent) in [(CallerType::Server, true), (CallerType::Client, false)] {
            let mut world = gun_range();
            // The wall is too narrow there
            let mut player = gunner(7.9, 13.9, 5.0);
            world.fill_box(IVec3::new(6, 10, 0), IVec3::new(6, 13, 0), BlockId::Glass);
            world.portals_mut().take_changed();
            shoot(
                &mut player,
                &mut world,
                &[NetworkAction::LeftClick],
                caller_type,
            );
            assert!(world.portals().is_empty());
            assert_eq!(world.portals_mut().take_changed(), sent, "{caller_type:?}");
        }
    }

    /// The shots of one input of `player` pressing the left button, aiming as its camera does.
    fn shots_of(player: &mut Player, world: &mut ChunkMap) -> Vec<PortalShot> {
        let input = PlayerFrameInput {
            delta_ms: 16,
            inputs: [NetworkAction::LeftClick].into(),
            camera: player.camera_transform,
            ..Default::default()
        };
        simulate_player_block_interactions(player, world, &input, CallerType::Server).shots
    }

    #[test]
    fn a_shot_flies_from_the_eye_to_where_the_aim_meets_the_wall() {
        let mut world = gun_range();
        let mut player = gunner(1.9, 11.9, 5.0);
        player.camera_transform = player
            .camera_transform
            .looking_to(Vec3::new(0.1, -0.1, -1.0), Vec3::Y);
        let eye = player.camera_transform.translation;
        let shots = shots_of(&mut player, &mut world);
        assert_eq!(shots.len(), 1, "{shots:?}");
        let shot = shots[0];
        assert_eq!(
            (shot.from, shot.color, shot.placed),
            (eye, PortalColor::Blue, true)
        );
        // On the face of the wall, at z = 1, along the aim
        assert!(
            shot.to.distance(Vec3::new(2.4, 11.4, 1.0)) < 1e-4,
            "{:?}",
            shot.to
        );
        // Released, the button shoots nothing
        player.portal_triggers_held = [false; 2];
        assert_eq!(
            shoot_nothing(&mut player, &mut world),
            [],
            "a shot without a press"
        );
    }

    /// The shots of one input of `player` pressing nothing.
    fn shoot_nothing(player: &mut Player, world: &mut ChunkMap) -> Vec<PortalShot> {
        let input = PlayerFrameInput {
            delta_ms: 16,
            camera: player.camera_transform,
            ..Default::default()
        };
        simulate_player_block_interactions(player, world, &input, CallerType::Server).shots
    }

    #[test]
    fn a_refused_shot_flies_too_to_the_wall_or_as_far_as_the_gun_reaches() {
        // The wall is too narrow there
        let mut world = gun_range();
        world.fill_box(IVec3::new(6, 10, 0), IVec3::new(6, 13, 0), BlockId::Glass);
        let mut player = gunner(7.9, 13.9, 5.0);
        let shots = shots_of(&mut player, &mut world);
        assert_eq!(shots.len(), 1, "{shots:?}");
        assert!(!shots[0].placed);
        assert!(shots[0].to.distance(Vec3::new(7.9, 13.9, 1.0)) < 1e-4);

        // At the sky: as far as the gun reaches along the aim
        let mut player = gunner(1.9, 11.9, 5.0);
        player.camera_transform = player.camera_transform.looking_to(Vec3::Y, Vec3::Z);
        let eye = player.camera_transform.translation;
        let shots = shots_of(&mut player, &mut world);
        assert_eq!(shots.len(), 1, "{shots:?}");
        assert!(!shots[0].placed);
        assert!(shots[0].to.distance(eye + Vec3::Y * PORTAL_GUN_REACH) < 1e-4);
    }

    /// The gun's ray goes through what a player walks through, as the front of a portal lets
    /// it be: a vine on the wall, tall grass, a fern, flowers, a torch. A block that collides,
    /// leaves or a cocoa pod, stops it, and no portal goes there.
    #[test]
    fn the_gun_shoots_through_what_a_player_walks_through() {
        use crate::world::BlockDirection;
        let walk_through = [
            (IVec3::new(1, 11, 1), BlockId::Vine),
            (IVec3::new(1, 11, 2), BlockId::TallGrass),
            (IVec3::new(1, 11, 2), BlockId::Fern),
            (IVec3::new(1, 11, 3), BlockId::Poppy),
            (IVec3::new(1, 11, 3), BlockId::Dandelion),
            (IVec3::new(1, 11, 4), BlockId::Torch),
        ];
        for (cell, id) in walk_through {
            let mut world = gun_range();
            // A vine hangs on the wall behind it, north
            world.set_block(cell, BlockData::new(id, BlockDirection::Front));
            world.take_changed_blocks();
            let mut player = gunner(1.5, 11.3, 5.0);
            let shots = shots_of(&mut player, &mut world);
            assert_eq!(shots.len(), 1, "{id:?}: {shots:?}");
            assert!(shots[0].placed, "{id:?}: {shots:?}");
            assert!((shots[0].to.z - 1.0).abs() < 1e-4, "{id:?}: {shots:?}");
            assert!(anchor(&world, PortalColor::Blue).is_some(), "{id:?}");
        }
        for id in [BlockId::OakLeaves, BlockId::Glass] {
            let mut world = gun_range();
            world.set_block(
                IVec3::new(1, 11, 3),
                BlockData::new(id, BlockDirection::Front),
            );
            world.take_changed_blocks();
            let mut player = gunner(1.5, 11.3, 5.0);
            let shots = shots_of(&mut player, &mut world);
            assert!(!shots[0].placed, "{id:?}: {shots:?}");
            assert!((shots[0].to.z - 4.0).abs() < 1e-4, "{id:?}: {shots:?}");
        }
        // A cocoa pod, the one block whose box is not the whole cell, stops it at its cell
        let mut world = gun_range();
        world.set_block(
            IVec3::new(1, 11, 3),
            BlockData::new(BlockId::Cocoa, BlockDirection::Front),
        );
        world.take_changed_blocks();
        let mut player = gunner(1.5, 11.3, 5.0);
        let shots = shots_of(&mut player, &mut world);
        assert!(!shots[0].placed, "{shots:?}");
        assert!(
            shots[0].to.z > 3.0 && shots[0].to.z < 4.0 + 1e-4,
            "{shots:?}"
        );
        assert!(anchor(&world, PortalColor::Blue).is_none());
        // The hand still aims at the vine, which it breaks
        let mut world = gun_range();
        let vine = BlockData::new(BlockId::Vine, BlockDirection::Front);
        world.set_block(IVec3::new(1, 11, 1), vine);
        let player = gunner(1.5, 11.3, 2.0);
        let hit = aim(
            &world,
            &player.camera_transform,
            &player.position,
            ViewMode::FirstPerson,
            None,
        );
        assert_eq!(hit.map(|hit| hit.position), Some(IVec3::new(1, 11, 1)));
    }

    #[test]
    fn the_gun_shoots_through_the_water_sources_that_only_a_bucket_aims_at() {
        let mut world = gun_range();
        // Water sources before the wall, which a bucket would aim at
        for z in 1..=3 {
            for x in 0..=3 {
                for y in 10..=13 {
                    world.set_block(IVec3::new(x, y, z), water(0));
                }
            }
        }
        world.take_changed_blocks();
        let mut player = gunner(1.9, 11.9, 5.0);
        shoot(
            &mut player,
            &mut world,
            &[NetworkAction::RightClick],
            CallerType::Server,
        );
        assert_eq!(
            anchor(&world, PortalColor::Orange),
            Some(IVec3::new(1, 10, 0))
        );
        // Nothing poured nor taken
        assert!(world.take_changed_blocks().is_empty());
    }
}

#[cfg(test)]
mod food_tests {
    use super::*;

    #[test]
    fn food_feeds_a_hungry_player_and_is_kept_by_a_fed_one() {
        let item = ItemId::RoastMushroom;
        for (before, after, left) in [
            (MAX_FOOD, MAX_FOOD, 2),
            (MAX_FOOD - 1, MAX_FOOD, 1),
            (3, 7, 1),
        ] {
            let camera = Transform::from_translation(Vec3::splat(0.5));
            let mut player = Player::new(1, "tester".into(), Vec3::splat(0.5), camera);
            player.food = before;
            player.saturation = 0.0;
            // The health has nothing to do with it
            player.health = 3;
            let stack = ItemStack {
                item_id: item,
                item_type: item.get_default_type(),
                nb: 2,
            };
            player.inventory.inner.insert(0, stack);
            // No block around: food is eaten whatever the player aims at
            let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
            let input = PlayerFrameInput {
                delta_ms: HIT_MS,
                inputs: [NetworkAction::RightClick].into(),
                camera,
                ..Default::default()
            };
            // Held, the button eats once
            for _ in 0..10 {
                simulate_player_block_interactions(
                    &mut player,
                    &mut world,
                    &input,
                    CallerType::Server,
                );
            }
            assert_eq!(player.food, after);
            assert_eq!(player.health, 3);
            assert_eq!(player.inventory.inner[&0].nb, left);
            // The saturation of a mushroom, but no more than the food
            let saturation = if left == 1 {
                4.8f32.min(after as f32)
            } else {
                0.0
            };
            assert_eq!(player.saturation, saturation);
        }
    }
}

#[cfg(test)]
mod use_tests {
    use super::*;

    const AT: IVec3 = IVec3::new(0, 0, -3);

    /// A player looking at `AT`, which holds `block`, with `item` in hand.
    fn facing_a(block: BlockId, item: Option<ItemId>) -> (Player, ChunkMap) {
        let camera = Transform::from_translation(Vec3::splat(0.5));
        let mut player = Player::new(1, "tester".into(), Vec3::splat(0.5), camera);
        if let Some(item_id) = item {
            let stack = ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb: 1,
            };
            player.inventory.inner.insert(0, stack);
        }
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        world.set_block(AT, BlockData::new(block, BlockDirection::Back));
        (player, world)
    }

    fn click(player: &mut Player, world: &mut ChunkMap, action: NetworkAction) {
        let input = PlayerFrameInput {
            delta_ms: 100,
            inputs: [action].into(),
            camera: player.camera_transform,
            ..Default::default()
        };
        simulate_player_block_interactions(player, world, &input, CallerType::Server);
    }

    fn id(world: &ChunkMap, pos: IVec3) -> Option<BlockId> {
        world.get_block(pos).map(|block| block.id)
    }

    #[test]
    fn a_right_click_turns_the_block_into_another() {
        for (block, item, after) in [
            (BlockId::RipeBerryBush, None, BlockId::BerryBush),
            (BlockId::Grass, Some(ItemId::StoneHoe), BlockId::TilledSoil),
            (
                BlockId::FullRainCistern,
                Some(ItemId::Bucket),
                BlockId::RainCistern,
            ),
            (
                BlockId::UnlitCampfire,
                Some(ItemId::Flint),
                BlockId::Campfire,
            ),
            (BlockId::Grass, Some(ItemId::StoneAxe), BlockId::Grass),
        ] {
            let (mut player, mut world) = facing_a(block, item);
            // A campfire is low: the player looks down at it
            player
                .camera_transform
                .look_to(Vec3::new(0.0, -0.1, -1.0), Vec3::Y);
            click(&mut player, &mut world, NetworkAction::RightClick);
            assert_eq!(id(&world, AT), Some(after), "{block:?}");
        }
    }

    #[test]
    fn a_campfire_roasts_the_raw_meats_and_the_mushroom() {
        for (raw, roast) in [
            (ItemId::RawMutton, ItemId::RoastMutton),
            (ItemId::RawPork, ItemId::RoastPork),
            (ItemId::RawBeef, ItemId::RoastBeef),
            (ItemId::RawChicken, ItemId::RoastChicken),
            (ItemId::RawRabbit, ItemId::RoastRabbit),
            (ItemId::Mushroom, ItemId::RoastMushroom),
            (ItemId::Potato, ItemId::BakedPotato),
            (ItemId::RawEel, ItemId::GrilledEel),
            (ItemId::Egg, ItemId::FriedEgg),
            (ItemId::RawPie, ItemId::ApplePie),
        ] {
            let (mut player, mut world) = facing_a(BlockId::Campfire, Some(raw));
            player
                .camera_transform
                .look_to(Vec3::new(0.0, -0.1, -1.0), Vec3::Y);
            click(&mut player, &mut world, NetworkAction::RightClick);
            let held = player.inventory.inner.values().map(|stack| stack.item_id);
            assert_eq!(held.collect::<Vec<_>>(), [roast], "{raw:?}");
        }
    }

    #[test]
    fn a_carrot_or_a_potato_is_planted_on_tilled_soil_and_eaten_elsewhere() {
        for (item, sprout) in [
            (ItemId::Carrot, BlockId::CarrotSprout),
            (ItemId::Potato, BlockId::PotatoSprout),
        ] {
            // Planted in the air above the soil, whichever face is aimed at
            let (mut player, mut world) = facing_a(BlockId::TilledSoil, Some(item));
            player.food = 10;
            click(&mut player, &mut world, NetworkAction::RightClick);
            assert_eq!(id(&world, AT + IVec3::Y), Some(sprout), "{item:?}");
            assert!(player.inventory.inner.is_empty());
            assert_eq!(player.food, 10);

            // Eaten when the soil is covered, or not tilled
            for (block, above) in [
                (BlockId::TilledSoil, Some(BlockId::Stone)),
                (BlockId::Grass, None),
            ] {
                let (mut player, mut world) = facing_a(block, Some(item));
                if let Some(above) = above {
                    world.set_block(AT + IVec3::Y, BlockData::new(above, BlockDirection::Front));
                }
                player.food = 10;
                click(&mut player, &mut world, NetworkAction::RightClick);
                assert_eq!(id(&world, AT + IVec3::Y), above, "{item:?} on {block:?}");
                assert!(player.inventory.inner.is_empty());
                assert!(player.food > 10, "{item:?} on {block:?}");
            }
        }
    }

    #[test]
    fn a_soup_leaves_its_bowl_and_the_chowder_heals_even_a_full_player() {
        use crate::players::effects::EffectKind;
        let (mut player, mut world) = facing_a(BlockId::Stone, Some(ItemId::MushroomSoup));
        player.food = 5;
        click(&mut player, &mut world, NetworkAction::RightClick);
        assert_eq!(player.food, 12);
        let held = player.inventory.inner.values().map(|stack| stack.item_id);
        assert_eq!(held.collect::<Vec<_>>(), [ItemId::Bowl]);

        let (mut player, mut world) = facing_a(BlockId::Stone, Some(ItemId::FishChowder));
        player.food = MAX_FOOD;
        click(&mut player, &mut world, NetworkAction::RightClick);
        assert_eq!(player.effects.level(EffectKind::Regeneration), 2);
        let held = player.inventory.inner.values().map(|stack| stack.item_id);
        assert_eq!(held.collect::<Vec<_>>(), [ItemId::Bowl]);

        // A food that does not heal is refused to a full player
        let (mut player, mut world) = facing_a(BlockId::Stone, Some(ItemId::Stew));
        player.food = MAX_FOOD;
        click(&mut player, &mut world, NetworkAction::RightClick);
        let held = player.inventory.inner.values().map(|stack| stack.item_id);
        assert_eq!(held.collect::<Vec<_>>(), [ItemId::Stew]);
    }

    #[test]
    fn compost_grows_a_crop_by_a_stage_and_ripens_a_bush() {
        for (block, after) in [
            (BlockId::YoungRye, BlockId::RipeRye),
            (BlockId::CarrotSprout, BlockId::YoungCarrots),
            (BlockId::BerryBush, BlockId::RipeBerryBush),
        ] {
            let (mut player, mut world) = facing_a(block, Some(ItemId::Compost));
            click(&mut player, &mut world, NetworkAction::RightClick);
            assert_eq!(id(&world, AT), Some(after), "{block:?}");
            assert!(player.inventory.inner.is_empty(), "{block:?}");
        }
    }

    #[test]
    fn both_halves_of_a_door_turn_and_break_together() {
        let (mut player, mut world) = facing_a(BlockId::Door, None);
        let top = AT + IVec3::Y;
        world.set_block(top, BlockData::new(BlockId::DoorTop, BlockDirection::Back));
        click(&mut player, &mut world, NetworkAction::RightClick);
        assert_eq!(id(&world, AT), Some(BlockId::OpenDoor));
        assert_eq!(id(&world, top), Some(BlockId::OpenDoorTop));
        // An open door lies beside the aim: shut again, it is broken
        world.set_block(AT, BlockData::new(BlockId::Door, BlockDirection::Back));
        for _ in 0..40 {
            click(&mut player, &mut world, NetworkAction::LeftClick);
        }
        assert_eq!((id(&world, AT), id(&world, top)), (None, None));
    }
}
