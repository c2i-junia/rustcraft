use std::ops::Range;

use bevy::{
    math::{IVec3, Vec3},
    prelude::{Component, Resource, Transform},
};
use bevy_platform::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::air::{full_air, MAX_AIR};
use super::constants::DRAG_MS;
use super::effects::StatusEffects;
use super::experience::Experience;
use super::game_mode::{
    DamageSource, DeathCause, GameMode, INVULNERABILITY_TICKS, MAX_HEALTH, THROWN_OFF_TICKS,
};
use super::hunger::{
    full_food, respawn_saturation, DAMAGE_EXHAUSTION, MAX_FOOD, RESPAWN_SATURATION,
};
use super::progress::Progress;
use super::riding::Riding;
use super::skin::PlayerSkin;
use crate::{
    messages::PlayerId,
    world::{
        cartography::Waypoints, depths::DepthTier, ItemId, ItemStack, ItemType, MobKind, Passer,
        PortalRest,
    },
    HOTBAR_SLOTS, MAX_INVENTORY_SLOTS,
};

/// What a player holds in its hand, as the others see it: the item, and whether it is enchanted,
/// which shows on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Held {
    pub item: ItemId,
    pub enchanted: bool,
}

#[derive(Debug, Resource, Clone, Serialize, Deserialize, PartialEq)]
pub struct Inventory {
    pub inner: HashMap<u32, ItemStack>,
    /// The stack held by the cursor while the inventory is open.
    #[serde(default)]
    pub floating: Option<ItemStack>,
    /// The crafting table the player is using, if any.
    #[serde(default)]
    pub open_table: Option<IVec3>,
    /// The chest the player is using, if any. Never at the same time as a table.
    #[serde(default)]
    pub open_chest: Option<IVec3>,
    /// The furnace, lit or out, the player is using, if any. Never at the same time as a table
    /// or a chest.
    #[serde(default)]
    pub open_furnace: Option<IVec3>,
    /// The engraving table the player is using, if any. Never at the same time as another block.
    #[serde(default)]
    pub open_engraver: Option<IVec3>,
    /// The anvil the player is using, if any. Never at the same time as another block.
    #[serde(default)]
    pub open_anvil: Option<IVec3>,
    /// The donkey or the mule whose chest the player has open, if any: its slots are those of
    /// a chest. Never at the same time as a block.
    #[serde(default)]
    pub open_pack: Option<crate::world::MobId>,
    /// Whether the furnace lent to the inventory while an action applies is an alembic, whose
    /// output takes a vial (`inventory::OpenContents::Alembic`). Never sent nor saved.
    #[serde(skip)]
    pub alembic_lent: bool,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            floating: None,
            open_table: None,
            open_chest: None,
            open_furnace: None,
            open_engraver: None,
            open_anvil: None,
            open_pack: None,
            alembic_lent: false,
        }
    }

    /// Closes the table, the chest, the furnace, the engraving table or the anvil the player is
    /// using.
    pub fn close_blocks(&mut self) {
        self.open_anvil = None;
        self.open_table = None;
        self.open_chest = None;
        self.open_furnace = None;
        self.open_engraver = None;
        self.open_pack = None;
    }

    /// Whether the player has the slots of a chest open: a chest, a barrel, or the chest on the
    /// back of a donkey or a mule.
    pub fn has_chest_open(&self) -> bool {
        self.open_chest.is_some() || self.open_pack.is_some()
    }

    /// What a player holds in its hand with the hotbar slot `slot` selected: the item of that
    /// slot, none when it is empty or when `slot` is not one of the hotbar's.
    pub fn in_hand(&self, slot: u32) -> Option<Held> {
        if slot >= HOTBAR_SLOTS {
            return None;
        }
        self.inner.get(&slot).map(|stack| Held {
            item: stack.item_id,
            enchanted: stack.is_enchanted(),
        })
    }

    /// Adds `stack` to the inventory: first on the stacks of the same item, then in empty slots,
    /// in slot order. Returns how many items did not fit, for the caller to drop at the feet of
    /// the player.
    pub fn add_item_to_inventory(&mut self, stack: ItemStack) -> u32 {
        self.add_item_to_range(stack, 0..MAX_INVENTORY_SLOTS)
    }

    /// Adds `stack` to the slots of `range`: first on stacks of the same item, then in empty
    /// slots, in slot order. Returns how many items did not fit.
    pub fn add_item_to_range(&mut self, mut stack: ItemStack, range: Range<u32>) -> u32 {
        let max_stack = stack.item_id.get_max_stack();
        for tops_up in [true, false] {
            for slot in range.clone() {
                if stack.nb == 0 {
                    return 0;
                }
                let matches = match self.inner.get(&slot) {
                    Some(held) => tops_up && held.item_id == stack.item_id,
                    None => !tops_up,
                };
                if !matches {
                    continue;
                }
                let held = self.inner.get(&slot).map_or(0, |held| held.nb);
                let added = stack.nb.min(max_stack.saturating_sub(held));
                if added > 0 {
                    self.inner.insert(
                        slot,
                        ItemStack {
                            nb: held + added,
                            ..stack
                        },
                    );
                    stack.nb -= added;
                }
            }
        }
        stack.nb
    }

    /// Add items to stack at specified position\
    /// Stacks cannot exceed the item's `get_max_stack()` number of items\
    /// Returns number of items really added to the stack
    pub fn add_item_to_stack(
        &mut self,
        stack: u32,
        mut nb: u32,
        id: ItemId,
        item_type: ItemType,
    ) -> u32 {
        let item_option = self.inner.get(&stack);
        let mut new_item = ItemStack {
            item_id: id,
            nb,
            item_type,
        };

        if let Some(item) = item_option {
            if nb + item.nb > item.item_id.get_max_stack() {
                nb = item.item_id.get_max_stack() - item.nb;
            }
            // The stack keeps its own type: a tool its wear
            new_item = ItemStack {
                nb: nb + item.nb,
                ..*item
            };
        }
        self.inner.insert(stack, new_item);
        nb
    }

    /// Adds `wear` to the tool in `slot`. A tool worn down to its durability breaks: it leaves
    /// the slot, and `true` is returned. Anything else in the slot is left alone.
    pub fn wear_tool(&mut self, slot: u32, wear: u32) -> bool {
        let Some(stack) = self.inner.get_mut(&slot) else {
            return false;
        };
        let (Some(durability), Some(worn)) = (stack.durability(), stack.item_type.wear()) else {
            return false;
        };
        let worn = worn.saturating_add(wear);
        if worn < durability {
            stack.item_type = stack.item_type.with_wear(worn);
            return false;
        }
        self.inner.remove(&slot);
        true
    }

    /// Removes items from stack at specified position\
    /// Stacks cannot have < 0 number of items\
    /// Returns number of items really removed from the stack
    pub fn remove_item_from_stack(&mut self, stack: u32, mut nb: u32) -> u32 {
        let item_option = self.inner.get(&stack);

        if let Some(&item) = item_option {
            if nb >= item.nb {
                nb = item.nb;
                self.inner.remove(&stack);
            } else {
                self.inner.insert(
                    stack,
                    ItemStack {
                        item_id: item.item_id,
                        nb: item.nb - nb,
                        item_type: item.item_type,
                    },
                );
            }
            return nb;
        }
        0
    }
}

