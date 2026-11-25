//! The villagers: where they go, and the trades the players make with them.
//!
//! By day a villager walks about its home, resting between two walks; at night it goes home and
//! stays there. It follows routes of cells (`find_route`), walks through the doors, closed or
//! not, and stands still while it trades. Nothing hurts it (`combat::interact`) and the
//! hostiles, who hunt the players alone, ignore it.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::ops::Range;

use bevy::prelude::*;
use bevy_renet::RenetServer;
use rand::Rng;
use shared::messages::{FullChatMessage, PlayerId, ServerToClientMessage};
use shared::players::{progress::Achievement, Player};
use shared::utils::unix_time_ms;
use shared::world::hostiles::is_night;
use shared::world::trade::{market_day, offers, Goods, TRADE_DISTANCE};
use shared::world::{
    BlockHitbox, BlockId, BlockTransparency, ChunkMap, MobAction, MobId, MobTarget, ServerMob,
    TradeView,
};
use shared::TICKS_PER_SECOND;

use super::behavior::{jump, may_move_to, rest, walk_speed, wander};
use super::float::is_afloat;
use crate::init::{ServerLobby, ServerTime};
use crate::network::commands::SERVER_AUTHOR;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::ServerWorldMap;
use crate::world::locate::Locator;
use crate::world::simulation::LoadedGenerator;

/// How far from its home a villager walks by day, in blocks along each axis.
const STROLL: i32 = 14;
/// How far above or below its home it walks by day, in blocks.
const STROLL_HEIGHTS: [i32; 7] = [0, -1, 1, -2, 2, -3, 3];
/// The cells looked at to find a route about the village, and home.
const STROLL_CELLS: usize = 1200;
const HOME_CELLS: usize = 6000;
/// The deepest drop of a route, in blocks.
const DROP: i32 = 3;
/// Ticks a villager takes to walk to the next cell of its route before it gives the route up.
const STEP_TICKS: u32 = 3 * TICKS_PER_SECOND as u32;
/// Ticks a villager that found no way home waits before it looks again.
const RETRY_TICKS: u32 = 10 * TICKS_PER_SECOND as u32;
/// Farther from the middle of the next cell than this, the villager left its route.
const OFF_ROUTE: f32 = 3.0;
const ARRIVAL: f32 = 0.08;

/// Whether a villager walks through `block`.
pub fn is_door(block: BlockId) -> bool {
    matches!(
        block,
        BlockId::Door | BlockId::DoorTop | BlockId::OpenDoor | BlockId::OpenDoorTop
    )
}

/// Tells the villagers whether to go home.
pub fn villager_day_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    let night = is_night(time.0);
    for mob in world_map.mobs.values_mut() {
        if let Some(villager) = &mut mob.villager {
            villager.homeward = night;
        }
    }
}

/// The cell of the feet of `mob`: the one above, for feet sunk into a slab.
fn feet_cell(mob: &ServerMob, chunks: &ChunkMap) -> IVec3 {
    let feet = mob.position - Vec3::Y * (mob.height / 2.0 - 0.01);
    let cell = feet.floor().as_ivec3();
    if is_free(chunks, cell) {
        cell
    } else {
        cell + IVec3::Y
    }
}

/// What a villager does in a step.
pub(super) fn act(
    mob: &mut ServerMob,
    chunks: &ChunkMap,
    players: &HashMap<PlayerId, Player>,
    delta: f32,
    rng: &mut impl Rng,
) {
    // Off its route, it swims ashore like the other mobs
    if is_afloat(mob, chunks) && mob.villager.as_ref().is_some_and(|v| !v.trading) {
        if let Some(villager) = &mut mob.villager {
            villager.route.clear();
        }
        return wander(mob, chunks, players, delta, rng);
    }
    let here = feet_cell(mob, chunks);
    let Some(villager) = &mut mob.villager else {
        return;
    };
    let home = *villager.home.get_or_insert(here);
    if villager.trading {
        villager.route.clear();
        stand(mob);
        return;
    }
    // Where it goes: the last cell of the route comes first
    if villager.homeward && villager.route.first().is_some_and(|&end| end != home) {
        villager.route.clear();
    }
    if !villager.route.is_empty() {
        return follow_route(mob, chunks, delta, rng);
    }
    if mob.action != MobAction::Idle {
        stand(mob);
    }
    if villager_is_home(here, home) && mob.villager.as_ref().is_some_and(|v| v.homeward) {
        mob.action_ticks = mob.action_ticks.max(1);
        return;
    }
    mob.action_ticks = mob.action_ticks.saturating_sub(1);
    if mob.action_ticks > 0 || !mob.on_ground {
        return;
    }
    let homeward = mob.villager.as_ref().is_some_and(|v| v.homeward);
    let route = if homeward {
        find_route(chunks, here, home, HOME_CELLS)
    } else {
        stroll(chunks, home, rng).and_then(|to| find_route(chunks, here, to, STROLL_CELLS))
    };
    match (route, &mut mob.villager) {
        (Some(route), Some(villager)) if !route.is_empty() => {
            villager.route = route;
            mob.action = MobAction::Walk;
            mob.action_ticks = STEP_TICKS;
        }
        _ if homeward => mob.action_ticks = RETRY_TICKS,
        _ => rest(mob, rng),
    }
}

