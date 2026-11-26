//! The shield: raised while the right button is held, it parries the blows and the shots that
//! come from the front of the player, and wears by what it stops. It is held in the selected
//! hotbar slot, or, as in the other hand, anywhere in the hotbar while the player holds a melee
//! weapon or nothing. An axe's blow (`WeaponClass::breaks_shields`) or a Roller's charge knocks
//! it down for `DISABLED_MS`.
//!
//! The raise is part of the movement the client predicts and replays
//! (`MovementState::shield_ms`), as it slows the player and the others see it.

use bevy::math::Vec3;

use super::blocks::held_item;
use super::melee::WeaponClass;
use super::movement::MAX_FRAME_DELTA_MS;
use super::Player;
use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    world::ItemId,
    HOTBAR_SLOTS,
};

/// How long the shield takes to be raised, in ms: a parry is no reflex.
pub const RAISE_MS: u32 = 250;
/// The speed of the player against its walk while it raises its shield.
pub const RAISED_SPEED_MULTIPLIER: f32 = 0.5;
/// How long a shield knocked down stays down, in ms of input.
pub const DISABLED_MS: u32 = 5000;
/// What the shield wears out at, in points of damage stopped.
pub const SHIELD_DURABILITY: u32 = 336;
/// The cosine of the widest angle from the look of the player a parried hit comes from.
const FRONT_COS: f32 = 0.0;

/// What a raised shield made of a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parry {
    /// It stopped it.
    Blocked,
    /// It stopped it, but was knocked down for `DISABLED_MS`.
    KnockedDown,
}

/// The hotbar slot of the shield `player` raises with the hotbar slot `selected`: that slot if it
/// holds a shield, else the first shield of the hotbar while the hand holds a melee weapon or
/// nothing.
pub fn shield_slot(player: &Player, selected: u32) -> Option<u32> {
    let holds = |slot: u32| {
        player
            .inventory
            .inner
            .get(&slot)
            .is_some_and(|stack| stack.item_id == ItemId::Shield)
    };
    if selected < HOTBAR_SLOTS && holds(selected) {
        return Some(selected);
    }
    let in_hand = player
        .inventory
        .inner
        .get(&selected)
        .map(|stack| stack.item_id);
    let free_hand = matches!(
        WeaponClass::of(in_hand),
        WeaponClass::Sword | WeaponClass::Axe | WeaponClass::Spear | WeaponClass::Mace
    ) || in_hand.is_none();
    if !free_hand {
        return None;
    }
    (0..HOTBAR_SLOTS).find(|&slot| holds(slot))
}

/// Raises the shield while the right button is down, and lowers it otherwise: part of the
/// movement, which a client replays. A press a block took (`Player::place_wait_ms`) raises
/// nothing, nor one while the bow is drawn or the shield knocked down, which counts down here.
pub fn raise_shield(player: &mut Player, action: &PlayerFrameInput) {
    let delta = action.delta_ms.min(MAX_FRAME_DELTA_MS) as u32;
    player.shield_disabled_ms = player.shield_disabled_ms.saturating_sub(delta);
    let raised_ms = player.shield_ms;
    let raises = action.is_pressed(NetworkAction::RightClick)
        && !action.is_pressed(NetworkAction::LowerBow)
        && player.bow_draw_ms == 0
        && player.shield_disabled_ms == 0
        && player.riding.is_none()
        && (raised_ms > 0 || player.place_wait_ms == 0)
        && shield_slot(player, action.hotbar_slot).is_some()
        // A held item with a use of its own keeps the button
        && !held_item(player, action).is_some_and(|item| item.food_value().is_some());
    player.shield_ms = if raises {
        (raised_ms + delta).min(RAISE_MS)
    } else {
        0
    };
}

/// Whether the shield of `player` is up, and parries.
pub fn is_raised(player: &Player) -> bool {
    player.shield_ms >= RAISE_MS
}

/// Whether a hit from `from` comes from the front of a player at `at` looking along `look`, on
/// the horizontal plane. One from right above or below is not.
pub fn in_front(look: Vec3, at: Vec3, from: Vec3) -> bool {
    let toward = (from - at).with_y(0.0).normalize_or_zero();
    let look = look.with_y(0.0).normalize_or_zero();
    toward != Vec3::ZERO && look.dot(toward) > FRONT_COS
}