#[derive(Component, Clone, Serialize, Deserialize, Debug)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub position: Vec3,
    pub camera_transform: Transform,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_flying: bool,
    #[serde(default)]
    pub is_sprinting: bool,
    #[serde(default)]
    pub is_sneaking: bool,
    #[serde(default)]
    pub is_swimming: bool,
    /// Blocks fallen since the player last stood on the ground, swam or flew.
    #[serde(default)]
    pub fall_distance: f32,
    #[serde(default)]
    pub game_mode: GameMode,
    /// From 0, dead, to `MAX_HEALTH`. Only the server changes it.
    #[serde(default = "full_health")]
    pub health: u32,
    /// The hundredths of a point of health the armour let through and no hit has taken yet,
    /// below 100: see `Player::absorb`. Not saved.
    #[serde(skip)]
    pub armour_carry: u32,
    /// What killed the player, while it is dead.
    #[serde(default)]
    pub death_cause: Option<DeathCause>,
    /// From 0, starving, to `MAX_FOOD`. Only the server changes it, but the client predicts
    /// what it eats.
    #[serde(default = "full_food")]
    pub food: u8,
    /// What the efforts take before the food, never above it. The server's only.
    #[serde(default = "respawn_saturation")]
    pub saturation: f32,
    /// The efforts since the last point of saturation or food taken, below `EXHAUSTION_WRAP`.
    /// The server's only.
    #[serde(default)]
    pub exhaustion: f32,
    /// The levels of the player. Only the server changes them. Saves from before them have none.
    #[serde(default)]
    pub experience: Experience,
    /// The depth tiers the player was in, each announced the first time. The server's only.
    #[serde(default)]
    pub depths_reached: Vec<DepthTier>,
    /// What the player achieved. The server's only, which tells the client of it. Saves from
    /// before it have none.
    #[serde(default = "Progress::of_an_old_save")]
    pub progress: Progress,
    /// Where the player died, what it found and the places it named (`cartography`). Saves
    /// from before them have none.
    #[serde(default)]
    pub waypoints: Waypoints,
    pub inventory: Inventory,
    pub height: f32,
    pub width: f32,
    pub last_input_processed: u64,
    /// Time the left button was held that did not add up to a mining hit yet, in ms.
    #[serde(skip)]
    pub mining_carry_ms: u64,
    /// Time the right button must still be held before the next block is placed, in ms.
    #[serde(skip)]
    pub place_wait_ms: u64,
    /// Whether the left and the right button were down in the previous input: the portal gun
    /// shoots once per press.
    #[serde(skip)]
    pub portal_triggers_held: [bool; 2],
    /// The floor portal the player came out of too slowly to leave, and rests on: it does not
    /// take the player back until it has left it (`Portals::still_resting`). Not saved, as the
    /// portals.
    #[serde(skip)]
    pub portal_rest: Option<PortalRest>,
    /// Ticks left during which a hit is ignored, but from the sources that pierce it.
    #[serde(skip)]
    pub invulnerable_ticks: u32,
    /// The kind of the mob that hit the player last, and the ticks left during which a deadly
    /// fall is put on it.
    #[serde(skip)]
    pub last_mob_hit: Option<(MobKind, u32)>,
    /// The ticks of air left, up to `MAX_AIR`, while the eyes are in a liquid. Not saved: a
    /// player who joins breathes.
    #[serde(skip, default = "full_air")]
    pub air: u16,
    /// The ticks spent with no air left since the last hit of drowning.
    #[serde(skip)]
    pub drowning_ticks: u16,
    /// The ticks the player has left to burn, set by a fire it touches. Not saved.
    #[serde(skip)]
    pub burning_ticks: u16,
    /// The ticks to the next hit of a burning player: it is hit when they reach 0.
    #[serde(skip)]
    pub fire_damage_wait: u16,
    /// The ticks to the next hit of lava while the player is in it: it is hit when they reach 0.
    #[serde(skip)]
    pub lava_damage_wait: u16,
    /// The time left, in ms of input, during which a Drowner drags the player down
    /// (`Player::drag`). Part of the `MovementState`. Not saved.
    #[serde(skip)]
    pub dragged_ms: u32,
    /// How long the player has drawn its bow, in ms (`bow::draw_bow`). Part of the
    /// `MovementState`, as it slows the player. Not saved.
    #[serde(skip)]
    pub bow_draw_ms: u32,
    /// Whether the left button was down in the last input: the player swings its arm, at a
    /// block or at the air. Sent to the others with each update, not saved.
    #[serde(skip)]
    pub is_swinging: bool,
    /// The hotbar slot selected in the last input, whose item the player holds in its hand
    /// (`Inventory::in_hand`): the others see it. Not saved.
    #[serde(skip)]
    pub hotbar_slot: u32,
    /// The skin the player asked for when it joined (`AuthRegisterRequest::skin`), which the
    /// others are given with it. Not saved: it is the client's choice, sent at each join.
    #[serde(skip)]
    pub skin: PlayerSkin,
    /// The mount the player rides (`riding`). Part of the `MovementState`; not saved: a player
    /// who joins stands.
    #[serde(skip)]
    pub riding: Option<Riding>,
    /// The chair the player sits on (`sitting`). Part of the `MovementState`; not saved.
    #[serde(skip)]
    pub sitting: Option<super::sitting::Seat>,
    /// The status effects on the player (`effects`). Only the server changes them; saved.
    #[serde(default)]
    pub effects: StatusEffects,
    /// The level of slowness of the player, from its effects (`Player::tick_effects`). Part of
    /// the `MovementState`, as it slows the player. Not saved: the effects give it again.
    #[serde(skip)]
    pub slowness: u8,
    /// The level of swiftness of the player, as `slowness`.
    #[serde(skip)]
    pub swiftness: u8,
    /// How long the player has raised its shield, in ms (`shield::raise_shield`). Part of the
    /// `MovementState`. Not saved.
    #[serde(skip)]
    pub shield_ms: u32,
    /// The time left, in ms of input, before a shield knocked down may be raised again. Part
    /// of the `MovementState`. Not saved.
    #[serde(skip)]
    pub shield_disabled_ms: u32,
    /// How many hits the shield of the player parried, wrapping: the clients hear each one. Not
    /// saved.
    #[serde(skip)]
    pub parries: u8,
    /// The points of health the blows of hostiles struck at the player while it rode a steed
    /// took from the steed instead, which the server takes from it (`companions`). Not saved.
    #[serde(skip)]
    pub mount_blows: u32,
}

