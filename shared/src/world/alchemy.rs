//! Alchemy: the alembic distils what the world gives into potions. An alembic holds a `Furnace`
//! (`BlockId::Alembic`): its input is the ingredient, its fuel burns as a furnace's, and its
//! output is the vial, which an ingredient turns into another in place every `DISTILL_TICKS`
//! (`Furnace::tick_as`, `distilled`).
//!
//! A glass vial is filled at a source of water; the water distilled with a mushroom is the
//! neutral base, and the base with an ingredient a potion. A potion distilled again with moss
//! lasts twice as long (`LONGER`), with a crystal shard is a level stronger for half the time
//! (`STRONGER`), each once: the potion keeps them in its `ItemType::Brewed`. The potions are
//! drunk, but the splash potions, which are thrown and burst (`SPLASH_RADIUS`).

use crate::players::effects::EffectKind;
use crate::world::{ItemId, ItemStack, ItemType};
use crate::TICKS_PER_SECOND;

/// Ticks to distil one ingredient: 15 seconds.
pub const DISTILL_TICKS: u16 = 300;
/// The ingredient that makes a potion last twice as long.
pub const LONGER: ItemId = ItemId::Moss;
/// The ingredient that makes a potion a level stronger, for half its time.
pub const STRONGER: ItemId = ItemId::CrystalShard;
/// How far around where it bursts a splash potion reaches, in blocks.
pub const SPLASH_RADIUS: f32 = 4.0;

/// What an ingredient makes of what the vial holds, without the modifiers: water and a mushroom
/// make the base, the base and an ingredient a potion. The item browser lists them.
pub const RECIPES: [(ItemId, ItemId, ItemId); 11] = [
    (ItemId::WaterVial, ItemId::Mushroom, ItemId::NeutralBase),
    (ItemId::NeutralBase, ItemId::Apple, ItemId::HealingPotion),
    (
        ItemId::NeutralBase,
        ItemId::RawGlowfin,
        ItemId::NightVisionPotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::Kelp,
        ItemId::WaterBreathingPotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::Magma,
        ItemId::FireResistancePotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::Feather,
        ItemId::SwiftnessPotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::GnawerFang,
        ItemId::StrengthPotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::Berries,
        ItemId::RegenerationPotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::CaveVine,
        ItemId::SplashPoisonPotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::Clay,
        ItemId::SplashSlownessPotion,
    ),
    (
        ItemId::NeutralBase,
        ItemId::Heather,
        ItemId::SplashWeaknessPotion,
    ),
];

/// What a potion does: its effect, at level 1, for `seconds` (none for healing), and whether it
/// is thrown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PotionKind {
    pub effect: EffectKind,
    pub seconds: u32,
    pub splash: bool,
}

/// The modifiers distilled into a potion, from its `ItemType::Brewed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Brew {
    pub longer: bool,
    pub stronger: bool,
}

impl Brew {
    /// The modifiers of a potion of type `item_type`: none but those of `ItemType::Brewed`.
    pub fn of(item_type: ItemType) -> Self {
        match item_type {
            ItemType::Brewed { longer, stronger } => Self { longer, stronger },
            _ => Self::default(),
        }
    }

    /// The type of a potion with these modifiers: `Generic` for none.
    pub fn item_type(self) -> ItemType {
        if self == Self::default() {
            ItemType::Generic
        } else {
            ItemType::Brewed {
                longer: self.longer,
                stronger: self.stronger,
            }
        }
    }
}

impl ItemId {
    /// Whether the item is a vial with something in it, which stacks to one: water, the base or
    /// a potion.
    pub fn is_vial_filled(self) -> bool {
        matches!(self, ItemId::WaterVial | ItemId::NeutralBase) || self.potion().is_some()
    }

    /// Whether the item goes in the vial slot of an alembic.
    pub fn is_vial(self) -> bool {
        self == ItemId::GlassVial || self.is_vial_filled()
    }