fn villager_is_home(here: IVec3, home: IVec3) -> bool {
    here.xz() == home.xz() && (here.y - home.y).abs() <= 1
}

/// Stops where it is.
fn stand(mob: &mut ServerMob) {
    mob.action = MobAction::Idle;
    mob.target = MobTarget::None;
    mob.velocity.x = 0.0;
    mob.velocity.z = 0.0;
}

/// A place to stand at random about `home`.
fn stroll(chunks: &ChunkMap, home: IVec3, rng: &mut impl Rng) -> Option<IVec3> {
    let range: Range<i32> = -STROLL..STROLL + 1;
    let column = IVec3::new(rng.gen_range(range.clone()), 0, rng.gen_range(range));
    STROLL_HEIGHTS
        .into_iter()
        .map(|y| home + column + IVec3::Y * y)
        .find(|&cell| can_stand(chunks, cell))
}

/// One step towards the next cell of the route: it climbs a block in its way, slides along one
/// beside its way, and gives the route up when it left it or does not get on.
fn follow_route(mob: &mut ServerMob, chunks: &ChunkMap, delta: f32, rng: &mut impl Rng) {
    let Some(villager) = &mut mob.villager else {
        return;
    };
    let Some(&next) = villager.route.last() else {
        return;
    };
    let middle = next.as_vec3() + Vec3::new(0.5, mob.height / 2.0, 0.5);
    let towards = (middle - mob.position).with_y(0.0);
    let left = towards.length();
    mob.action_ticks = mob.action_ticks.saturating_sub(1);
    if mob.action_ticks == 0 || left > OFF_ROUTE {
        villager.route.clear();
        return rest(mob, rng);
    }
    if left < ARRIVAL && mob.on_ground {
        villager.route.pop();
        mob.action_ticks = STEP_TICKS;
        if villager.route.is_empty() {
            rest(mob, rng);
        }
        return;
    }
    let speed = walk_speed(mob.kind);
    let step = towards.normalize_or_zero() * (speed * delta).min(left);
    let climbs = middle.y - mob.position.y > 0.6;
    mob.action = MobAction::Walk;
    let sides = [step * Vec3::X, step * Vec3::Z];
    if may_move_to(mob, chunks, mob.position + step) {
        mob.position += step;
        let velocity = step / delta;
        (mob.velocity.x, mob.velocity.z) = (velocity.x, velocity.z);
    } else if climbs {
        if mob.on_ground {
            jump(mob);
        }
    } else if let Some(side) = sides.into_iter().find(|side| {
        side.length() > speed * delta * 0.02 && may_move_to(mob, chunks, mob.position + *side)
    }) {
        mob.position += side;
    } else if mob.on_ground {
        jump(mob);
    }
    if left > ARRIVAL {
        mob.rotation = Quat::from_rotation_y(towards.x.atan2(towards.z));
    }
}

/// Whether a villager goes through the block of `cell`.
fn is_free(chunks: &ChunkMap, cell: IVec3) -> bool {
    chunks.get_block(cell).is_none_or(|block| {
        let passable = matches!(block.id.get_hitbox(), BlockHitbox::None)
            && block.id.get_visibility() != BlockTransparency::Liquid
            && block.id != BlockId::Fire;
        passable || is_door(block.id)
    })
}

