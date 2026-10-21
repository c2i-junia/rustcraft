//! The items lying in the world: dropped by the blocks the players break, moved once per
//! tick, merged when they lie together, and picked up by the players who walk over them.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use rand::Rng;
use shared::{
    messages::{DroppedItemState, DroppedItemsUpdate, ItemRemoval, PlayerId},
    players::{blocks::BrokenBlock, Player},
    world::{
        step_dropped_item, world_position_to_chunk_position, ChunkMap, DroppedItem, DroppedItemId,
        ItemStack, DROPPED_ITEM_SIZE, LIFETIME_TICKS, MERGE_RADIUS, VOID_Y,
    },
    HALF_BLOCK,
};

use super::data::ServerWorldMap;

/// Ticks between two looks for items to merge.
const MERGE_INTERVAL_TICKS: u64 = 10;
/// How much farther than its own box a player reaches for items, on each side.
const PICKUP_REACH: Vec3 = Vec3::new(1.0, 0.5, 1.0);
/// Most horizontal speed of an item popping out of its block, in blocks per second.
const POP_HORIZONTAL_SPEED: f32 = 2.0;
/// Upward speed of an item popping out of its block, in blocks per second.
const POP_UPWARD_SPEED: f32 = 4.0;
/// An item that moved less than this is not sent again.
const MOVE_EPSILON: f32 = 1.0 / 256.0;

/// The dropped items of the world, and what changed since the last broadcast.
#[derive(Default, Debug)]
pub struct DroppedItems {
    items: HashMap<DroppedItemId, DroppedItem>,
    next_id: DroppedItemId,
    /// The fixed steps done, which say when to look for items to merge.
    steps: u64,
    changed: HashSet<DroppedItemId>,
    removed: Vec<(DroppedItemId, ItemRemoval)>,
}

impl DroppedItems {
    /// The items of a saved world, with new ids.
    pub fn from_saved(items: Vec<DroppedItem>) -> Self {
        let mut dropped = Self::default();
        for item in items {
            dropped.spawn(item);
        }
        dropped.changed.clear();
        dropped
    }

    /// The items to save, in the order of their ids.
    pub fn to_saved(&self) -> Vec<DroppedItem> {
        let mut ids: Vec<_> = self.items.keys().copied().collect();
        ids.sort_unstable();
        ids.iter().map(|id| self.items[id].clone()).collect()
    }

    pub fn spawn(&mut self, item: DroppedItem) -> DroppedItemId {
        let id = self.next_id;
        self.next_id += 1;
        self.items.insert(id, item);
        self.changed.insert(id);
        id
    }

    pub fn remove(&mut self, id: DroppedItemId, why: ItemRemoval) {
        if self.items.remove(&id).is_some() {
            self.changed.remove(&id);
            self.removed.push((id, why));
        }
    }

    /// The items that appeared or changed, and the ones removed, since the previous call.
    pub fn take_changes(&mut self) -> (HashSet<DroppedItemId>, Vec<(DroppedItemId, ItemRemoval)>) {
        (
            std::mem::take(&mut self.changed),
            std::mem::take(&mut self.removed),
        )
    }