/// What the movement simulation carries over from one input to the next. The server sends it
/// with each acknowledgement, so that a client resumes its prediction from the very same state.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy, Default)]
pub struct MovementState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_flying: bool,
    pub is_sprinting: bool,
    pub is_sneaking: bool,
    pub is_swimming: bool,
    pub fall_distance: f32,
    pub portal_rest: Option<PortalRest>,
    /// `Player::dragged_ms`.
    pub dragged_ms: u32,
    /// `Player::bow_draw_ms`: the others see the bow drawn.
    pub bow_draw_ms: u32,
    /// `Player::riding`: the others see the player seated.
    pub riding: Option<Riding>,
    /// `Player::slowness`.
    pub slowness: u8,
    /// `Player::swiftness`.
    pub swiftness: u8,
    /// `Player::shield_ms`: the others see the shield raised.
    pub shield_ms: u32,
    /// `Player::shield_disabled_ms`.
    pub shield_disabled_ms: u32,
    /// `Player::sitting`: the others see the player seated.
    pub sitting: Option<super::sitting::Seat>,
}

/// The upward velocity a knock back gives at least, in blocks/s: a hop of a quarter of a block.
pub const KNOCK_BACK_HOP: f32 = 4.0;

fn full_health() -> u32 {
    MAX_HEALTH
}