/// Whether a villager stands with its feet in `cell`: room for it, on a block that is neither
/// tilled nor a hazard.
fn can_stand(chunks: &ChunkMap, cell: IVec3) -> bool {
    is_free(chunks, cell)
        && is_free(chunks, cell + IVec3::Y)
        && chunks.get_block(cell - IVec3::Y).is_some_and(|ground| {
            !matches!(ground.id.get_hitbox(), BlockHitbox::None)
                && !is_door(ground.id)
                && !matches!(
                    ground.id,
                    BlockId::Fence | BlockId::Cactus | BlockId::Campfire | BlockId::TilledSoil
                )
        })
}

/// The cells next to `cell` a villager walks to: on the same level, a block higher, or down to
/// `DROP` lower.
fn next_cells(chunks: &ChunkMap, cell: IVec3) -> impl Iterator<Item = IVec3> + '_ {
    [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
        .into_iter()
        .filter_map(move |side| {
            let level = cell + side;
            let above = level + IVec3::Y;
            if !is_free(chunks, above) {
                return None;
            }
            if !is_free(chunks, level) {
                let climbs = is_free(chunks, cell + IVec3::Y * 2) && can_stand(chunks, above);
                return climbs.then_some(above);
            }
            (0..=DROP)
                .map(|down| level - IVec3::Y * down)
                .take_while(|&lower| is_free(chunks, lower))
                .find(|&lower| can_stand(chunks, lower))
        })
}

/// The shortest way from `from` to `to`, the next cell last and `from` left out, if one is found
/// after looking at `most` cells.
fn find_route(chunks: &ChunkMap, from: IVec3, to: IVec3, most: usize) -> Option<Vec<IVec3>> {
    let left = |cell: IVec3| (to - cell).abs().element_sum();
    let mut came: HashMap<IVec3, (i32, IVec3)> = HashMap::from([(from, (0, from))]);
    let mut open = BinaryHeap::from([Reverse((left(from), 0, from.to_array()))]);
    let mut seen = HashSet::new();
    while let Some(Reverse((_, walked, cell))) = open.pop() {
        let cell = IVec3::from_array(cell);
        if cell == to {
            let mut route = Vec::new();
            let mut at = to;
            while at != from {
                route.push(at);
                at = came[&at].1;
            }
            return Some(route);
        }
        if !seen.insert(cell) {
            continue;
        }
        if seen.len() > most {
            return None;
        }
        for next in next_cells(chunks, cell) {
            let walked = walked + 1;
            if came.get(&next).is_none_or(|&(best, _)| walked < best) {
                came.insert(next, (walked, cell));
                open.push(Reverse((walked + left(next), walked, next.to_array())));
            }
        }
    }
    None
}

/// The trades of the players: a villager may trade with several.
#[derive(Default)]
pub struct Trades {
    /// The villager each player trades with.
    pub open: HashMap<PlayerId, MobId>,
    /// What each was last told of its trade.
    told: HashMap<PlayerId, TradeView>,
    /// The offers taken since the last tick.
    asked: Vec<(PlayerId, MobId, u8)>,
    /// The players holding the right button.
    holding: HashSet<PlayerId>,
}

impl Trades {
    pub fn ask(&mut self, player: PlayerId, mob: MobId, offer: u8) {
        self.asked.push((player, mob, offer));
    }

    /// Whether `player`, whose right button is `down` or not, has just pressed it.
    pub fn press(&mut self, player: PlayerId, down: bool) -> bool {
        if down {
            self.holding.insert(player)
        } else {
            self.holding.remove(&player);
            false
        }
    }

    /// The player closed its screen.
    pub fn end(&mut self, player: PlayerId) {
        self.open.remove(&player);
        self.told.remove(&player);
    }
}

/// Ends the trades of the players too far, dead or gone, makes the trades asked for, and tells
/// each player what its villager has left when it changed.
pub fn trade_system(
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
    (locator, generator): (Res<Locator>, Option<Res<LoadedGenerator>>),
    lobby: Res<ServerLobby>,
    mut server: ResMut<RenetServer>,
) {
    let mut send = |player: PlayerId, message| {
        if lobby.players.contains_key(&player) {
            server.send_game_message(player, message);
        }
    };
    let day = market_day(time.0);
    let generator = generator.as_ref().map(|generator| &generator.0);
    for (player, goods) in make_trades(&mut world_map, day, |goods| {
        goods != Goods::Directions || generator.is_some_and(|generator| generator.has_dungeons())
    }) {
        let from = world_map.players.get(&player).map(|p| p.position);
        match (goods, generator, from) {
            (Some(Goods::Directions), Some(generator), Some(from)) => {
                locator.start_directions(player, generator.clone(), from);
            }
            (None, ..) => {
                let notice = FullChatMessage {
                    author: SERVER_AUTHOR.to_string(),
                    content: "No dungeon is known in this world".to_string(),
                    timestamp: unix_time_ms(),
                };
                send(player, ServerToClientMessage::ChatMessage(notice));
            }
            _ => {}
        }
    }
    for (player, update) in tell_trades(&mut world_map, day) {
        send(player, ServerToClientMessage::TradeUpdate(update));
    }
}