    /// Drops the items of `broken` if it was harvested, and what it held if it was a chest or a
    /// furnace, at its centre, each thrown in a random direction.
    pub fn drop_block_items(&mut self, broken: &BrokenBlock, rng: &mut impl Rng) {
        let centre = broken.position.as_vec3() + HALF_BLOCK;
        let nb_drops = u32::from(broken.harvested);
        let drops = broken
            .block
            .get_drops(nb_drops)
            .into_iter()
            .map(|(item_id, nb)| ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb,
            });
        let contents = broken
            .contents
            .iter()
            .flat_map(|contents| contents.stacks());
        let furnace = broken.furnace.iter().flat_map(|furnace| furnace.stacks());
        for stack in drops.chain(contents).chain(furnace) {
            let velocity = Vec3::new(
                rng.gen_range(-POP_HORIZONTAL_SPEED..=POP_HORIZONTAL_SPEED),
                POP_UPWARD_SPEED,
                rng.gen_range(-POP_HORIZONTAL_SPEED..=POP_HORIZONTAL_SPEED),
            );
            self.spawn(DroppedItem::new(stack, centre, velocity));
        }
    }

    /// One fixed step of every item: it ages, moves, and vanishes once too old or out of the
    /// world. Every `MERGE_INTERVAL_TICKS` steps, the items lying together merge.
    pub fn step(&mut self, chunks: &ChunkMap, delta: f32) {
        let mut gone = Vec::new();
        for (&id, item) in self.items.iter_mut() {
            item.age_ticks = item.age_ticks.saturating_add(1);
            if item.age_ticks >= LIFETIME_TICKS || item.position.y < VOID_Y {
                gone.push(id);
                continue;
            }
            let before = item.position;
            step_dropped_item(item, chunks, delta);
            if item.position.distance(before) > MOVE_EPSILON {
                self.changed.insert(id);
            }
        }
        for id in gone {
            self.remove(id, ItemRemoval::Gone);
        }

        self.steps += 1;
        if self.steps.is_multiple_of(MERGE_INTERVAL_TICKS) {
            self.merge();
        }
    }

    /// Adds each item resting on the ground to the stack of an older one of the same kind
    /// within `MERGE_RADIUS`, when the two fit in one stack.
    fn merge(&mut self) {
        let mut ids: Vec<_> = self
            .items
            .iter()
            .filter(|(_, item)| item.on_ground)
            .map(|(&id, _)| id)
            .collect();
        // The older item, the one with the lower id, takes the other
        ids.sort_unstable();
        for (index, &into) in ids.iter().enumerate() {
            for &from in &ids[index + 1..] {
                let (Some(target), Some(source)) = (self.items.get(&into), self.items.get(&from))
                else {
                    continue;
                };
                if target.position.distance(source.position) > MERGE_RADIUS
                    || !target.can_merge(source)
                {
                    continue;
                }
                let (nb, age) = (source.stack.nb, source.age_ticks);
                let target = self.items.get_mut(&into).expect("checked above");
                target.stack.nb += nb;
                // The merged stack lasts as long as the younger of the two
                target.age_ticks = target.age_ticks.min(age);
                self.changed.insert(into);
                self.remove(from, ItemRemoval::Merged(into));
            }
        }
    }

    /// Gives each item that can be picked up to a player close enough, in the order of their
    /// ids. A player with no room for the whole stack takes what fits.
    pub fn pick_up(&mut self, players: &mut HashMap<PlayerId, Player>) {
        let mut player_ids: Vec<_> = players.keys().copied().collect();
        player_ids.sort_unstable();
        let mut item_ids: Vec<_> = self.items.keys().copied().collect();
        item_ids.sort_unstable();

        for id in item_ids {
            for player_id in &player_ids {
                let item = &self.items[&id];
                let player = players.get_mut(player_id).expect("listed above");
                if !item.can_be_picked_up() || !within_reach(player, item.position) {
                    continue;
                }
                let left = player.inventory.add_item_to_inventory(item.stack);
                if left == 0 {
                    self.remove(id, ItemRemoval::PickedUp(*player_id));
                    break;
                }
                if left != item.stack.nb {
                    self.items.get_mut(&id).expect("still there").stack.nb = left;
                    self.changed.insert(id);
                }
            }
        }
    }
}

/// Whether the box of an item at `position` touches the box of `player` grown by
/// `PICKUP_REACH`.
fn within_reach(player: &Player, position: Vec3) -> bool {
    let player_half = Vec3::new(player.width, player.height, player.width) / 2.0;
    let reach = player_half + PICKUP_REACH + Vec3::splat(DROPPED_ITEM_SIZE / 2.0);
    let distance = (position - player.position).abs();
    distance.cmple(reach).all()
}

/// What the client of a player that has the chunks `sent_chunks` must be told: the items in
/// those chunks that it lacks or that `changed`, then the items it has that are `removed` or
/// no longer in its chunks. `sent_items`, the items it has, is kept up to date.
pub fn dropped_items_update(
    items: &DroppedItems,
    changed: &HashSet<DroppedItemId>,
    removed: &[(DroppedItemId, ItemRemoval)],
    sent_chunks: &HashSet<IVec3>,
    sent_items: &mut HashSet<DroppedItemId>,
) -> DroppedItemsUpdate {
    let mut update = DroppedItemsUpdate::default();
    for &(id, why) in removed {
        if sent_items.remove(&id) {
            update.removals.push((id, why));
        }
    }

    let mut ids: Vec<_> = items.items.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let item = &items.items[&id];
        let chunk = world_position_to_chunk_position(item.position);
        if sent_chunks.contains(&chunk) {
            if sent_items.insert(id) || changed.contains(&id) {
                update.upserts.push((
                    id,
                    DroppedItemState {
                        stack: item.stack,
                        position: item.position,
                        velocity: item.velocity,
                    },
                ));
            }
        } else if sent_items.remove(&id) {
            update.removals.push((id, ItemRemoval::Gone));
        }
    }
    update
}

