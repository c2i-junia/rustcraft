//! What a player has achieved in a world, kept in its save. Only the server changes it.

use serde::{Deserialize, Serialize};

use super::Player;
use crate::world::{depths::DepthTier, hostiles::is_night, ItemId, MobKind};
use crate::{DAY_DURATION_IN_TICKS, TICKS_PER_SECOND};

/// A night counts for a player there this long after it fell, at most.
const NIGHTFALL_TICKS: u64 = 60 * TICKS_PER_SECOND;

/// What a player achieves on its way down. Append only: it is saved and sent. `ALL` gives the
/// order they are shown in.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Achievement {
    Tool,
    CraftingTable,
    Furnace,
    Night,
    Bed,
    Hostile,
    Bow,
    Armour,
    FullSuit,
    Iron,
    Diamond,
    Trade,
    Animal,
    Dungeon,
    Caves,
    Depths,
    FurnaceTier,
    Enchantment,
    Compass,
    Lair,
    Boss,
}

impl Achievement {
    pub const ALL: [Achievement; 21] = [
        Self::Tool,
        Self::CraftingTable,
        Self::Furnace,
        Self::Night,
        Self::Bed,
        Self::Hostile,
        Self::Bow,
        Self::Armour,
        Self::FullSuit,
        Self::Iron,
        Self::Trade,
        Self::Animal,
        Self::Caves,
        Self::Dungeon,
        Self::Depths,
        Self::Diamond,
        Self::Enchantment,
        Self::FurnaceTier,
        Self::Compass,
        Self::Lair,
        Self::Boss,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tool => "Better Than Bare Hands",
            Self::CraftingTable => "Room to Work",
            Self::Furnace => "A Fire That Serves",
            Self::Night => "Still Here at Dawn",
            Self::Bed => "Somewhere to Come Back To",
            Self::Hostile => "Holding Your Ground",
            Self::Bow => "From a Safe Distance",
            Self::Armour => "A Second Skin",
            Self::FullSuit => "Dressed for the Descent",
            Self::Iron => "Worth the Climb Down",
            Self::Diamond => "Cold Light",
            Self::Trade => "A Fair Price",
            Self::Animal => "Good Company",
            Self::Dungeon => "Someone Built This",
            Self::Caves => "Under the Roots",
            Self::Depths => "Where the Sun Is a Story",
            Self::FurnaceTier => "The Shores of Fire",
            Self::Enchantment => "More Than Metal",
            Self::Compass => "It Points Down",
            Self::Lair => "At Her Door",
            Self::Boss => "The Last Ember",
        }
    }

    /// What earns it.
    pub fn goal(self) -> &'static str {
        match self {
            Self::Tool => "Hold your first tool",
            Self::CraftingTable => "Get a crafting table",
            Self::Furnace => "Get a furnace",
            Self::Night => "Live through a whole night",
            Self::Bed => "Lie on a bedroll",
            Self::Hostile => "Beat a hostile creature",
            Self::Bow => "Get a bow",
            Self::Armour => "Wear a piece of armour",
            Self::FullSuit => "Wear four pieces of armour",
            Self::Iron => "Get an iron ingot",
            Self::Diamond => "Get a diamond",
            Self::Trade => "Trade with a villager",
            Self::Animal => "Tame an animal, or have two breed",
            Self::Dungeon => "Set foot in a dungeon",
            Self::Caves => "Go down to the Caves",
            Self::Depths => "Go down to the Depths",
            Self::FurnaceTier => "Go down to the Furnace",
            Self::Enchantment => "Enchant an item",
            Self::Compass => "Get a depths compass",
            Self::Lair => "Enter the lair of the Ember Mother",
            Self::Boss => "Be in the lair as the Ember Mother falls",
        }
    }

    /// The item it is shown as.
    pub fn icon(self) -> ItemId {
        match self {
            Self::Tool => ItemId::WoodenPickaxe,
            Self::CraftingTable => ItemId::CraftingTable,
            Self::Furnace => ItemId::Furnace,
            Self::Night => ItemId::Torch,
            Self::Bed => ItemId::Bedroll,
            Self::Hostile => ItemId::StoneSword,
            Self::Bow => ItemId::Bow,
            Self::Armour => ItemId::LeatherChestplate,
            Self::FullSuit => ItemId::IronChestplate,
            Self::Iron => ItemId::IronIngot,
            Self::Diamond => ItemId::Diamond,
            Self::Trade => ItemId::Coin,
            Self::Animal => ItemId::Wool,
            Self::Dungeon => ItemId::HewnStone,
            Self::Caves => ItemId::Cobblestone,
            Self::Depths => ItemId::Slate,
            Self::FurnaceTier => ItemId::LavaBucket,
            Self::Enchantment => ItemId::DiamondSword,
            Self::Compass => ItemId::DepthsCompass,
            Self::Lair => ItemId::Obsidian,
            Self::Boss => ItemId::EmberHeart,
        }
    }

    pub fn of_tier(tier: DepthTier) -> Self {
        match tier {
            DepthTier::Caves => Self::Caves,
            DepthTier::Depths => Self::Depths,
            DepthTier::Furnace => Self::FurnaceTier,
        }
    }
}