/// Whether the player `player` may still trade with the villager `mob`.
fn may_trade(world_map: &ServerWorldMap, player: PlayerId, mob: MobId) -> bool {
    let (Some(player), Some(mob)) = (world_map.players.get(&player), world_map.mobs.get(&mob))
    else {
        return false;
    };
    !player.is_dead()
        && mob.villager.is_some()
        && player.position.distance(mob.position) <= TRADE_DISTANCE
}

/// Makes the trades asked for, and returns what was given to whom: none when the villager does
/// not have the goods (`has`), and nothing is paid.
fn make_trades(
    world_map: &mut ServerWorldMap,
    day: u64,
    has: impl Fn(Goods) -> bool,
) -> Vec<(PlayerId, Option<Goods>)> {
    let mut given = Vec::new();
    for (player, mob, index) in std::mem::take(&mut world_map.trades.asked) {
        if world_map.trades.open.get(&player) != Some(&mob) || !may_trade(world_map, player, mob) {
            continue;
        }
        let ServerWorldMap { players, mobs, .. } = &mut *world_map;
        let (Some(customer), Some(mob)) = (players.get_mut(&player), mobs.get_mut(&mob)) else {
            continue;
        };
        let Some(offer) = offers(mob.kind).get(usize::from(index)) else {
            continue;
        };
        if !has(offer.goods) {
            given.push((player, None));
            continue;
        }
        let Some(villager) = &mut mob.villager else {
            continue;
        };
        let inventory = &mut customer.inventory;
        if let Ok(goods) = villager.trade(mob.kind, usize::from(index), day, inventory) {
            customer.earn(Achievement::Trade);
            given.push((player, Some(goods)));
        }
    }
    given
}

