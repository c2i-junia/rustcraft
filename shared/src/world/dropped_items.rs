//! The items lying in the world, such as the ones a broken block drops. The server simulates
//! them and tells the clients where they are.

use bevy::math::{bounding::Aabb3d, Vec3};
use serde::{Deserialize, Serialize};

use crate::players::{
    constants::{GRAVITY, TERMINAL_VELOCITY},
    movement::is_terrain_loaded,
};
use crate::world::{ChunkMap, ItemStack};

pub type DroppedItemId = u64;

/// Edge of the collision box of a dropped item, and of the cube drawn for a block item.
pub const DROPPED_ITEM_SIZE: f32 = 0.25;
/// Ticks before an item can be picked up, so that it pops out of its block first.
pub const PICKUP_DELAY_TICKS: u32 = 10;
/// Ticks before an item a player threw can be picked up, 2 s as in Minecraft: long enough to
/// throw it to someone, or away.
pub const THROWN_PICKUP_DELAY_TICKS: u32 = 40;
/// Ticks after which an item nobody picked up vanishes: 5 minutes.
pub const LIFETIME_TICKS: u32 = 6000;
/// Below this height an item has fallen out of the world, and vanishes.
pub const VOID_Y: f32 = -64.0;
/// Two items of the same kind closer than this merge into one stack.
pub const MERGE_RADIUS: f32 = 0.5;
/// Share of its velocity an item keeps per tick in the air, then on the ground.
const AIR_DRAG: f32 = 0.98;
const GROUND_FRICTION: f32 = 0.59;
/// Ticks per second the drags above are given for.
const DRAG_TICK_RATE: f32 = 20.0;
/// Steps to find, by halving, how far an item can go along an axis before it meets a block.
const CONTACT_STEPS: u32 = 8;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DroppedItem {
    pub stack: ItemStack,
    /// Centre of the collision box.
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    /// Ticks since the item appeared: it can be picked up from `pickup_delay_ticks` on, and
    /// vanishes at `LIFETIME_TICKS`.
    pub age_ticks: u32,
    /// The age from which the item can be picked up: `PICKUP_DELAY_TICKS`, or
    /// `THROWN_PICKUP_DELAY_TICKS` for an item a player threw. Not saved, so that the world
    /// files do not change: a loaded item takes the default, and one thrown less than 2 s
    /// before the save can be picked up sooner than it would have been.
    #[serde(skip, default = "default_pickup_delay")]
    pub pickup_delay_ticks: u32,
    /// How many portals the item went through, wrapping: a client that sees it change puts the
    /// item on the other side at once, rather than easing it across the gap.
    #[serde(default)]
    pub passes: u32,
}

fn default_pickup_delay() -> u32 {
    PICKUP_DELAY_TICKS
}

impl DroppedItem {
    pub fn new(stack: ItemStack, position: Vec3, velocity: Vec3) -> Self {
        Self {
            stack,
            position,
            velocity,
            on_ground: false,
            age_ticks: 0,
            pickup_delay_ticks: PICKUP_DELAY_TICKS,
            passes: 0,
        }
    }

    pub fn can_be_picked_up(&self) -> bool {
        self.age_ticks >= self.pickup_delay_ticks
    }

    /// The ticks left before the item can be picked up.
    pub fn ticks_before_pickup(&self) -> u32 {
        self.pickup_delay_ticks.saturating_sub(self.age_ticks)
    }

    /// Whether `other` can be added to this item's stack: same item, and room for all of it.
    pub fn can_merge(&self, other: &DroppedItem) -> bool {
        self.stack.item_id == other.stack.item_id
            && self.stack.item_type == other.stack.item_type
            && self.stack.nb + other.stack.nb <= self.stack.item_id.get_max_stack()
    }
}

fn hitbox(position: Vec3) -> Aabb3d {
    Aabb3d::new(position, Vec3::splat(DROPPED_ITEM_SIZE / 2.0))
}