/// Size of a player's collision box, in blocks.
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_WIDTH: f32 = 0.8;

impl Player {
    pub fn movement_state(&self) -> MovementState {
        MovementState {
            position: self.position,
            velocity: self.velocity,
            on_ground: self.on_ground,
            is_flying: self.is_flying,
            is_sprinting: self.is_sprinting,
            is_sneaking: self.is_sneaking,
            is_swimming: self.is_swimming,
            fall_distance: self.fall_distance,
            portal_rest: self.portal_rest,
            dragged_ms: self.dragged_ms,
            bow_draw_ms: self.bow_draw_ms,
            riding: self.riding,
            slowness: self.slowness,
            swiftness: self.swiftness,
            shield_ms: self.shield_ms,
            shield_disabled_ms: self.shield_disabled_ms,
            sitting: self.sitting,
        }
    }

    pub fn set_movement_state(&mut self, state: MovementState) {
        self.position = state.position;
        self.velocity = state.velocity;
        self.on_ground = state.on_ground;
        self.is_flying = state.is_flying;
        self.is_sprinting = state.is_sprinting;
        self.is_sneaking = state.is_sneaking;
        self.is_swimming = state.is_swimming;
        self.fall_distance = state.fall_distance;
        self.portal_rest = state.portal_rest;
        self.dragged_ms = state.dragged_ms;
        self.bow_draw_ms = state.bow_draw_ms;
        self.riding = state.riding;
        self.slowness = state.slowness;
        self.swiftness = state.swiftness;
        self.shield_ms = state.shield_ms;
        self.shield_disabled_ms = state.shield_disabled_ms;
        self.sitting = state.sitting;
    }

    /// What the portals know of the player going through them: its box, and the floor portal it
    /// rests on.
    pub fn passer(&self) -> Passer {
        Passer::new(
            Vec3::new(self.width, self.height, self.width) / 2.0,
            self.portal_rest,
        )
    }

    pub fn is_dead(&self) -> bool {
        self.health == 0
    }

