//! The rhythm of the blows: each weapon recharges at its own pace, and a blow hurts by how far
//! it was recharged (`blow_damage`). A full blow while falling is a critical one, and a full
//! blow of a sword on the ground sweeps the creatures around its target too (`in_sweep`).
//!
//! The recharge follows the inputs alone (`Recharge::step`), so that the server, which counts
//! the blows that land, and the client, which shows the gauge, count it alike.

use bevy::math::Vec3;
use serde::{Deserialize, Serialize};

use crate::world::{ItemId, ToolKind};

/// What a weapon is, for its rhythm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponClass {
    /// The bare hand, or an item that is no weapon.
    Hand,
    /// Quick blows, which sweep.
    Sword,
    /// Slow, heavy blows, which will break the shields.
    Axe,
    /// Blows from afar, at a steady pace.
    Spear,
    /// The slowest and heaviest blows, which throw back far (`WeaponClass::knock_back`).
    Mace,
    /// A pickaxe, a shovel or a hoe: a tool, not a weapon.
    Tool,
}

/// How a weapon recharges, in words for its tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    Quick,
    Steady,
    Slow,
}

impl WeaponClass {
    pub fn of(held: Option<ItemId>) -> Self {
        let Some(item) = held else {
            return Self::Hand;
        };
        match item {
            ItemId::Spear => return Self::Spear,
            ItemId::Mace => return Self::Mace,
            ItemId::ThrowingAxe => return Self::Axe,
            _ => {}
        }
        match item.tool().map(|tool| tool.kind) {
            Some(ToolKind::Sword) => Self::Sword,
            Some(ToolKind::Axe) => Self::Axe,
            Some(_) => Self::Tool,
            None => Self::Hand,
        }
    }

    /// The time it takes to recharge a blow, in ms.
    pub fn recharge_ms(self) -> u32 {
        match self {
            Self::Hand => 400,
            Self::Sword => 500,
            Self::Tool => 650,
            Self::Spear => 750,
            Self::Axe => 1000,
            Self::Mace => 1400,
        }
    }

    /// What the weapon multiplies the throw back of a blow by.
    pub fn knock_back(self) -> f32 {
        if self == Self::Mace {
            2.2
        } else {
            1.0
        }
    }

    pub fn pace(self) -> Pace {
        match self.recharge_ms() {
            ..=500 => Pace::Quick,
            ..=750 => Pace::Steady,
            _ => Pace::Slow,
        }
    }

    /// Whether a full blow on the ground sweeps the creatures around its target.
    pub fn sweeps(self) -> bool {
        self == Self::Sword
    }

    /// Whether a blow knocks down the shield it meets for a while (`shield::Parry`), or the
    /// bark of a Stumpling.
    pub fn breaks_shields(self) -> bool {
        self == Self::Axe
    }
}

/// Below this recharge, a blow is a hurried one: neither critical nor sweeping.
pub const FULL_BLOW: f32 = 0.9;
/// What a critical blow multiplies the damage by.
pub const CRITICAL_FACTOR: f32 = 1.5;
/// How far from the target of a sweep the creatures it touches stand, in blocks.
pub const SWEEP_RADIUS: f32 = 2.0;

/// The recharge of a player's blows: the time since its last swing, the item it was for, and
/// whether the left button was down. Every press is a swing, whatever it meets, and so is a
/// full recharge while the button stays down; changing the item held starts it again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recharge {
    rest_ms: u32,
    /// The item held at the last input; `None` before the first, which finds it full.
    weapon: Option<Option<ItemId>>,
    down: bool,
}

impl Default for Recharge {
    /// Full: the first blow of a player who just came is not a hurried one.
    fn default() -> Self {
        Self {
            rest_ms: u32::MAX,
            weapon: None,
            down: false,
        }
    }
}

impl Recharge {
    /// How far the blow with `held` is recharged, from 0 to 1.
    pub fn charge(&self, held: Option<ItemId>) -> f32 {
        if self.weapon.is_some_and(|weapon| weapon != held) {
            return 0.0;
        }
        (self.rest_ms as f32 / WeaponClass::of(held).recharge_ms() as f32).min(1.0)
    }

    /// Counts the input of `delta_ms` holding `held`, the left button `pressed` or not.
    /// Returns the recharge of the swing it makes, if it makes one.
    pub fn step(&mut self, held: Option<ItemId>, pressed: bool, delta_ms: u64) -> Option<f32> {
        if self.weapon.is_some_and(|weapon| weapon != held) {
            self.rest_ms = 0;
        }
        self.weapon = Some(held);
        // As the blows did: a long stall is no long rest
        self.rest_ms = self.rest_ms.saturating_add(delta_ms.min(100) as u32);
        let charge = self.charge(held);
        let swings = pressed && (!self.down || charge >= 1.0);
        self.down = pressed;
        if swings {
            self.rest_ms = 0;
        }
        swings.then_some(charge)
    }
}

/// The damage of a blow of `base` points recharged to `charge`, critical or not: a fifth of it
/// when hurried, all of it when full, and half as much again when critical; never nothing.
pub fn blow_damage(base: u32, charge: f32, critical: bool) -> u32 {
    let charge = charge.clamp(0.0, 1.0);
    let factor = 0.2 + 0.8 * charge * charge;
    let critical = if critical { CRITICAL_FACTOR } else { 1.0 };
    ((base as f32 * factor * critical).round() as u32).max(1)
}

/// Whether a blow recharged to `charge` by a player in the air is critical: a full blow while it
/// falls, neither flying nor swimming.
pub fn is_critical(charge: f32, on_ground: bool, falling_speed: f32, flies_or_swims: bool) -> bool {
    charge >= FULL_BLOW && !on_ground && falling_speed > 0.0 && !flies_or_swims
}