/// Moves the dropped items, once per tick.
pub fn dropped_items_physics_system(mut world_map: ResMut<ServerWorldMap>, time: Res<Time<Fixed>>) {
    let world_map = world_map.as_mut();
    world_map
        .dropped_items
        .step(&world_map.chunks, time.delta_secs());
}

/// Gives the items lying near the players to them, after the inputs of the tick moved them.
pub fn pickup_dropped_items_system(mut world_map: ResMut<ServerWorldMap>) {
    let world_map = world_map.as_mut();
    world_map.dropped_items.pick_up(&mut world_map.players);
}

#[cfg(test)]
impl DroppedItems {
    fn get(&self, id: DroppedItemId) -> Option<&DroppedItem> {
        self.items.get(&id)
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use shared::{
        world::{global_block_to_chunk_pos, BlockId, ItemId, PICKUP_DELAY_TICKS},
        MAX_INVENTORY_SLOTS,
    };

    const DELTA: f32 = 0.05;

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    /// An item resting on a floor at y = 64, old enough to be picked up.
    fn resting(item_id: ItemId, nb: u32, position: Vec3) -> DroppedItem {
        DroppedItem {
            on_ground: true,
            age_ticks: PICKUP_DELAY_TICKS,
            ..DroppedItem::new(stack(item_id, nb), position, Vec3::ZERO)
        }
    }

    fn floor() -> ChunkMap {
        let mut world =
            ChunkMap::with_loaded_region(IVec3::new(-16, 48, -16), IVec3::new(15, 79, 15));
        world.fill_box(
            IVec3::new(-16, 63, -16),
            IVec3::new(15, 63, 15),
            BlockId::Stone,
        );
        world
    }

    const ON_FLOOR: Vec3 = Vec3::new(0.5, 64.0 + DROPPED_ITEM_SIZE / 2.0, 0.5);

    fn player_at(id: PlayerId, position: Vec3) -> HashMap<PlayerId, Player> {
        let player = Player::new(id, "p".into(), position, Transform::default());
        HashMap::from([(id, player)])
    }

    #[test]
    fn a_broken_block_drops_its_items_at_its_centre() {
        let mut items = DroppedItems::default();
        let broken = BrokenBlock {
            position: IVec3::new(3, 64, -2),
            block: BlockId::Dirt,
            contents: None,
            furnace: None,
            harvested: true,
        };
        items.drop_block_items(&broken, &mut StdRng::seed_from_u64(1));

        assert_eq!(items.len(), 1);
        let (&id, item) = items.items.iter().next().unwrap();
        assert_eq!(item.stack, stack(ItemId::Dirt, 1));
        assert_eq!(item.position, Vec3::new(3.5, 64.5, -1.5));
        assert!(item.velocity.y > 0.0);
        assert!(!item.can_be_picked_up());
        assert_eq!(items.take_changes().0, HashSet::from([id]));
    }

    #[test]
    fn a_broken_chest_drops_what_it_held_with_itself() {
        let mut items = DroppedItems::default();
        let mut contents = shared::world::Container::default();
        contents.slots[2] = Some(stack(ItemId::Stone, 12));
        contents.slots[20] = Some(stack(ItemId::Dirt, 64));
        let broken = BrokenBlock {
            position: IVec3::new(3, 64, -2),
            block: BlockId::Chest,
            contents: Some(contents),
            furnace: None,
            harvested: true,
        };
        items.drop_block_items(&broken, &mut StdRng::seed_from_u64(1));

        let mut dropped: Vec<ItemStack> = items.items.values().map(|item| item.stack).collect();
        dropped.sort_by_key(|stack| stack.item_id);
        assert_eq!(
            dropped,
            [
                stack(ItemId::Dirt, 64),
                stack(ItemId::Stone, 12),
                stack(ItemId::Chest, 1)
            ]
        );
    }

    #[test]
    fn a_broken_furnace_drops_its_three_stacks_even_without_a_pickaxe() {
        let mut items = DroppedItems::default();
        let furnace = shared::world::Furnace {
            input: Some(stack(ItemId::Sand, 3)),
            fuel: Some(stack(ItemId::Coal, 2)),
            output: Some(stack(ItemId::Glass, 5)),
            ..Default::default()
        };
        let broken = BrokenBlock {
            position: IVec3::new(3, 64, -2),
            block: BlockId::LitFurnace,
            contents: None,
            furnace: Some(furnace),
            harvested: false,
        };
        items.drop_block_items(&broken, &mut StdRng::seed_from_u64(1));

        let mut dropped: Vec<ItemStack> = items.items.values().map(|item| item.stack).collect();
        dropped.sort_by_key(|stack| stack.item_id);
        assert_eq!(
            dropped,
            [
                stack(ItemId::Sand, 3),
                stack(ItemId::Glass, 5),
                stack(ItemId::Coal, 2)
            ]
        );
    }

    #[test]
    fn a_block_broken_without_its_tool_drops_nothing() {
        let mut items = DroppedItems::default();
        let broken = BrokenBlock {
            position: IVec3::new(3, 64, -2),
            block: BlockId::Stone,
            contents: None,
            furnace: None,
            harvested: false,
        };
        items.drop_block_items(&broken, &mut StdRng::seed_from_u64(1));
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn a_worn_tool_dropped_from_a_chest_keeps_its_wear() {
        let mut items = DroppedItems::default();
        let worn = ItemStack {
            item_type: shared::world::ItemType::Tool { wear: 17 },
            ..stack(ItemId::WoodenPickaxe, 1)
        };
        let mut contents = shared::world::Container::default();
        contents.slots[0] = Some(worn);
        let broken = BrokenBlock {
            position: IVec3::new(3, 64, -2),
            block: BlockId::Chest,
            contents: Some(contents),
            furnace: None,
            harvested: true,
        };
        items.drop_block_items(&broken, &mut StdRng::seed_from_u64(1));
        assert!(items.items.values().any(|item| item.stack == worn));
    }

    #[test]
    fn an_item_at_rest_is_not_sent_again() {
        let world = floor();
        let mut items = DroppedItems::default();
        items.spawn(DroppedItem::new(
            stack(ItemId::Dirt, 1),
            ON_FLOOR + Vec3::Y,
            Vec3::ZERO,
        ));
        for _ in 0..40 {
            items.step(&world, DELTA);
        }
        items.take_changes();
        items.step(&world, DELTA);
        assert!(items.take_changes().0.is_empty());
    }

    #[test]
    fn items_vanish_once_too_old() {
        let world = floor();
        let mut items = DroppedItems::default();
        let id = items.spawn(DroppedItem {
            age_ticks: LIFETIME_TICKS - 1,
            ..resting(ItemId::Dirt, 1, ON_FLOOR)
        });
        items.take_changes();
        items.step(&world, DELTA);
        assert!(items.is_empty());
        assert_eq!(items.take_changes().1, vec![(id, ItemRemoval::Gone)]);
    }

    #[test]
    fn two_items_of_the_same_kind_merge_into_the_older_one() {
        let world = floor();
        let mut items = DroppedItems::default();
        let older = items.spawn(resting(ItemId::Dirt, 3, ON_FLOOR));
        let younger = items.spawn(resting(ItemId::Dirt, 2, ON_FLOOR + Vec3::X * 0.3));
        let other = items.spawn(resting(ItemId::Stone, 1, ON_FLOOR - Vec3::X * 0.3));
        items.take_changes();
        for _ in 0..MERGE_INTERVAL_TICKS {
            items.step(&world, DELTA);
        }

        assert_eq!(items.len(), 2);
        assert_eq!(items.get(older).unwrap().stack.nb, 5);
        assert_eq!(items.get(other).unwrap().stack.nb, 1);
        let (changed, removed) = items.take_changes();
        assert!(changed.contains(&older));
        assert_eq!(removed, vec![(younger, ItemRemoval::Merged(older))]);
    }

    #[test]
    fn items_that_would_overflow_a_stack_do_not_merge() {
        let world = floor();
        let mut items = DroppedItems::default();
        items.spawn(resting(ItemId::Dirt, 60, ON_FLOOR));
        items.spawn(resting(ItemId::Dirt, 5, ON_FLOOR + Vec3::X * 0.3));
        for _ in 0..MERGE_INTERVAL_TICKS {
            items.step(&world, DELTA);
        }
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn a_player_picks_up_an_item_within_reach_once_the_delay_is_over() {
        let mut players = player_at(7, ON_FLOOR + Vec3::new(1.2, 0.9, 0.0));
        let mut items = DroppedItems::default();
        let id = items.spawn(DroppedItem {
            age_ticks: PICKUP_DELAY_TICKS - 1,
            ..resting(ItemId::Dirt, 4, ON_FLOOR)
        });
        items.take_changes();

        items.pick_up(&mut players);
        assert_eq!(items.len(), 1, "picked up before the delay");

        items.step(&floor(), DELTA);
        items.pick_up(&mut players);
        assert!(items.is_empty());
        assert_eq!(players[&7].inventory.inner[&0], stack(ItemId::Dirt, 4));
        assert_eq!(items.take_changes().1, vec![(id, ItemRemoval::PickedUp(7))]);
    }

    #[test]
    fn a_player_too_far_picks_nothing_up() {
        let mut players = player_at(7, ON_FLOOR + Vec3::new(3.0, 0.9, 0.0));
        let mut items = DroppedItems::default();
        items.spawn(resting(ItemId::Dirt, 1, ON_FLOOR));
        items.pick_up(&mut players);
        assert_eq!(items.len(), 1);
        assert!(players[&7].inventory.inner.is_empty());
    }

    #[test]
    fn a_full_inventory_takes_what_fits_and_leaves_the_rest_on_the_ground() {
        let mut players = player_at(7, ON_FLOOR + Vec3::Y * 0.9);
        let inventory = &mut players.get_mut(&7).unwrap().inventory;
        for slot in 0..MAX_INVENTORY_SLOTS {
            inventory.inner.insert(slot, stack(ItemId::Stone, 64));
        }
        inventory.inner.insert(0, stack(ItemId::Dirt, 60));
        let mut items = DroppedItems::default();
        let id = items.spawn(resting(ItemId::Dirt, 10, ON_FLOOR));
        items.take_changes();

        items.pick_up(&mut players);

        assert_eq!(players[&7].inventory.inner[&0].nb, 64);
        assert_eq!(items.get(id).unwrap().stack.nb, 6);
        assert_eq!(items.take_changes().0, HashSet::from([id]));
    }

    #[test]
    fn a_client_is_told_of_the_items_in_its_chunks_only_and_of_their_removal() {
        let mut items = DroppedItems::default();
        let near = items.spawn(resting(ItemId::Dirt, 1, ON_FLOOR));
        let far = items.spawn(resting(ItemId::Dirt, 1, ON_FLOOR + Vec3::X * 40.0));
        let sent_chunks = HashSet::from([global_block_to_chunk_pos(&IVec3::new(0, 64, 0))]);
        let mut sent_items = HashSet::new();

        let (changed, removed) = items.take_changes();
        let update =
            dropped_items_update(&items, &changed, &removed, &sent_chunks, &mut sent_items);
        let upserted: Vec<_> = update.upserts.iter().map(|(id, _)| *id).collect();
        assert_eq!(upserted, vec![near]);
        assert_eq!(sent_items, HashSet::from([near]));

        // Nothing changed: nothing is sent
        let (changed, removed) = items.take_changes();
        let update =
            dropped_items_update(&items, &changed, &removed, &sent_chunks, &mut sent_items);
        assert!(update.is_empty());

        // The client loses the chunk of the item
        let update =
            dropped_items_update(&items, &changed, &removed, &HashSet::new(), &mut sent_items);
        assert_eq!(update.removals, vec![(near, ItemRemoval::Gone)]);

        // A removed item the client never had is not mentioned
        items.remove(far, ItemRemoval::Gone);
        let (changed, removed) = items.take_changes();
        let update =
            dropped_items_update(&items, &changed, &removed, &sent_chunks, &mut sent_items);
        assert!(update.removals.is_empty());
    }

    #[test]
    fn saved_items_come_back_with_new_ids_and_unchanged() {
        let mut items = DroppedItems::default();
        items.spawn(resting(ItemId::Dirt, 3, ON_FLOOR));
        items.spawn(resting(ItemId::Stone, 1, ON_FLOOR + Vec3::X));
        let saved = items.to_saved();
        let loaded = DroppedItems::from_saved(saved.clone());
        assert_eq!(loaded.to_saved(), saved);
    }
}