    /// The one door of damage: takes `amount` points of health from `source`, if the game mode
    /// lets it and the player was not hit within the last `INVULNERABILITY_TICKS` (unless the
    /// source pierces that), and names the cause when none is left. A hit by a mob is
    /// remembered for `THROWN_OFF_TICKS` even when it is ignored, as its knock back is not.
    /// The armour worn absorbs its share first and wears (`Player::absorb`). Returns whether the
    /// hit landed, even when the armour left nothing of it.
    pub fn hurt(&mut self, amount: u32, source: DamageSource) -> bool {
        if amount == 0 || self.is_dead() || !self.game_mode.takes_damage() {
            return false;
        }
        if matches!(source, DamageSource::Fire | DamageSource::Lava) && self.resists_fire() {
            return false;
        }
        if let DamageSource::Mob(kind) = source {
            self.last_mob_hit = Some((kind, THROWN_OFF_TICKS));
        }
        if self.invulnerable_ticks > 0 && !source.pierces_invulnerability() {
            return false;
        }
        self.invulnerable_ticks = INVULNERABILITY_TICKS;
        let amount = self.absorb(amount, source);
        self.health = self.health.saturating_sub(amount);
        self.exhaust(DAMAGE_EXHAUSTION);
        if self.is_dead() {
            self.progress.stats.deaths += 1;
            self.waypoints.last_death = Some(self.position.floor().as_ivec3());
            let thrown_by = self.last_mob_hit.map(|(kind, _)| kind);
            self.death_cause = Some(DeathCause::of(source, thrown_by));
        }
        true
    }

    /// Counts down the invulnerability and the memory of the last mob hit, once a tick.
    pub fn tick_damage_timers(&mut self) {
        self.invulnerable_ticks = self.invulnerable_ticks.saturating_sub(1);
        self.last_mob_hit = self
            .last_mob_hit
            .map(|(kind, ticks)| (kind, ticks - 1))
            .filter(|&(_, ticks)| ticks > 0);
    }

    /// Pushes the player away along the horizontal part of `direction` at `strength` blocks/s,
    /// half that while it sneaks, and lifts it in a small hop. The movement simulation carries the push on through the air,
    /// as it does a portal's, and drops it on landing, in a liquid or in flight; as the velocity is part of the `MovementState`, the client resumes from the push.
    pub fn knock_back(&mut self, direction: Vec3, strength: f32) {
        if self.is_dead() {
            return;
        }
        let strength = if self.is_sneaking {
            strength / 2.0
        } else {
            strength
        };
        let push = direction.with_y(0.0).normalize_or_zero() * strength;
        self.velocity.x = push.x;
        self.velocity.z = push.z;
        if !self.is_flying {
            self.velocity.y = self.velocity.y.max(KNOCK_BACK_HOP);
        }
    }

    /// A Drowner grabs the player: in a liquid it is dragged down at `DRAG_SPEED` and cannot
    /// swim up for `DRAG_MS` of its inputs, which the movement simulation counts down.
    pub fn drag(&mut self) {
        if !self.is_dead() {
            self.dragged_ms = DRAG_MS;
        }
    }

    /// Brings a dead player back to life at `spawn`, standing still, in full health and fed.
    pub fn respawn(&mut self, spawn: Vec3) {
        self.set_movement_state(MovementState {
            position: spawn,
            on_ground: true,
            is_flying: self.is_flying && self.game_mode.can_fly(),
            ..Default::default()
        });
        self.health = MAX_HEALTH;
        self.armour_carry = 0;
        self.death_cause = None;
        self.invulnerable_ticks = 0;
        self.last_mob_hit = None;
        self.air = MAX_AIR;
        self.drowning_ticks = 0;
        self.burning_ticks = 0;
        self.fire_damage_wait = 0;
        self.lava_damage_wait = 0;
        self.food = MAX_FOOD;
        self.saturation = RESPAWN_SATURATION;
        self.exhaustion = 0.0;
        self.effects.clear();
    }