/// Whether a blow of `class` recharged to `charge` sweeps: a full blow of a sword, the striker
/// standing on the ground.
pub fn sweeps(class: WeaponClass, charge: f32, on_ground: bool) -> bool {
    class.sweeps() && charge >= FULL_BLOW && on_ground
}

/// Whether a sweep of a striker at `striker` looking along `forward`, whose blow lands on a
/// creature at `target`, touches the creature at `other`: near the target, and in front of the
/// striker.
pub fn in_sweep(striker: Vec3, forward: Vec3, target: Vec3, other: Vec3) -> bool {
    let ahead = (other - striker).with_y(0.0).dot(forward.with_y(0.0));
    ahead > 0.0 && other.distance(target) <= SWEEP_RADIUS
}

/// The damage a sweep does to the creatures around the target of a blow of `damage`.
pub fn sweep_damage(damage: u32) -> u32 {
    (damage / 3).max(1)
}

/// What made a blow special, which the clients show and hear.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strike {
    /// A full blow while falling.
    Critical,
    /// The sweep of a sword: its target, and the creatures around it.
    Sweep,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hurried_blow_does_a_fifth_a_full_one_all_and_a_critical_half_again() {
        assert_eq!(blow_damage(10, 0.0, false), 2);
        assert_eq!(blow_damage(10, 0.5, false), 4);
        assert_eq!(blow_damage(10, 1.0, false), 10);
        assert_eq!(blow_damage(10, 1.0, true), 15);
        // A fist's hurried blow still lands
        assert_eq!(blow_damage(1, 0.0, false), 1);
    }

    #[test]
    fn each_weapon_has_its_pace_and_the_heavy_axe_strikes_harder_but_slower() {
        let class = |item| WeaponClass::of(Some(item));
        assert_eq!(WeaponClass::of(None).pace(), Pace::Quick);
        assert_eq!(class(ItemId::IronSword).pace(), Pace::Quick);
        assert_eq!(class(ItemId::Spear).pace(), Pace::Steady);
        assert_eq!(class(ItemId::IronAxe).pace(), Pace::Slow);
        assert!(class(ItemId::IronAxe).breaks_shields());
        assert!(!class(ItemId::IronSword).breaks_shields());
        assert_eq!(class(ItemId::Mace).pace(), Pace::Slow);
        assert!(class(ItemId::Mace).knock_back() > class(ItemId::IronAxe).knock_back());
        assert!(ItemId::Mace.attack_damage() > ItemId::DiamondAxe.attack_damage());
        let (sword, axe) = (ItemId::IronSword, ItemId::IronAxe);
        assert!(axe.attack_damage() > sword.attack_damage());
        // Per second, the sword still wins
        let per_second = |item: ItemId| item.attack_damage() * 1000 / class(item).recharge_ms();
        assert!(per_second(sword) > per_second(axe));
    }

    #[test]
    fn a_press_swings_with_what_was_recharged_and_holding_swings_when_full() {
        let sword = Some(ItemId::IronSword);
        let mut recharge = Recharge::default();
        // The first press finds it full
        assert_eq!(recharge.step(sword, true, 50), Some(1.0));
        // Held, it swings again once full: 500 ms later
        for _ in 0..9 {
            assert_eq!(recharge.step(sword, true, 50), None);
        }
        assert_eq!(recharge.step(sword, true, 50), Some(1.0));
        recharge.step(sword, false, 50);
        // Pressed again 100 ms after the last swing: a hurried blow
        assert_eq!(recharge.step(sword, true, 50), Some(0.2));
    }

    #[test]
    fn changing_the_item_held_starts_the_recharge_again() {
        let mut recharge = Recharge::default();
        recharge.step(Some(ItemId::IronSword), false, 50);
        assert_eq!(recharge.charge(Some(ItemId::IronSword)), 1.0);
        assert_eq!(recharge.charge(Some(ItemId::IronAxe)), 0.0);
        assert_eq!(recharge.step(Some(ItemId::IronAxe), true, 100), Some(0.1));
    }

    #[test]
    fn a_full_blow_while_falling_is_critical() {
        assert!(is_critical(1.0, false, 3.0, false));
        assert!(!is_critical(0.5, false, 3.0, false), "hurried");
        assert!(!is_critical(1.0, true, 0.0, false), "on the ground");
        assert!(!is_critical(1.0, false, -2.0, false), "rising");
        assert!(!is_critical(1.0, false, 3.0, true), "flying");
    }

    #[test]
    fn a_full_blow_of_a_sword_on_the_ground_sweeps_what_is_near_its_target_and_ahead() {
        assert!(sweeps(WeaponClass::Sword, 1.0, true));
        assert!(!sweeps(WeaponClass::Sword, 0.5, true));
        assert!(!sweeps(WeaponClass::Sword, 1.0, false));
        assert!(!sweeps(WeaponClass::Axe, 1.0, true));
        let (striker, forward) = (Vec3::ZERO, Vec3::NEG_Z);
        let target = Vec3::new(0.0, 0.0, -2.0);
        assert!(in_sweep(
            striker,
            forward,
            target,
            Vec3::new(1.5, 0.0, -2.0)
        ));
        assert!(
            !in_sweep(striker, forward, target, Vec3::new(0.0, 0.0, -4.5)),
            "far"
        );
        assert!(
            !in_sweep(striker, forward, target, Vec3::new(1.0, 0.0, 0.5)),
            "behind"
        );
        assert_eq!(sweep_damage(7), 2);
        assert_eq!(sweep_damage(1), 1);
    }
}
