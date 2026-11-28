//! What the chests and barrels of the structures hold, as the generation of the world fills
//! them and the Codex shows them: each item with its chance, its fewest and its most, each line
//! rolled on its own. A new item is added at the end of a table, as the draws of an entry come
//! from its rank.

use super::ItemId;

/// An item a container may hold: its chance, its fewest and its most.
pub type LootEntry = (ItemId, f32, u32, u32);

/// The forge of a village.
pub const FORGE: &[LootEntry] = &[
    (ItemId::BronzeIngot, 0.7, 1, 4),
    (ItemId::CopperIngot, 0.5, 1, 5),
    (ItemId::TinIngot, 0.4, 1, 3),
    (ItemId::IronIngot, 0.25, 1, 3),
    (ItemId::Coal, 0.6, 2, 8),
    (ItemId::BronzePickaxe, 0.15, 1, 1),
    (ItemId::BronzeSword, 0.15, 1, 1),
    (ItemId::BronzeAxe, 0.1, 1, 1),
    (ItemId::Flint, 0.3, 1, 3),
];

/// The granary of a village.
pub const GRANARY: &[LootEntry] = &[
    (ItemId::Rye, 0.9, 4, 16),
    (ItemId::RyeBread, 0.6, 1, 4),
    (ItemId::Berries, 0.4, 2, 8),
    (ItemId::Fiber, 0.3, 2, 6),
];

/// The houses and the other buildings of a village.
pub const HOUSE: &[LootEntry] = &[
    (ItemId::RyeBread, 0.6, 1, 3),
    (ItemId::Berries, 0.4, 1, 5),
    (ItemId::Torch, 0.4, 2, 6),
    (ItemId::Stick, 0.3, 2, 6),
    (ItemId::Wool, 0.2, 1, 3),
    (ItemId::Rope, 0.2, 1, 2),
    (ItemId::WoodenHoe, 0.1, 1, 1),
];

/// A ruin, on the surface.
pub const RUIN: &[LootEntry] = &[
    (ItemId::RyeBread, 0.6, 1, 3),
    (ItemId::Torch, 0.6, 2, 8),
    (ItemId::Rope, 0.4, 1, 3),
    (ItemId::Coal, 0.5, 2, 6),
    (ItemId::CopperIngot, 0.5, 1, 4),
    (ItemId::TinIngot, 0.3, 1, 3),
    (ItemId::BronzeIngot, 0.3, 1, 3),
    (ItemId::BronzeSword, 0.15, 1, 1),
    (ItemId::BronzePickaxe, 0.1, 1, 1),
    (ItemId::Spear, 0.15, 1, 1),
    (ItemId::FlintAndSteel, 0.1, 1, 1),
    (ItemId::Arrow, 0.35, 2, 8),
    (ItemId::Bow, 0.12, 1, 1),
    (ItemId::LeatherHelmet, 0.1, 1, 1),
    (ItemId::LeatherBoots, 0.1, 1, 1),
    (ItemId::BronzeHelmet, 0.05, 1, 1),
    (ItemId::Saddle, 0.15, 1, 1),
    (ItemId::LeatherHorseArmour, 0.1, 1, 1),
    (ItemId::IronHorseArmour, 0.05, 1, 1),
    (ItemId::Lead, 0.15, 1, 2),
];

/// A crypt, off a cave.
pub const CRYPT: &[LootEntry] = &[
    (ItemId::Torch, 0.5, 4, 10),
    (ItemId::RyeBread, 0.4, 1, 3),
    (ItemId::Coal, 0.5, 3, 10),
    (ItemId::BronzeIngot, 0.5, 2, 5),
    (ItemId::IronIngot, 0.7, 2, 6),
    (ItemId::GoldIngot, 0.3, 1, 3),
    (ItemId::IronSword, 0.2, 1, 1),
    (ItemId::IronPickaxe, 0.15, 1, 1),
    (ItemId::IronAxe, 0.1, 1, 1),
    (ItemId::Lantern, 0.2, 1, 2),
    (ItemId::Diamond, 0.06, 1, 1),
    (ItemId::Arrow, 0.5, 4, 12),
    (ItemId::Bow, 0.2, 1, 1),
    (ItemId::BronzeChestplate, 0.12, 1, 1),
    (ItemId::IronHelmet, 0.1, 1, 1),
    (ItemId::IronBoots, 0.1, 1, 1),
    (ItemId::Saddle, 0.12, 1, 1),
    (ItemId::IronHorseArmour, 0.1, 1, 1),
    (ItemId::GoldHorseArmour, 0.06, 1, 1),
    (ItemId::Lead, 0.1, 1, 1),
];

/// A sanctuary, by the lava: the richest.
pub const SANCTUARY: &[LootEntry] = &[
    (ItemId::IronIngot, 0.8, 3, 8),
    (ItemId::GoldIngot, 0.8, 2, 6),
    (ItemId::Diamond, 0.55, 1, 3),
    (ItemId::GoldenSword, 0.15, 1, 1),
    (ItemId::DiamondSword, 0.12, 1, 1),
    (ItemId::DiamondPickaxe, 0.12, 1, 1),
    (ItemId::DiamondAxe, 0.08, 1, 1),
    (ItemId::Obsidian, 0.4, 2, 6),
    (ItemId::GoldBlock, 0.1, 1, 1),
    (ItemId::RoastBeef, 0.5, 1, 4),
    (ItemId::Arrow, 0.6, 8, 20),
    (ItemId::Bow, 0.25, 1, 1),
    (ItemId::IronChestplate, 0.15, 1, 1),
    (ItemId::IronLeggings, 0.15, 1, 1),
    (ItemId::DiamondHelmet, 0.06, 1, 1),
    (ItemId::DiamondBoots, 0.06, 1, 1),
];

/// The chest of a shipwreck, besides its treasure map.
pub const SHIPWRECK: &[LootEntry] = &[
    (ItemId::Coal, 0.5, 2, 8),
    (ItemId::IronIngot, 0.45, 1, 5),
    (ItemId::GoldIngot, 0.25, 1, 3),
    (ItemId::CopperIngot, 0.4, 2, 6),
    (ItemId::Rope, 0.4, 1, 4),
    (ItemId::Paper, 0.4, 2, 6),
    (ItemId::Leather, 0.3, 1, 3),
    (ItemId::RyeBread, 0.4, 1, 3),
    (ItemId::Coin, 0.5, 2, 9),
    (ItemId::Compass, 0.1, 1, 1),
    (ItemId::Saddle, 0.08, 1, 1),
];

/// The chest buried where the map of a shipwreck leads.
pub const BURIED_TREASURE: &[LootEntry] = &[
    (ItemId::GoldIngot, 0.9, 3, 9),
    (ItemId::Diamond, 0.6, 1, 3),
    (ItemId::IronIngot, 0.7, 3, 8),
    (ItemId::Coin, 0.9, 8, 24),
    (ItemId::GoldBlock, 0.15, 1, 1),
    (ItemId::DiamondSword, 0.08, 1, 1),
    (ItemId::CrystalShard, 0.3, 2, 6),
];