/// One fixed step of `delta` seconds: gravity, then the motion along y, x and z one after the
/// other, each stopped against the first block met, then the drag. An item whose terrain is
/// not loaded stays where it is, and one stuck inside a block rises out of it.
///
/// Portals let an item through as they let a player (`Portals::passable_cells`, of its half
/// width): a move into one is made whole and ends the motion of the step, and an item whose
/// centre crossed comes out of the twin, its velocity turned with it (`Portals::pass`).
pub fn step_dropped_item(item: &mut DroppedItem, chunks: &ChunkMap, delta: f32) {
    if !is_terrain_loaded(item.position, chunks) {
        return;
    }
    let portals = chunks.portals();
    let margin = DROPPED_ITEM_SIZE / 2.0;
    let passable = portals.passable_cells(item.position, margin);
    let collides = |position: Vec3| {
        chunks.check_collision_box_except(&hitbox(position), |cell| passable.contains(&cell))
    };
    if collides(item.position) {
        // A block was placed onto the item: it goes up to the top of the cell its bottom is in
        let bottom = item.position.y - DROPPED_ITEM_SIZE / 2.0;
        item.position.y = bottom.floor() + 1.0 + DROPPED_ITEM_SIZE / 2.0;
        item.velocity = Vec3::ZERO;
        return;
    }

    // It falls no faster than a player or a mob, which its drag alone nearly keeps it to
    if !item.on_ground {
        item.velocity.y = (item.velocity.y + GRAVITY * delta).max(-TERMINAL_VELOCITY);
    }
    let start = item.position;
    let falling = item.velocity.y <= 0.0;
    for axis in [1, 0, 2] {
        let wished = item.velocity[axis] * delta;
        let mut to = item.position;
        to[axis] += wished;
        if portals.goes_into(item.position, to, margin) {
            item.position = to;
            break;
        }
        let moved = move_along(item, collides, axis, wished);
        if moved != wished {
            item.velocity[axis] = 0.0;
        }
        if axis == 1 {
            item.on_ground = falling && moved != wished;
        }
    }
    if let Some(passage) = portals.pass(start, item.position, Vec3::splat(margin)) {
        item.position = passage.position;
        item.velocity = passage.through.transform_vector3(item.velocity);
        item.on_ground = false;
        item.passes = item.passes.wrapping_add(1);
        return;
    }
    // Keep resting on the ground: a still item checks it is still there
    if item.velocity.y == 0.0 && !item.on_ground {
        item.on_ground = collides(item.position - Vec3::Y * 0.01);
    }

    let ticks = delta * DRAG_TICK_RATE;
    let horizontal = if item.on_ground {
        AIR_DRAG * GROUND_FRICTION
    } else {
        AIR_DRAG
    };
    item.velocity.x *= horizontal.powf(ticks);
    item.velocity.z *= horizontal.powf(ticks);
    item.velocity.y *= AIR_DRAG.powf(ticks);
}