    /// What the potion does, if the item is one.
    pub fn potion(self) -> Option<PotionKind> {
        let (effect, seconds, splash) = match self {
            ItemId::HealingPotion => (EffectKind::Healing, 0, false),
            ItemId::NightVisionPotion => (EffectKind::NightVision, 180, false),
            ItemId::WaterBreathingPotion => (EffectKind::WaterBreathing, 180, false),
            ItemId::FireResistancePotion => (EffectKind::FireResistance, 180, false),
            ItemId::SwiftnessPotion => (EffectKind::Swiftness, 180, false),
            ItemId::StrengthPotion => (EffectKind::Strength, 180, false),
            ItemId::RegenerationPotion => (EffectKind::Regeneration, 45, false),
            ItemId::SplashPoisonPotion => (EffectKind::Poison, 30, true),
            ItemId::SplashSlownessPotion => (EffectKind::Slowness, 60, true),
            ItemId::SplashWeaknessPotion => (EffectKind::Weakness, 60, true),
            _ => return None,
        };
        Some(PotionKind {
            effect,
            seconds,
            splash,
        })
    }

    /// Whether the item is a splash potion, thrown as a throwing axe is.
    pub fn is_splash_potion(self) -> bool {
        self.potion().is_some_and(|potion| potion.splash)
    }

    /// The colour of the potion, as its vial and its burst show it: sRGB.
    pub fn potion_color(self) -> Option<[u8; 3]> {
        Some(match self {
            ItemId::WaterVial => [56, 92, 214],
            ItemId::NeutralBase => [150, 170, 210],
            ItemId::HealingPotion => [248, 36, 35],
            ItemId::NightVisionPotion => [31, 31, 161],
            ItemId::WaterBreathingPotion => [46, 82, 153],
            ItemId::FireResistancePotion => [228, 154, 58],
            ItemId::SwiftnessPotion => [124, 175, 198],
            ItemId::StrengthPotion => [147, 36, 35],
            ItemId::RegenerationPotion => [205, 92, 171],
            ItemId::SplashPoisonPotion => [78, 147, 49],
            ItemId::SplashSlownessPotion => [90, 108, 129],
            ItemId::SplashWeaknessPotion => [72, 77, 72],
            _ => return None,
        })
    }
}

impl PotionKind {
    /// Whether a crystal shard makes it stronger: not those whose level does nothing.
    pub fn strengthens(self) -> bool {
        !matches!(
            self.effect,
            EffectKind::NightVision | EffectKind::WaterBreathing | EffectKind::FireResistance
        )
    }

    /// Whether moss makes it last longer: not healing, which lasts nothing.
    pub fn lengthens(self) -> bool {
        !self.effect.is_instant()
    }

    /// The effect it gives with `brew`: its kind, its level and its ticks. Stronger, it is a
    /// level higher for half the time; longer, twice the time.
    pub fn effect(self, brew: Brew) -> (EffectKind, u8, u32) {
        let mut ticks = self.seconds * TICKS_PER_SECOND as u32;
        if brew.longer {
            ticks *= 2;
        }
        let level = if brew.stronger {
            ticks /= 2;
            2
        } else {
            1
        };
        (self.effect, level, ticks)
    }
}

/// The effect a potion stack gives: its kind, its level and its ticks.
pub fn potion_effect(stack: &ItemStack) -> Option<(EffectKind, u8, u32)> {
    Some(stack.item_id.potion()?.effect(Brew::of(stack.item_type)))
}

/// The share of the effect of a splash potion a player `distance` blocks from its burst gets:
/// all of it on the spot, half of it at the edge, nothing beyond.
pub fn splash_share(distance: f32) -> f32 {
    if distance > SPLASH_RADIUS {
        0.0
    } else {
        1.0 - 0.5 * distance / SPLASH_RADIUS
    }
}

/// What `ingredient` makes of the `vial` in an alembic, if anything: the stack that takes its
/// place, as many of them. The server distils by it and the client shows it.
pub fn distilled(ingredient: ItemId, vial: &ItemStack) -> Option<ItemStack> {
    let made = |item_id: ItemId, item_type: ItemType| ItemStack {
        item_id,
        item_type,
        nb: vial.nb,
    };
    if let Some(&(_, _, result)) = RECIPES
        .iter()
        .find(|(base, with, _)| *base == vial.item_id && *with == ingredient)
    {
        return Some(made(result, result.get_default_type()));
    }
    let potion = vial.item_id.potion()?;
    let mut brew = Brew::of(vial.item_type);
    match ingredient {
        LONGER if potion.lengthens() && !brew.longer => brew.longer = true,
        STRONGER if potion.strengthens() && !brew.stronger => brew.stronger = true,
        _ => return None,
    }
    Some(made(vial.item_id, brew.item_type()))
}