impl Player {
    /// The raised shield of the player meets a hit of `damage` points coming from `from`: it
    /// parries it if it comes from the front, and wears by it; one that `knocks_down` also
    /// knocks it down. Returns `None` when the hit is not parried, and is to land.
    pub fn parry(&mut self, from: Vec3, damage: u32, knocks_down: bool) -> Option<Parry> {
        if self.is_dead() || !is_raised(self) {
            return None;
        }
        let look = self.camera_transform.forward().as_vec3();
        if !in_front(look, self.position, from) {
            return None;
        }
        let slot = shield_slot(self, self.hotbar_slot)?;
        if self.game_mode.takes_damage() {
            self.inventory.wear_tool(slot, damage.max(1));
        }
        self.parries = self.parries.wrapping_add(1);
        if knocks_down {
            self.shield_ms = 0;
            self.shield_disabled_ms = DISABLED_MS;
            return Some(Parry::KnockedDown);
        }
        Some(Parry::Blocked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ItemStack, ItemType};
    use bevy::prelude::Transform;

    fn guard(items: &[(u32, ItemId)]) -> Player {
        let mut player = Player::new(1, "guard".into(), Vec3::ZERO, Transform::default());
        // Looking along -Z
        player.camera_transform = Transform::default().looking_to(Vec3::NEG_Z, Vec3::Y);
        for &(slot, item_id) in items {
            let stack = ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb: 1,
            };
            player.inventory.inner.insert(slot, stack);
        }
        player
    }

    fn hold_right(player: &mut Player, slot: u32, ms: u64) {
        let mut input = PlayerFrameInput {
            delta_ms: 50,
            hotbar_slot: slot,
            ..Default::default()
        };
        input.inputs.insert(NetworkAction::RightClick);
        for _ in 0..ms / 50 {
            raise_shield(player, &input);
        }
        player.hotbar_slot = slot;
    }

    #[test]
    fn a_raised_shield_parries_from_the_front_only_and_wears() {
        let mut player = guard(&[(0, ItemId::Shield)]);
        hold_right(&mut player, 0, 100);
        assert!(!is_raised(&player), "not yet up");
        assert_eq!(player.parry(Vec3::new(0.0, 0.0, -3.0), 4, false), None);
        hold_right(&mut player, 0, 300);
        assert!(is_raised(&player));
        assert_eq!(
            player.parry(Vec3::new(1.0, 1.0, -3.0), 4, false),
            Some(Parry::Blocked)
        );
        assert_eq!(player.inventory.inner[&0].item_type, ItemType::Tool { wear: 4 });
        assert_eq!(player.parry(Vec3::new(0.0, 0.0, 3.0), 4, false), None, "behind");
        assert_eq!(player.parry(Vec3::new(3.0, 0.0, 0.1), 4, false), None, "beside");
        assert_eq!(player.parries, 1);
    }

    #[test]
    fn a_shield_in_the_hotbar_is_raised_beside_a_sword_but_not_beside_a_bow() {
        let mut player = guard(&[(0, ItemId::IronSword), (1, ItemId::Bow), (5, ItemId::Shield)]);
        assert_eq!(shield_slot(&player, 0), Some(5));
        assert_eq!(shield_slot(&player, 1), None);
        assert_eq!(shield_slot(&player, 2), Some(5), "an empty hand");
        hold_right(&mut player, 0, 300);
        assert!(is_raised(&player));
    }

    #[test]
    fn an_axe_knocks_the_shield_down_for_a_while() {
        let mut player = guard(&[(0, ItemId::Shield)]);
        hold_right(&mut player, 0, 300);
        let from = Vec3::new(0.0, 0.0, -2.0);
        assert_eq!(player.parry(from, 6, true), Some(Parry::KnockedDown));
        assert!(!is_raised(&player));
        hold_right(&mut player, 0, 1000);
        assert!(!is_raised(&player), "still down");
        hold_right(&mut player, 0, DISABLED_MS as u64);
        assert!(is_raised(&player));
    }
}