/// Ends the trades that may not go on, holds the villagers of the others, facing the player,
/// and returns what to tell whom.
fn tell_trades(world_map: &mut ServerWorldMap, day: u64) -> Vec<(PlayerId, Option<TradeView>)> {
    let mut updates = Vec::new();
    let (mut held, mut ended) = (HashMap::new(), Vec::new());
    for (&player, &id) in &world_map.trades.open {
        if may_trade(world_map, player, id) {
            held.insert(id, player);
        } else {
            ended.push(player);
        }
    }
    for player in ended {
        world_map.trades.end(player);
        updates.push((player, None));
    }
    let ServerWorldMap {
        trades,
        mobs,
        players,
        changed_mobs,
        ..
    } = &mut *world_map;
    for (&player, id) in &trades.open {
        let Some((mob, villager)) = mobs
            .get(id)
            .and_then(|mob| Some((mob, mob.villager.as_ref()?)))
        else {
            continue;
        };
        let told = trades.told.get(&player);
        let left = villager.left(mob.kind, day);
        let opening = told.is_none_or(|told| told.mob != *id);
        if opening || told.is_some_and(|told| told.left != left) {
            let view = TradeView {
                mob: *id,
                kind: mob.kind,
                left,
                opening,
            };
            updates.push((player, Some(view.clone())));
            trades.told.insert(player, view);
        }
    }
    for (id, mob) in mobs.iter_mut() {
        let Some(villager) = &mut mob.villager else {
            continue;
        };
        let customer = held.get(id).and_then(|player| players.get(player));
        villager.trading = customer.is_some();
        if let Some(customer) = customer {
            let towards = customer.position - mob.position;
            let rotation = Quat::from_rotation_y(towards.x.atan2(towards.z));
            if mob.rotation != rotation {
                mob.rotation = rotation;
                changed_mobs.insert(*id);
            }
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::super::behavior::apply_gravity;
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::world::{BlockData, BlockDirection, ItemId, ItemStack, MobKind, Villager};

    fn set(chunks: &mut ChunkMap, at: IVec3, id: BlockId) {
        chunks.set_block(at, BlockData::new(id, BlockDirection::Front));
    }

    /// A floor at y 0 with a house of one room and a closed door, its inside around (0, 1, 0).
    fn house() -> ChunkMap {
        let mut chunks = ChunkMap::with_loaded_region(IVec3::splat(-32), IVec3::splat(31));
        chunks.fill_box(
            IVec3::new(-20, 0, -20),
            IVec3::new(20, 0, 20),
            BlockId::Dirt,
        );
        for x in -2..=2 {
            for z in -2i32..=2 {
                for y in 1..=3 {
                    if x == -2 || x == 2 || z.abs() == 2 {
                        set(&mut chunks, IVec3::new(x, y, z), BlockId::OakPlanks);
                    }
                }
                set(&mut chunks, IVec3::new(x, 4, z), BlockId::OakPlanks);
            }
        }
        set(&mut chunks, IVec3::new(0, 1, -2), BlockId::Door);
        set(&mut chunks, IVec3::new(0, 2, -2), BlockId::DoorTop);
        chunks
    }

    #[test]
    fn a_villager_walks_out_by_day_and_back_home_through_its_door_at_night() {
        let chunks = house();
        let home = IVec3::new(0, 1, 0);
        let mut rng = StdRng::seed_from_u64(3);
        let mut mob = ServerMob {
            villager: Some(Villager::living_at(home)),
            ..ServerMob::new(
                MobKind::Farmer,
                ServerMob::standing_at(MobKind::Farmer, 0.5, 1.0, 0.5),
            )
        };
        let mut live = |mob: &mut ServerMob| {
            apply_gravity(mob, &chunks, 0.05);
            act(mob, &chunks, &HashMap::new(), 0.05, &mut rng);
        };
        let outside = |mob: &ServerMob| mob.position.xz().abs().max_element() > 3.0;
        let mut went_out = false;
        for _ in 0..2400 {
            live(&mut mob);
            went_out |= outside(&mob);
        }
        assert!(went_out);
        mob.villager.as_mut().unwrap().homeward = true;
        for _ in 0..1200 {
            live(&mut mob);
        }
        assert_eq!(feet_cell(&mob, &chunks), home, "{}", mob.position);
        let there = mob.position;
        for _ in 0..200 {
            live(&mut mob);
        }
        assert_eq!(mob.position.xz(), there.xz());
    }

    #[test]
    fn a_trade_is_checked_and_ends_when_the_player_is_far() {
        let mut world = ServerWorldMap::default();
        let at = ServerMob::standing_at(MobKind::Farmer, 0.5, 1.0, 0.5);
        world.mobs.insert(7, ServerMob::new(MobKind::Farmer, at));
        let mut player = Player::new(1, "buyer".into(), at + Vec3::X * 2.0, Transform::default());
        let rye = ItemStack {
            item_id: ItemId::Rye,
            item_type: ItemId::Rye.get_default_type(),
            nb: 24,
        };
        player.inventory.inner.insert(0, rye);
        world.players.insert(1, player);
        // Not trading yet
        world.trades.ask(1, 7, 0);
        assert!(make_trades(&mut world, 0, |_| true).is_empty());
        world.trades.open.insert(1, 7);
        let updates = tell_trades(&mut world, 0);
        assert!(matches!(&updates[..], [(1, Some(view))] if view.opening && view.left[0] == 8));
        assert!(world.mobs[&7].villager.as_ref().unwrap().trading);
        for offer in [0, 0, 0, 200] {
            world.trades.ask(1, 7, offer);
        }
        assert_eq!(make_trades(&mut world, 0, |_| true).len(), 2);
        let updates = tell_trades(&mut world, 0);
        assert!(matches!(&updates[..], [(1, Some(view))] if !view.opening && view.left[0] == 6));
        assert!(tell_trades(&mut world, 0).is_empty());
        world.players.get_mut(&1).unwrap().position += Vec3::X * 5.0;
        world.trades.ask(1, 7, 3);
        assert!(make_trades(&mut world, 0, |_| true).is_empty());
        assert_eq!(tell_trades(&mut world, 0), [(1, None)]);
        assert!(!world.mobs[&7].villager.as_ref().unwrap().trading);
    }
}