    pub fn new(id: PlayerId, name: String, position: Vec3, camera_transform: Transform) -> Self {
        Self {
            id,
            name,
            position,
            camera_transform,
            velocity: Vec3::ZERO,
            on_ground: true,
            is_flying: false,
            is_sprinting: false,
            is_sneaking: false,
            is_swimming: false,
            fall_distance: 0.0,
            game_mode: GameMode::default(),
            health: MAX_HEALTH,
            armour_carry: 0,
            death_cause: None,
            food: MAX_FOOD,
            saturation: RESPAWN_SATURATION,
            exhaustion: 0.0,
            experience: Experience::default(),
            depths_reached: Vec::new(),
            progress: Progress::default(),
            waypoints: Waypoints::default(),
            inventory: Inventory::new(),
            height: PLAYER_HEIGHT,
            width: PLAYER_WIDTH,
            last_input_processed: 0,
            mining_carry_ms: 0,
            place_wait_ms: 0,
            portal_triggers_held: [false; 2],
            portal_rest: None,
            invulnerable_ticks: 0,
            last_mob_hit: None,
            air: MAX_AIR,
            drowning_ticks: 0,
            burning_ticks: 0,
            fire_damage_wait: 0,
            lava_damage_wait: 0,
            dragged_ms: 0,
            bow_draw_ms: 0,
            is_swinging: false,
            hotbar_slot: 0,
            skin: PlayerSkin::default(),
            riding: None,
            sitting: None,
            effects: StatusEffects::default(),
            slowness: 0,
            swiftness: 0,
            shield_ms: 0,
            shield_disabled_ms: 0,
            parries: 0,
            mount_blows: 0,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new(0, "Default".into(), Vec3::ZERO, Transform::default())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Resource, Serialize, Deserialize, Default)]
pub enum ViewMode {
    #[default]
    FirstPerson,
    ThirdPerson,
}

impl ViewMode {
    pub fn toggle(&mut self) {
        *self = match *self {
            ViewMode::FirstPerson => ViewMode::ThirdPerson,
            ViewMode::ThirdPerson => ViewMode::FirstPerson,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::{INVULNERABILITY_TICKS, THROWN_OFF_TICKS};
    use crate::world::MobKind;

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    #[test]
    fn the_hand_holds_the_item_of_the_selected_hotbar_slot_and_nothing_beyond_the_hotbar() {
        let mut inventory = Inventory::new();
        inventory.inner.insert(0, stack(ItemId::Dirt, 1));
        inventory.inner.insert(8, stack(ItemId::Stick, 1));
        inventory
            .inner
            .insert(HOTBAR_SLOTS, stack(ItemId::Cobblestone, 1));
        let item_in_hand = |slot| inventory.in_hand(slot).map(|held| held.item);
        assert_eq!(item_in_hand(0), Some(ItemId::Dirt));
        assert_eq!(item_in_hand(8), Some(ItemId::Stick));
        assert_eq!(inventory.in_hand(1), None, "an empty slot");
        assert_eq!(
            inventory.in_hand(HOTBAR_SLOTS),
            None,
            "a slot of the backpack"
        );
        assert_eq!(inventory.in_hand(u32::MAX), None);
    }

    #[test]
    fn the_hand_tells_whether_its_item_is_enchanted() {
        use crate::world::Enchantment;
        let mut inventory = Inventory::new();
        let sword = stack(ItemId::WoodenSword, 1);
        inventory.inner.insert(0, sword);
        inventory
            .inner
            .insert(1, sword.enchanted(Enchantment::Sharpness).unwrap());
        assert_eq!(inventory.in_hand(0).map(|held| held.enchanted), Some(false));
        assert_eq!(inventory.in_hand(1).map(|held| held.enchanted), Some(true));
    }

    fn max_stack() -> u32 {
        ItemId::Dirt.get_max_stack()
    }

    fn survivor() -> Player {
        Player::new(1, "survivor".into(), Vec3::ZERO, Transform::default())
    }

    #[test]
    fn a_hit_takes_its_points_of_health() {
        let mut player = survivor();
        assert!(player.hurt(3, DamageSource::Fire));
        assert_eq!(player.health, MAX_HEALTH - 3);
        assert_eq!(player.death_cause, None);
    }

    #[test]
    fn creative_dead_players_and_harmless_hits_take_nothing() {
        let mut creative = Player {
            game_mode: GameMode::Creative,
            ..survivor()
        };
        assert!(!creative.hurt(5, DamageSource::Lava));
        assert!(!creative.hurt(5, DamageSource::Void));
        assert_eq!(creative.health, MAX_HEALTH);

        let mut player = survivor();
        assert!(!player.hurt(0, DamageSource::Fall));
        assert_eq!(
            player.invulnerable_ticks, 0,
            "a harmless hit protects nothing"
        );

        player.health = 0;
        assert!(!player.hurt(2, DamageSource::Void));
        assert_eq!(player.death_cause, None);
    }

    #[test]
    fn a_second_hit_within_ten_ticks_is_ignored() {
        let mut player = survivor();
        assert!(player.hurt(2, DamageSource::Mob(MobKind::Wolf)));
        for _ in 1..INVULNERABILITY_TICKS {
            player.tick_damage_timers();
            assert!(!player.hurt(2, DamageSource::Fire));
        }
        assert_eq!(player.health, MAX_HEALTH - 2);
        player.tick_damage_timers();
        assert!(player.hurt(2, DamageSource::Fire));
        assert_eq!(player.health, MAX_HEALTH - 4);
    }

    #[test]
    fn the_void_and_starvation_pierce_the_invulnerability() {
        let mut player = survivor();
        player.hurt(1, DamageSource::Fall);
        assert!(player.hurt(1, DamageSource::Starvation));
        assert!(player.hurt(player.health, DamageSource::Void));
        assert_eq!(player.death_cause, Some(DeathCause::Void));
    }

    #[test]
    fn the_last_hit_names_the_cause_of_death() {
        let mut player = survivor();
        player.hurt(MAX_HEALTH - 1, DamageSource::Fire);
        assert_eq!(player.death_cause, None);
        for _ in 0..INVULNERABILITY_TICKS {
            player.tick_damage_timers();
        }
        player.hurt(5, DamageSource::Drowning);
        assert!(player.is_dead());
        assert_eq!(player.death_cause, Some(DeathCause::Drowning));
    }

    #[test]
    fn a_deadly_fall_within_three_seconds_of_a_mob_hit_is_the_mobs() {
        let fall_after = |ticks: u32| {
            let mut player = survivor();
            player.hurt(1, DamageSource::Mob(MobKind::Goat));
            for _ in 0..ticks {
                player.tick_damage_timers();
            }
            player.hurt(MAX_HEALTH, DamageSource::Fall);
            player.death_cause
        };
        assert_eq!(fall_after(20), Some(DeathCause::ThrownOff(MobKind::Goat)));
        assert_eq!(
            fall_after(THROWN_OFF_TICKS - 1),
            Some(DeathCause::ThrownOff(MobKind::Goat))
        );
        assert_eq!(fall_after(THROWN_OFF_TICKS), Some(DeathCause::Fall));
    }

    #[test]
    fn a_mob_hit_swallowed_by_the_invulnerability_still_names_the_mob() {
        let mut player = survivor();
        player.hurt(1, DamageSource::Fire);
        assert!(!player.hurt(1, DamageSource::Mob(MobKind::Wolf)));
        for _ in 0..INVULNERABILITY_TICKS {
            player.tick_damage_timers();
        }
        player.hurt(MAX_HEALTH, DamageSource::Fall);
        assert_eq!(
            player.death_cause,
            Some(DeathCause::ThrownOff(MobKind::Wolf))
        );
    }

    #[test]
    fn a_respawn_forgets_the_timers() {
        let mut player = survivor();
        player.hurt(1, DamageSource::Mob(MobKind::Wolf));
        player.hurt(MAX_HEALTH, DamageSource::Void);
        player.respawn(Vec3::ONE);
        assert_eq!(player.invulnerable_ticks, 0);
        assert_eq!(player.last_mob_hit, None);
        assert!(player.hurt(1, DamageSource::Fall));
        player.health = 1;
        player.invulnerable_ticks = 0;
        player.hurt(1, DamageSource::Fall);
        assert_eq!(player.death_cause, Some(DeathCause::Fall));
    }

    #[test]
    fn a_knock_back_pushes_along_the_ground_with_a_hop() {
        let mut player = survivor();
        player.knock_back(Vec3::new(3.0, 5.0, 4.0), 10.0);
        assert!((player.velocity.x - 6.0).abs() < 1e-5);
        assert!((player.velocity.z - 8.0).abs() < 1e-5);
        assert_eq!(player.velocity.y, KNOCK_BACK_HOP);

        // A dead player stays where it lies
        let mut dead = Player {
            health: 0,
            ..survivor()
        };
        dead.knock_back(Vec3::X, 10.0);
        assert_eq!(dead.velocity, Vec3::ZERO);
    }

    #[test]
    fn add_to_empty_inventory_uses_first_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 5));
        assert_eq!(inventory.inner.len(), 1);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
    }