/// What a player did in a world. All but the play time counts survival only.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Stats {
    pub play_ticks: u64,
    /// The lowest cell its feet were in, `None` until it was above the void.
    pub lowest: Option<i32>,
    pub blocks_mined: u64,
    pub creatures_beaten: u64,
    pub deaths: u64,
}

impl Stats {
    pub const LABELS: [&'static str; 5] = [
        "Play time",
        "Greatest depth",
        "Blocks mined",
        "Creatures beaten",
        "Deaths",
    ];

    /// What each of them reads, in the order of `LABELS`.
    pub fn values(&self) -> [String; 5] {
        let minutes = self.play_ticks / TICKS_PER_SECOND / 60;
        let time = match minutes / 60 {
            0 => format!("{minutes} min"),
            hours => format!("{hours} h {:02} min", minutes % 60),
        };
        let depth = match self.lowest {
            None => "-".to_string(),
            // As the minimap writes it
            Some(y) => format!("Y {y}"),
        };
        [
            time,
            depth,
            self.blocks_mined.to_string(),
            self.creatures_beaten.to_string(),
            self.deaths.to_string(),
        ]
    }
}

/// What a player has achieved in a world.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Progress {
    /// In the order they were earned.
    pub achievements: Vec<Achievement>,
    /// The night the player has been alive in since it fell, the first of the world being 0.
    pub night: Option<u64>,
    pub stats: Stats,
}

impl Progress {
    pub fn has(&self, achievement: Achievement) -> bool {
        self.achievements.contains(&achievement)
    }
}

impl Player {
    /// The player earns `achievement`, unless it has it. Nothing is earned in creative.
    pub fn earn(&mut self, achievement: Achievement) {
        if self.game_mode.takes_damage() && !self.progress.has(achievement) {
            self.progress.achievements.push(achievement);
        }
    }

    /// The player beat a creature of `kind`.
    pub fn beat(&mut self, kind: MobKind) {
        if self.game_mode.takes_damage() {
            self.progress.stats.creatures_beaten += 1;
        }
        if kind.is_hostile() {
            self.earn(Achievement::Hostile);
        }
        if kind.is_boss() {
            self.earn(Achievement::Boss);
        }
    }

    /// The player broke `blocks` blocks.
    pub fn mined(&mut self, blocks: usize) {
        if self.game_mode.takes_damage() {
            self.progress.stats.blocks_mined += blocks as u64;
        }
    }

    /// Counts a tick of the world the player is in.
    pub fn count_tick(&mut self) {
        let stats = &mut self.progress.stats;
        stats.play_ticks += 1;
        let feet = (self.position.y - self.height / 2.0 + 0.01).floor() as i32;
        // The void under the world is no depth
        if feet >= 0 && self.game_mode.takes_damage() && self.health > 0 {
            stats.lowest = Some(stats.lowest.map_or(feet, |lowest| lowest.min(feet)));
        }
    }