/// Moves `item` by `distance` along `axis`, or as far as it goes before it meets a block, a
/// position where `collides`. Returns the distance moved.
fn move_along(
    item: &mut DroppedItem,
    collides: impl Fn(Vec3) -> bool,
    axis: usize,
    distance: f32,
) -> f32 {
    let at = |d: f32| {
        let mut position = item.position;
        position[axis] += d;
        position
    };
    if distance == 0.0 {
        return 0.0;
    }
    if !collides(at(distance)) {
        item.position = at(distance);
        return distance;
    }
    // Halve the gap between the last free distance and the first blocked one
    let (mut free, mut blocked) = (0.0, distance);
    for _ in 0..CONTACT_STEPS {
        let middle = (free + blocked) / 2.0;
        if collides(at(middle)) {
            blocked = middle;
        } else {
            free = middle;
        }
    }
    item.position = at(free);
    free
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockId, FaceDirection, ItemId, Portal, PortalColor};
    use bevy::math::IVec3;

    const FLOOR_TOP: f32 = 64.0;

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    /// A stone floor whose top is at `FLOOR_TOP`.
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

    fn run(item: &mut DroppedItem, world: &ChunkMap, delta: f32, seconds: f32) {
        for _ in 0..(seconds / delta) as u32 {
            step_dropped_item(item, world, delta);
            assert!(
                !world.check_collision_box(&hitbox(item.position)),
                "inside a block at {}",
                item.position
            );
        }
    }

    #[test]
    fn a_dropped_item_falls_and_rests_on_the_floor_at_any_step() {
        let world = floor();
        for delta in [0.01, 0.05, 0.1] {
            let start = Vec3::new(0.5, FLOOR_TOP + 3.0, 0.5);
            let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::ZERO);
            run(&mut item, &world, delta, 3.0);
            let bottom = item.position.y - DROPPED_ITEM_SIZE / 2.0;
            assert!(item.on_ground, "{delta}");
            assert!(
                (bottom - FLOOR_TOP).abs() < 0.01,
                "{delta}: bottom at {bottom}"
            );
            assert_eq!(item.velocity.y, 0.0);
        }
    }

    #[test]
    fn a_thrown_item_slows_down_on_the_ground_and_stops() {
        let world = floor();
        let start = Vec3::new(0.5, FLOOR_TOP + DROPPED_ITEM_SIZE / 2.0, 0.5);
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::new(2.0, 0.0, 0.0));
        run(&mut item, &world, 0.05, 5.0);
        assert!(item.velocity.x.abs() < 0.01);
        assert!(item.position.x > start.x && item.position.x < start.x + 2.0);
    }

    #[test]
    fn an_item_thrown_at_a_wall_stops_against_it_and_still_falls() {
        let mut world = floor();
        world.fill_box(IVec3::new(2, 64, -1), IVec3::new(2, 66, 1), BlockId::Stone);
        let start = Vec3::new(0.5, FLOOR_TOP + 1.5, 0.5);
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::new(10.0, 0.0, 0.0));
        run(&mut item, &world, 0.05, 2.0);
        assert!((item.position.x + DROPPED_ITEM_SIZE / 2.0 - 2.0).abs() < 0.01);
        assert!(item.on_ground);
    }

    #[test]
    fn an_item_whose_terrain_is_not_loaded_does_not_move() {
        let world = ChunkMap::default();
        let start = Vec3::new(0.5, 100.0, 0.5);
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::ZERO);
        step_dropped_item(&mut item, &world, 0.05);
        assert_eq!(item.position, start);
    }

    #[test]
    fn an_item_inside_a_block_rises_out_of_it() {
        let mut world = floor();
        world.fill_box(IVec3::new(0, 64, 0), IVec3::new(0, 64, 0), BlockId::Dirt);
        let start = Vec3::new(0.5, 64.2, 0.5);
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::ZERO);
        step_dropped_item(&mut item, &world, 0.05);
        assert!(!world.check_collision_box(&hitbox(item.position)));
        assert!((item.position.y - DROPPED_ITEM_SIZE / 2.0 - 65.0).abs() < 1e-4);
    }

    /// The floor of `floor`, three blocks thick, with an open pair of portals: the blue one on
    /// the floor, x in 0..2 and z in -2..1, its up north; the orange one on a wall facing -X at
    /// x = 10, z in 0..2, its bottom on the floor.
    fn floor_portal_world() -> ChunkMap {
        let mut world = floor();
        world.fill_box(
            IVec3::new(-16, 61, -16),
            IVec3::new(15, 62, 15),
            BlockId::Stone,
        );
        world.fill_box(
            IVec3::new(10, 64, -6),
            IVec3::new(10, 68, 6),
            BlockId::Stone,
        );
        let portals = world.portals_mut();
        portals.insert(Portal {
            owner: 7,
            color: PortalColor::Blue,
            anchor: IVec3::new(0, 63, 0),
            face: FaceDirection::PlusY,
            up: FaceDirection::MinusZ,
        });
        portals.insert(Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(10, 64, 0),
            face: FaceDirection::MinusX,
            up: FaceDirection::PlusY,
        });
        world
    }

    /// Steps `item` until it went through a portal, and returns its velocity before that step.
    fn step_through(item: &mut DroppedItem, world: &ChunkMap, delta: f32) -> Vec3 {
        for _ in 0..100 {
            let (before, passes) = (item.velocity, item.passes);
            step_dropped_item(item, world, delta);
            if item.passes != passes {
                return before;
            }
        }
        panic!("never went through, at {}", item.position);
    }

    #[test]
    fn a_dropped_item_falls_through_a_floor_portal_and_out_of_a_wall_at_the_same_speed() {
        let world = floor_portal_world();
        let start = Vec3::new(1.0, FLOOR_TOP + 3.0, -0.5);
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::ZERO);
        let before = step_through(&mut item, &world, 0.05);
        assert_eq!(item.passes, 1);
        // Out of the wall, in front of it, flying west as fast as it fell, within a step's
        // gravity and drag
        assert!(
            item.position.x < 10.0 && item.position.x > 9.0,
            "{}",
            item.position
        );
        assert!((item.position.z - 1.0).abs() < 0.01, "{}", item.position);
        assert!(!item.on_ground);
        let (fall, flight) = (-before.y, -item.velocity.x);
        assert!(fall > 8.0, "fell at {fall}");
        assert!(
            (flight - fall).abs() < 2.0,
            "flew at {flight}, fell at {fall}"
        );
        // Then it lands on the floor west of the wall, and stays there
        for _ in 0..200 {
            step_dropped_item(&mut item, &world, 0.05);
        }
        assert!(item.on_ground);
        assert_eq!(item.passes, 1);
        assert!(item.position.x < 9.0, "{}", item.position);
        let bottom = item.position.y - DROPPED_ITEM_SIZE / 2.0;
        assert!((bottom - FLOOR_TOP).abs() < 0.01, "{}", item.position);
    }

    #[test]
    fn an_item_falling_between_a_floor_and_a_ceiling_portal_keeps_to_the_terminal_speed() {
        // The orange portal on a stone ceiling over the blue one, 4 blocks above the floor
        let mut world = floor_portal_world();
        world.fill_box(IVec3::new(-3, 68, -5), IVec3::new(4, 68, 3), BlockId::Stone);
        world.portals_mut().insert(Portal {
            owner: 7,
            color: PortalColor::Orange,
            anchor: IVec3::new(1, 68, 0),
            face: FaceDirection::MinusY,
            up: FaceDirection::MinusZ,
        });
        // In short steps, where the drag alone would let it fall a little faster
        let mut item = DroppedItem::new(
            stack(ItemId::Dirt, 1),
            Vec3::new(1.0, 66.0, -0.5),
            Vec3::ZERO,
        );
        for _ in 0..3000 {
            step_dropped_item(&mut item, &world, 0.01);
            assert!(
                item.velocity.length() <= TERMINAL_VELOCITY + 1e-3,
                "{}",
                item.velocity
            );
        }
        // Falling at the terminal speed, less the drag of the step after it
        assert!(item.passes > 100, "{}", item.passes);
        assert!(
            item.velocity.y < 1.0 - TERMINAL_VELOCITY,
            "{}",
            item.velocity
        );
    }

    #[test]
    fn an_item_lying_on_a_floor_portal_falls_into_it_once_it_opens() {
        let mut world = floor_portal_world();
        let orange = world.portals_mut().remove(7, PortalColor::Orange).unwrap();
        let start = Vec3::new(1.0, FLOOR_TOP + DROPPED_ITEM_SIZE / 2.0, -1.0);
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::ZERO);
        run(&mut item, &world, 0.05, 1.0);
        assert!(item.on_ground);
        assert_eq!(item.position, start);

        world.portals_mut().insert(orange);
        step_through(&mut item, &world, 0.05);
        assert!(item.position.x < 10.0, "{}", item.position);
    }

    #[test]
    fn an_item_lying_beside_the_opening_of_a_floor_portal_stays_there() {
        let world = floor_portal_world();
        // Its centre over the floor's edge of the opening, its box half over the stone beside
        let start = Vec3::new(-0.05, FLOOR_TOP + DROPPED_ITEM_SIZE / 2.0, -1.0);
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), start, Vec3::ZERO);
        run(&mut item, &world, 0.05, 1.0);
        assert_eq!(item.position, start);
        assert_eq!(item.passes, 0);
    }

    #[test]
    fn an_item_can_be_picked_up_once_its_own_delay_is_over() {
        let mut item = DroppedItem::new(stack(ItemId::Dirt, 1), Vec3::ZERO, Vec3::ZERO);
        assert_eq!(item.pickup_delay_ticks, PICKUP_DELAY_TICKS);
        item.age_ticks = PICKUP_DELAY_TICKS;
        assert!(item.can_be_picked_up());

        item.pickup_delay_ticks = THROWN_PICKUP_DELAY_TICKS;
        assert!(!item.can_be_picked_up());
        item.age_ticks = THROWN_PICKUP_DELAY_TICKS;
        assert!(item.can_be_picked_up());
    }

    #[test]
    fn a_saved_thrown_item_comes_back_with_the_default_delay() {
        let thrown = DroppedItem {
            pickup_delay_ticks: THROWN_PICKUP_DELAY_TICKS,
            ..DroppedItem::new(stack(ItemId::Dirt, 1), Vec3::ZERO, Vec3::ZERO)
        };
        let saved = ron::to_string(&thrown).unwrap();
        let loaded: DroppedItem = ron::from_str(&saved).unwrap();
        assert_eq!(loaded.pickup_delay_ticks, PICKUP_DELAY_TICKS);
        assert_eq!(loaded.stack, thrown.stack);
    }

    #[test]
    fn only_items_of_the_same_kind_that_fit_in_one_stack_merge() {
        let item = |id, nb| DroppedItem::new(stack(id, nb), Vec3::ZERO, Vec3::ZERO);
        assert!(item(ItemId::Dirt, 10).can_merge(&item(ItemId::Dirt, 54)));
        assert!(!item(ItemId::Dirt, 10).can_merge(&item(ItemId::Dirt, 55)));
        assert!(!item(ItemId::Dirt, 1).can_merge(&item(ItemId::Stone, 1)));
    }
}