    #[test]
    fn add_stacks_onto_existing_partial_stack() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 5));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 7));
        assert_eq!(inventory.inner.len(), 1);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 12));
    }

    #[test]
    fn add_different_item_goes_to_next_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 5));
        inventory.add_item_to_inventory(stack(ItemId::Sand, 2));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 5));
        assert_eq!(inventory.inner[&1], stack(ItemId::Sand, 2));
    }

    #[test]
    fn add_overflowing_stack_spills_into_next_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack() - 4));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.inner.len(), 2);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 6));
    }

    #[test]
    fn add_more_than_a_stack_to_empty_inventory_spills_into_next_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack() + 36));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 36));
    }

    #[test]
    fn add_returns_what_did_not_fit() {
        let mut inventory = Inventory::new();
        for slot in 1..MAX_INVENTORY_SLOTS {
            inventory.inner.insert(slot, stack(ItemId::Sand, 1));
        }
        assert_eq!(inventory.add_item_to_inventory(stack(ItemId::Dirt, 3)), 0);
        let lost = inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack()));
        assert_eq!(lost, 3);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
    }

    #[test]
    fn adding_nothing_takes_no_slot() {
        let mut inventory = Inventory::new();
        assert_eq!(inventory.add_item_to_inventory(stack(ItemId::Dirt, 0)), 0);
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn add_tops_up_a_later_partial_stack_before_an_earlier_empty_slot() {
        let mut inventory = Inventory::new();
        inventory
            .inner
            .insert(3, stack(ItemId::Dirt, max_stack() - 4));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.inner[&3], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 6));
        assert_eq!(inventory.inner.len(), 2);
    }

    #[test]
    fn add_skips_full_stack_of_same_item() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack()));
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 1));
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, max_stack()));
        assert_eq!(inventory.inner[&1], stack(ItemId::Dirt, 1));
    }

    #[test]
    fn filling_every_slot_then_adding_more_changes_nothing() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, max_stack() * MAX_INVENTORY_SLOTS));
        assert_eq!(inventory.inner.len(), MAX_INVENTORY_SLOTS as usize);
        for slot in 0..MAX_INVENTORY_SLOTS {
            assert_eq!(inventory.inner[&slot], stack(ItemId::Dirt, max_stack()));
        }

        let full = inventory.clone();
        let lost = inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(lost, 10);
        inventory.add_item_to_inventory(stack(ItemId::Sand, 10));
        assert_eq!(inventory, full);
    }

    #[test]
    fn add_item_to_stack_caps_at_max_stack() {
        let mut inventory = Inventory::new();
        assert_eq!(
            inventory.add_item_to_stack(3, 60, ItemId::Dirt, ItemId::Dirt.get_default_type()),
            60
        );
        assert_eq!(
            inventory.add_item_to_stack(3, 10, ItemId::Dirt, ItemId::Dirt.get_default_type()),
            4
        );
        assert_eq!(inventory.inner[&3], stack(ItemId::Dirt, max_stack()));
    }

    #[test]
    fn remove_partial_leaves_remainder() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.remove_item_from_stack(0, 4), 4);
        assert_eq!(inventory.inner[&0], stack(ItemId::Dirt, 6));
    }

    #[test]
    fn remove_total_empties_the_slot() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.remove_item_from_stack(0, 10), 10);
        assert!(!inventory.inner.contains_key(&0));
    }

    #[test]
    fn remove_more_than_present_returns_only_what_was_there() {
        let mut inventory = Inventory::new();
        inventory.add_item_to_inventory(stack(ItemId::Dirt, 10));
        assert_eq!(inventory.remove_item_from_stack(0, 25), 10);
        assert!(!inventory.inner.contains_key(&0));
    }

    #[test]
    fn remove_from_empty_slot_returns_zero() {
        let mut inventory = Inventory::new();
        assert_eq!(inventory.remove_item_from_stack(5, 3), 0);
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn a_tool_wears_down_then_breaks_at_its_durability() {
        let durability = ItemId::WoodenAxe.tool().unwrap().durability;
        let mut inventory = Inventory::new();
        inventory.inner.insert(2, stack(ItemId::WoodenAxe, 1));
        for wear in 1..durability {
            assert!(!inventory.wear_tool(2, 1));
            assert_eq!(inventory.inner[&2].item_type, ItemType::Tool { wear });
        }
        assert!(inventory.wear_tool(2, 1));
        assert!(inventory.inner.is_empty());
    }

    #[test]
    fn wearing_anything_but_a_tool_changes_nothing() {
        let mut inventory = Inventory::new();
        inventory.inner.insert(0, stack(ItemId::Dirt, 3));
        let before = inventory.clone();
        assert!(!inventory.wear_tool(0, 1));
        assert!(!inventory.wear_tool(1, 1));
        assert_eq!(inventory, before);
    }

    #[test]
    fn a_worn_tool_keeps_its_wear_when_moved_around() {
        let worn = ItemStack {
            item_type: ItemType::Tool { wear: 30 },
            ..stack(ItemId::WoodenPickaxe, 1)
        };
        let mut inventory = Inventory::new();
        assert_eq!(inventory.add_item_to_inventory(worn), 0);
        assert_eq!(inventory.inner[&0], worn);
        // A second tool never stacks onto the first one
        assert_eq!(
            inventory.add_item_to_inventory(stack(ItemId::WoodenPickaxe, 1)),
            0
        );
        assert_eq!(inventory.inner[&0], worn);
        assert_eq!(inventory.inner[&1], stack(ItemId::WoodenPickaxe, 1));
    }
}