    /// Earns what the player holds and wears.
    pub fn earn_possessions(&mut self) {
        if !self.game_mode.takes_damage() {
            return;
        }
        let worn = self.inventory.worn().0.iter().flatten().count();
        if worn > 0 {
            self.earn(Achievement::Armour);
        }
        if worn == 4 {
            self.earn(Achievement::FullSuit);
        }
        let stacks = self
            .inventory
            .inner
            .values()
            .chain(&self.inventory.floating);
        let held: Vec<ItemId> = stacks.map(|stack| stack.item_id).collect();
        for item in held {
            let achievement = match item {
                ItemId::CraftingTable => Achievement::CraftingTable,
                ItemId::Furnace => Achievement::Furnace,
                ItemId::Bow => Achievement::Bow,
                ItemId::IronIngot => Achievement::Iron,
                ItemId::Diamond => Achievement::Diamond,
                ItemId::DepthsCompass => Achievement::Compass,
                item if item.tool().is_some() => Achievement::Tool,
                _ => continue,
            };
            self.earn(achievement);
        }
    }

    /// Follows the night the player lives through, from its fall to its dawn, at the tick `tick`
    /// of the world.
    pub fn watch_the_night(&mut self, tick: u64) {
        if self.is_dead() || !self.game_mode.takes_damage() {
            self.progress.night = None;
            return;
        }
        // A night is the first half of a day
        let (day, since) = (tick / DAY_DURATION_IN_TICKS, tick % DAY_DURATION_IN_TICKS);
        if is_night(tick) && since < NIGHTFALL_TICKS {
            self.progress.night = Some(day);
        } else if self.progress.night.is_some_and(|night| night != day) {
            self.progress.night = None;
        } else if !is_night(tick) && self.progress.night.take().is_some() {
            self.earn(Achievement::Night);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::GameMode;
    use crate::world::ItemStack;

    #[test]
    fn all_lists_every_achievement_once() {
        for (i, achievement) in Achievement::ALL.iter().enumerate() {
            assert!((*achievement as usize) < Achievement::ALL.len());
            assert!(!Achievement::ALL[..i].contains(achievement));
        }
    }

    #[test]
    fn the_statistics_count_survival_only_but_for_the_time() {
        use crate::players::game_mode::DamageSource;
        let mut player = Player {
            game_mode: GameMode::Creative,
            ..Default::default()
        };
        player.position.y = 12.9;
        player.count_tick();
        player.mined(3);
        player.beat(MobKind::Cow);
        let time = |play_ticks| Stats {
            play_ticks,
            ..Default::default()
        };
        assert_eq!(player.progress.stats, time(1));

        player.game_mode = GameMode::Survival;
        player.count_tick();
        player.position.y = 70.9;
        player.count_tick();
        player.position.y = -5.0;
        player.count_tick();
        player.mined(3);
        player.beat(MobKind::Cow);
        player.hurt(1000, DamageSource::Fall);
        player.hurt(1000, DamageSource::Fall);
        let stats = Stats {
            lowest: Some(12),
            blocks_mined: 3,
            creatures_beaten: 1,
            deaths: 1,
            ..time(4)
        };
        assert_eq!(player.progress.stats, stats);
        assert_eq!(
            player.progress.stats.values(),
            ["0 min", "Y 12", "3", "1", "1"]
        );
        assert_eq!(time(20 * 60 * 125).values()[0], "2 h 05 min");
    }

    const DAWN: u64 = DAY_DURATION_IN_TICKS / 2;

    #[test]
    fn possessions_and_nights_are_earned_once_and_never_in_creative() {
        let mut player = Player::default();
        let item_id = ItemId::IronPickaxe;
        player.inventory.inner.insert(
            3,
            ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb: 1,
            },
        );
        player.game_mode = GameMode::Creative;
        player.earn_possessions();
        player.watch_the_night(0);
        player.watch_the_night(DAWN);
        assert_eq!(player.progress, Progress::default());

        player.game_mode = GameMode::Survival;
        player.earn_possessions();
        player.earn_possessions();
        // A night joined late does not count
        player.watch_the_night(DAWN - 1);
        player.watch_the_night(DAWN);
        assert_eq!(player.progress.achievements, [Achievement::Tool]);
        for since in [0, DAWN - 1, DAWN] {
            player.watch_the_night(DAY_DURATION_IN_TICKS + since);
        }
        assert_eq!(
            player.progress.achievements,
            [Achievement::Tool, Achievement::Night]
        );
    }
}