/// Whether `item` is distilled by an alembic into something, in a vial: what its input slot is
/// for.
pub fn is_ingredient(item: ItemId) -> bool {
    item == LONGER || item == STRONGER || RECIPES.iter().any(|(_, with, _)| *with == item)
}

/// The recipes of the alembic that make `result`, as (vial, ingredient): the item browser lists
/// them.
pub fn distilling_inputs(result: ItemId) -> impl Iterator<Item = (ItemId, ItemId)> {
    RECIPES
        .into_iter()
        .filter(move |(_, _, made)| *made == result)
        .map(|(vial, with, _)| (vial, with))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item_id: ItemId, item_type: ItemType) -> ItemStack {
        ItemStack {
            item_id,
            item_type,
            nb: 1,
        }
    }

    fn plain(item_id: ItemId) -> ItemStack {
        stack(item_id, item_id.get_default_type())
    }

    #[test]
    fn water_makes_the_base_and_the_base_the_potions() {
        let base = distilled(ItemId::Mushroom, &plain(ItemId::WaterVial)).unwrap();
        assert_eq!(base, plain(ItemId::NeutralBase));
        let healing = distilled(ItemId::Apple, &base).unwrap();
        assert_eq!(healing, plain(ItemId::HealingPotion));
        assert_eq!(distilled(ItemId::Apple, &plain(ItemId::WaterVial)), None);
        assert_eq!(distilled(ItemId::Dirt, &base), None);
        assert_eq!(distilled(ItemId::Mushroom, &plain(ItemId::GlassVial)), None);
        for (vial, with, result) in RECIPES {
            assert!(
                vial.is_vial_filled() && result.is_vial_filled(),
                "{result:?}"
            );
            assert!(is_ingredient(with), "{with:?}");
            assert_eq!(result.get_max_stack(), 1);
        }
    }

    #[test]
    fn every_potion_is_made_and_has_a_colour() {
        for item in ItemId::ALL
            .into_iter()
            .filter(|item| item.potion().is_some())
        {
            assert_eq!(distilling_inputs(item).count(), 1, "{item:?}");
            assert!(item.potion_color().is_some(), "{item:?}");
        }
    }

    #[test]
    fn moss_lengthens_and_a_shard_strengthens_once_each() {
        let swiftness = plain(ItemId::SwiftnessPotion);
        let (kind, level, ticks) = potion_effect(&swiftness).unwrap();
        assert_eq!((kind, level), (EffectKind::Swiftness, 1));

        let longer = distilled(LONGER, &swiftness).unwrap();
        assert_eq!(potion_effect(&longer).unwrap(), (kind, 1, ticks * 2));
        assert_eq!(distilled(LONGER, &longer), None, "once");

        let both = distilled(STRONGER, &longer).unwrap();
        assert_eq!(
            both.item_type,
            ItemType::Brewed {
                longer: true,
                stronger: true
            }
        );
        assert_eq!(potion_effect(&both).unwrap(), (kind, 2, ticks));
        assert_eq!(distilled(STRONGER, &both), None, "once");

        let stronger = distilled(STRONGER, &swiftness).unwrap();
        assert_eq!(potion_effect(&stronger).unwrap(), (kind, 2, ticks / 2));
    }

    #[test]
    fn what_a_modifier_would_not_change_it_refuses() {
        let healing = plain(ItemId::HealingPotion);
        assert_eq!(distilled(LONGER, &healing), None);
        assert!(distilled(STRONGER, &healing).is_some());
        let night = plain(ItemId::NightVisionPotion);
        assert_eq!(distilled(STRONGER, &night), None);
        assert!(distilled(LONGER, &night).is_some());
        // The base is no potion yet
        assert_eq!(distilled(LONGER, &plain(ItemId::NeutralBase)), None);
    }

    #[test]
    fn a_splash_reaches_less_far_from_its_burst() {
        assert_eq!(splash_share(0.0), 1.0);
        assert_eq!(splash_share(SPLASH_RADIUS), 0.5);
        assert_eq!(splash_share(SPLASH_RADIUS + 0.1), 0.0);
        assert!(ItemId::SplashPoisonPotion.is_splash_potion());
        assert!(!ItemId::HealingPotion.is_splash_potion());
    }
}
