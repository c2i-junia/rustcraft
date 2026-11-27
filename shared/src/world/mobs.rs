use bevy::math::{IVec3, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::messages::PlayerId;
use crate::players::bow::BowShot;
use crate::world::depths::{strengthened, DepthTier};
use crate::world::{ItemId, PortalRest, ToolKind, Villager};
use crate::TICKS_PER_SECOND;

pub type MobId = u128;

/// Append only: the variants are sent by index (see `ENCODING_WARNING`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MobKind {
    Fox,
    Sheep,
    Cow,
    Pig,
    Chicken,
    Rabbit,
    Wolf,
    Horse,
    Goat,
    Cat,
    /// A cave rat, in packs.
    Gnawer,
    /// A bush that bites who comes close at night.
    Bramble,
    /// A boulder that winds up and rolls at its prey.
    Roller,
    /// A walking stump, out of the grass at night, back into the ground at dawn.
    Stumpling,
    /// A drowned thing that swims at its prey in deep water and drags it down.
    Drowner,
    /// A burning rock that hovers in the deep caves and throws embers.
    Cinderling,
    /// A Cinderling's ember in flight: a projectile, neither hostile nor an animal.
    Ember,
    /// An owl of the peaks that circles over its prey at night and dives at it.
    GustOwl,
    /// An arrow in flight or stuck in a block: a projectile (`ArrowFlight`).
    Arrow,
    /// The villagers (`is_villager`): see `trade`.
    Farmer,
    Smith,
    Cartographer,
    /// The mother of the Cinderlings, asleep in her lair (`crate::world::lair`): the boss, which
    /// the server moves apart from the other hostiles.
    EmberMother,
    /// A boat of planks (`crate::players::riding`): a vehicle, neither an animal nor hostile.
    Boat,
    /// The float of a fishing rod (`crate::world::fishing`): a projectile, which plunges
    /// (`MobAction::Attack`) when a fish bites.
    Bobber,
    /// Small bright fish of the reefs, in shoals (`is_aquatic`).
    Glintfin,
    /// Silver fish of the open sea, in shoals (`is_aquatic`).
    Shoalback,
    /// A slow shelled grazer of the damp caves, its back grown with moss.
    Mossback,
    /// A crab of crystal that guards the crystal caverns.
    Shardling,
    /// A crawler of slag of the basalt galleries, whose bite sets the player burning.
    Slagcrawler,
    /// A steed of the plains, slower than the horse and steady, which carries a chest
    /// (`ServerMob::pack`).
    Donkey,
    /// Born of a horse and a donkey: it carries a chest too, and never breeds.
    Mule,
    /// A splash potion in flight (`ServerMob::potion`): a projectile, which bursts where it
    /// lands (`crate::world::alchemy`).
    ThrownPotion,
}

impl MobKind {
    pub const ALL: [MobKind; 33] = [
        MobKind::Fox,
        MobKind::Sheep,
        MobKind::Cow,
        MobKind::Pig,
        MobKind::Chicken,
        MobKind::Rabbit,
        MobKind::Wolf,
        MobKind::Horse,
        MobKind::Goat,
        MobKind::Cat,
        MobKind::Gnawer,
        MobKind::Bramble,
        MobKind::Roller,
        MobKind::Stumpling,
        MobKind::Drowner,
        MobKind::Cinderling,
        MobKind::Ember,
        MobKind::GustOwl,
        MobKind::Arrow,
        MobKind::Farmer,
        MobKind::Smith,
        MobKind::Cartographer,
        MobKind::EmberMother,
        MobKind::Boat,
        MobKind::Bobber,
        MobKind::Glintfin,
        MobKind::Shoalback,
        MobKind::Mossback,
        MobKind::Shardling,
        MobKind::Slagcrawler,
        MobKind::Donkey,
        MobKind::Mule,
        MobKind::ThrownPotion,
    ];

    /// Whether mobs of this kind hunt the players (`crate::world::hostiles`), and appear in the
    /// dark rather than with the chunks.
    pub fn is_hostile(self) -> bool {
        matches!(
            self,
            MobKind::Gnawer
                | MobKind::Bramble
                | MobKind::Roller
                | MobKind::Stumpling
                | MobKind::Drowner
                | MobKind::Cinderling
                | MobKind::GustOwl
                | MobKind::EmberMother
                | MobKind::Shardling
                | MobKind::Slagcrawler
        )
    }

    /// Whether mobs of this kind live in the water, which they swim in and do not leave: the
    /// fish (`server::mob::swim`).
    pub fn is_aquatic(self) -> bool {
        matches!(self, MobKind::Glintfin | MobKind::Shoalback)
    }

    /// Whether this kind is a boss: hostile, but neither born, pushed, removed nor saved with the
    /// others.
    pub fn is_boss(self) -> bool {
        self == MobKind::EmberMother
    }

    /// Whether mobs of this kind parry the blows from their front, as with a shield: the
    /// Stumpling, behind its thick bark, which an axe knocks aside for a while.
    pub fn guards(self) -> bool {
        self == MobKind::Stumpling
    }

    /// Whether mobs of this kind are projectiles the server flies (`MobKind::Ember`, `MobKind::Arrow`): they do
    /// not walk, are not pushed nor struck, are not counted with the hostiles, and are not
    /// saved.
    pub fn is_projectile(self) -> bool {
        matches!(self, MobKind::Ember | MobKind::Arrow | MobKind::Bobber | MobKind::ThrownPotion)
    }

    /// Whether mobs of this kind are villagers: they trade (`trade::offers`), live in a house,
    /// and nothing hurts them.
    pub fn is_villager(self) -> bool {
        matches!(
            self,
            MobKind::Farmer | MobKind::Smith | MobKind::Cartographer
        )
    }

    /// Whether mobs of this kind are vehicles, which players ride and break into an item: the
    /// boat. They neither live nor are born.
    pub fn is_vehicle(self) -> bool {
        self == MobKind::Boat
    }

    /// Whether players ride mobs of this kind (`crate::players::riding`): the horse, saddled and
    /// tamed, and the boat.
    pub fn is_mount(self) -> bool {
        self.is_steed() || self == MobKind::Boat
    }

    /// Whether mobs of this kind are steeds, ridden saddled once tamed (`crate::players::riding`):
    /// the horse, the donkey and the mule. They are born with their gifts (`HorseTraits`).
    pub fn is_steed(self) -> bool {
        matches!(self, MobKind::Horse | MobKind::Donkey | MobKind::Mule)
    }

    /// Whether a tamed mob of this kind carries a chest on its back (`ServerMob::pack`): the
    /// donkey and the mule.
    pub fn carries_chest(self) -> bool {
        matches!(self, MobKind::Donkey | MobKind::Mule)
    }

    /// Whether a tamed mob of this kind sits and stands up when its owner tells it to with a
    /// right click: the wolf and the cat.
    pub fn sits(self) -> bool {
        self.follows_owner()
    }

    /// Whether a lead ties mobs of this kind (`ServerMob::leash`): the animals but the fish.
    pub fn is_leashable(self) -> bool {
        self.is_animal() && !self.is_aquatic()
    }

    /// The points of health of a mob of this kind.
    pub fn health(self) -> u32 {
        match self {
            // A few blows break it
            MobKind::Boat => 4,
            MobKind::Fox | MobKind::Pig | MobKind::Goat | MobKind::Cat => 10,
            MobKind::Gnawer => 6,
            MobKind::Bramble => 12,
            MobKind::Roller => 16,
            MobKind::Stumpling => 24,
            MobKind::Drowner => 18,
            MobKind::Cinderling => 10,
            MobKind::GustOwl => 14,
            // Alone against her; more with the players (`ServerMob::extra_health`)
            MobKind::EmberMother => 200,
            // Never struck
            MobKind::Ember | MobKind::Arrow | MobKind::Bobber | MobKind::ThrownPotion => 1,
            MobKind::Farmer | MobKind::Smith | MobKind::Cartographer => 20,
            MobKind::Cow => 12,
            MobKind::Wolf | MobKind::Horse => 16,
            MobKind::Donkey => 15,
            MobKind::Mule => 18,
            MobKind::Chicken | MobKind::Rabbit => 4,
            MobKind::Sheep => 8,
            MobKind::Glintfin | MobKind::Shoalback => 3,
            MobKind::Mossback => 14,
            MobKind::Shardling => 12,
            MobKind::Slagcrawler => 14,
        }
    }

    /// Whether an axe deals this kind half again its damage (`blow_damage`): the Stumpling, a
    /// walking stump.
    pub fn weak_to_axes(self) -> bool {
        self == MobKind::Stumpling
    }

    /// The points of health a blow with `held` in hand takes from a mob of this kind: the
    /// weapon's (`ItemId::attack_damage`), 1 bare-handed, and half again with an axe against a
    /// kind `weak_to_axes`, rounded down.
    pub fn blow_damage(self, held: Option<ItemId>) -> u32 {
        let damage = held.map_or(1, |item| item.attack_damage());
        let axe = held
            .and_then(|item| item.tool())
            .is_some_and(|tool| tool.kind == ToolKind::Axe);
        if axe && self.weak_to_axes() {
            damage * 3 / 2
        } else {
            damage
        }
    }

    /// Whether mobs of this kind are animals: neither hostile, nor projectiles, nor villagers.
    pub fn is_animal(self) -> bool {
        !self.is_hostile() && !self.is_projectile() && !self.is_villager() && !self.is_vehicle()
    }

    /// Whether a player tames mobs of this kind by feeding them (`MobKind::eats`): the wolf,
    /// the cat and the horse. Only tamed ones breed, and a tamed horse takes a saddle.
    pub fn is_tameable(self) -> bool {
        matches!(self, MobKind::Wolf | MobKind::Cat) || self.is_steed()
    }

    /// Whether a tamed mob of this kind follows its owner: the wolf and the cat, not the horse,
    /// which waits where it was left.
    pub fn follows_owner(self) -> bool {
        matches!(self, MobKind::Wolf | MobKind::Cat)
    }

    /// Whether mobs of this kind eat `item` from a player's hand: fed to an adult, it makes it
    /// want a mate (`LOVE_TICKS`), fed to a young one it grows it up faster, and fed to a wolf
    /// or a cat not yet tamed it may tame it (`TAME_CHANCE`).
    pub fn eats(self, item: ItemId) -> bool {
        use ItemId as I;
        match self {
            MobKind::Cow | MobKind::Sheep | MobKind::Goat | MobKind::Chicken => item == I::Rye,
            MobKind::Horse | MobKind::Donkey | MobKind::Mule => {
                matches!(item, I::Carrot | I::Rye | I::Apple)
            }
            MobKind::Pig => matches!(item, I::Carrot | I::Potato),
            MobKind::Rabbit => item == I::Carrot,
            MobKind::Fox => item == I::Berries,
            MobKind::Wolf => matches!(
                item,
                I::RawMutton
                    | I::RawPork
                    | I::RawBeef
                    | I::RawChicken
                    | I::RawRabbit
                    | I::RoastMutton
                    | I::RoastPork
                    | I::RoastBeef
                    | I::RoastChicken
                    | I::RoastRabbit
            ),
            MobKind::Cat => matches!(item, I::RawChicken | I::RawRabbit),
            _ => false,
        }
    }

    /// The name the player reads, in the death messages.
    pub fn display_name(self) -> &'static str {
        match self {
            MobKind::Fox => "Fox",
            MobKind::Sheep => "Sheep",
            MobKind::Cow => "Cow",
            MobKind::Pig => "Pig",
            MobKind::Chicken => "Chicken",
            MobKind::Rabbit => "Rabbit",
            MobKind::Wolf => "Wolf",
            MobKind::Horse => "Horse",
            MobKind::Goat => "Goat",
            MobKind::Cat => "Cat",
            MobKind::Gnawer => "Gnawer",
            MobKind::Bramble => "Bramble",
            MobKind::Roller => "Roller",
            MobKind::Stumpling => "Stumpling",
            MobKind::Drowner => "Drowner",
            MobKind::Cinderling => "Cinderling",
            MobKind::Ember => "Ember",
            MobKind::GustOwl => "Gust Owl",
            MobKind::Arrow => "Arrow",
            MobKind::Farmer => "Farmer",
            MobKind::Smith => "Smith",
            MobKind::Cartographer => "Cartographer",
            MobKind::EmberMother => "Ember Mother",
            MobKind::Boat => "Boat",
            MobKind::Bobber => "Bobber",
            MobKind::Glintfin => "Glintfin",
            MobKind::Shoalback => "Shoalback",
            MobKind::Mossback => "Mossback",
            MobKind::Shardling => "Shardling",
            MobKind::Slagcrawler => "Slagcrawler",
            MobKind::Donkey => "Donkey",
            MobKind::Mule => "Mule",
            MobKind::ThrownPotion => "Splash Potion",
        }
    }

    /// Width, height and depth of the hitbox, in blocks. It does not turn with the mob, so it
    /// is as wide as it is deep.
    pub fn size(self) -> Vec3 {
        match self {
            MobKind::Fox => Vec3::new(0.7, 0.7, 0.7),
            MobKind::Sheep => Vec3::new(0.9, 1.25, 0.9),
            MobKind::Cow => Vec3::new(0.9, 1.4, 0.9),
            MobKind::Pig => Vec3::new(0.9, 0.9, 0.9),
            MobKind::Chicken => Vec3::new(0.5, 0.8, 0.5),
            MobKind::Rabbit => Vec3::new(0.4, 0.6, 0.4),
            MobKind::Wolf => Vec3::new(0.7, 0.95, 0.7),
            MobKind::Horse => Vec3::new(1.2, 1.7, 1.2),
            MobKind::Goat => Vec3::new(0.9, 1.3, 0.9),
            MobKind::Cat => Vec3::new(0.6, 0.7, 0.6),
            // The models of `scripts/generate_hostiles.py`: the Gnawer is 0.375 wide and
            // 0.625 tall (its ears), the spec's 0.6 wide; the Bramble 1.0 × 0.75, the Roller
            // 0.75 × 0.75
            MobKind::Gnawer => Vec3::new(0.6, 0.6, 0.6),
            MobKind::Bramble => Vec3::new(1.0, 0.75, 1.0),
            MobKind::Roller => Vec3::new(0.75, 0.75, 0.75),
            // 1.125 wide with its arms, 2.06 tall: its box fits the two free cells of a spawn
            MobKind::Stumpling => Vec3::new(0.9, 1.95, 0.9),
            // 1.0 wide with its arms, 1.56 tall
            MobKind::Drowner => Vec3::new(0.8, 1.5, 0.8),
            // Its body 0.625 wide (1.125 with its orbiting embers), 1.75 tall with its flame
            MobKind::Cinderling => Vec3::new(0.7, 1.6, 0.7),
            // 2 voxels, 0.125
            MobKind::Ember => Vec3::new(0.25, 0.25, 0.25),
            // Its shaft is 14 voxels long, 1 thick: it is hit as a point grown a little
            MobKind::Arrow => Vec3::new(0.25, 0.25, 0.25),
            // 2.5 wide with its wings spread; from its talons, 0.4375 above the origin of its
            // model (`client::mob::models::model_lift`), to the top of its head, 1.875
            MobKind::GustOwl => Vec3::new(0.7, 1.45, 0.7),
            // The models of `scripts/generate_villagers.py`, 1.0 wide with their
            // hands, 1.56 tall without a hat
            MobKind::Farmer | MobKind::Smith | MobKind::Cartographer => Vec3::new(0.7, 1.6, 0.7),
            // Her body 1.5 wide, 3.1 tall with her crown of flames
            MobKind::EmberMother => Vec3::new(1.9, 3.0, 1.9),
            // The hull of `scripts/generate_boat.py`, 1.25 wide and 2 long, 0.5 high
            MobKind::Boat => Vec3::new(1.4, 0.6, 1.4),
            // `scripts/generate_bobber.py`: 4 voxels wide, 6 tall
            MobKind::Bobber => Vec3::new(0.25, 0.375, 0.25),
            // The models of `scripts/generate_caverns.py`
            MobKind::Glintfin => Vec3::new(0.4, 0.35, 0.4),
            MobKind::Shoalback => Vec3::new(0.5, 0.35, 0.5),
            MobKind::Mossback => Vec3::new(0.9, 0.7, 0.9),
            MobKind::Shardling => Vec3::new(0.8, 0.6, 0.8),
            MobKind::Slagcrawler => Vec3::new(0.9, 0.6, 0.9),
            // `scripts/generate_animals.py`: a smaller horse, the mule between the two
            MobKind::Donkey => Vec3::new(1.0, 1.4, 1.0),
            MobKind::Mule => Vec3::new(1.1, 1.55, 1.1),
            MobKind::ThrownPotion => Vec3::new(0.25, 0.25, 0.25),
        }
    }
}

/// The coat of a horse, which goes with its breed: some are faster, some last longer at a
/// gallop (`Coat::base`). Append only: sent by index.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Coat {
    Bay,
    Chestnut,
    Black,
    Grey,
    White,
    Dun,
}

impl Coat {
    pub const ALL: [Coat; 6] = [
        Coat::Bay,
        Coat::Chestnut,
        Coat::Black,
        Coat::Grey,
        Coat::White,
        Coat::Dun,
    ];

    /// The speed, in blocks/s, and the endurance, in seconds of gallop, a horse of this coat has
    /// on average: the dark ones are fast and short of breath, the pale ones slower and enduring.
    pub fn base(self) -> (f32, f32) {
        match self {
            Coat::Bay => (8.5, 8.0),
            Coat::Chestnut => (9.5, 6.5),
            Coat::Black => (10.0, 5.0),
            Coat::Grey => (8.0, 11.0),
            Coat::White => (9.0, 8.5),
            Coat::Dun => (7.5, 13.0),
        }
    }

    /// The key of its name in the translations.
    pub fn key(self) -> &'static str {
        match self {
            Coat::Bay => "horse.coat.Bay",
            Coat::Chestnut => "horse.coat.Chestnut",
            Coat::Black => "horse.coat.Black",
            Coat::Grey => "horse.coat.Grey",
            Coat::White => "horse.coat.White",
            Coat::Dun => "horse.coat.Dun",
        }
    }
}

/// The slowest and the fastest a horse walks, in blocks/s.
pub const HORSE_SPEED: std::ops::RangeInclusive<f32> = 6.0..=12.0;
/// The lowest and the highest a horse jumps at full charge, in blocks.
pub const HORSE_JUMP: std::ops::RangeInclusive<f32> = 1.3..=3.2;
/// The shortest and the longest a horse gallops, in seconds.
pub const HORSE_ENDURANCE: std::ops::RangeInclusive<f32> = 3.0..=16.0;

/// What a horse is born with.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct HorseTraits {
    pub coat: Coat,
    /// How fast it walks under a rider, in blocks/s: it gallops faster (`riding::GALLOP`).
    pub speed: f32,
    /// How high it jumps at full charge, in blocks.
    pub jump: f32,
    /// How long it gallops before it is out of breath, in seconds.
    pub endurance: f32,
}

fn clamp_to(value: f32, range: &std::ops::RangeInclusive<f32>) -> f32 {
    value.clamp(*range.start(), *range.end())
}

impl HorseTraits {
    /// A horse born in the wild: a coat drawn at random, its speed and endurance about those
    /// of its coat, its jump anywhere.
    pub fn roll(rng: &mut impl rand::Rng) -> Self {
        let coat = Coat::ALL[rng.gen_range(0..Coat::ALL.len())];
        let (speed, endurance) = coat.base();
        Self {
            coat,
            speed: clamp_to(speed + rng.gen_range(-1.0..=1.0), &HORSE_SPEED),
            jump: rng.gen_range(HORSE_JUMP),
            endurance: clamp_to(endurance + rng.gen_range(-2.0..=2.0), &HORSE_ENDURANCE),
        }
    }

    /// A donkey born in the wild: slower than any horse and a poor jumper, but long of
    /// breath. Its coat is not drawn.
    pub fn donkey(rng: &mut impl rand::Rng) -> Self {
        Self {
            coat: Coat::Grey,
            speed: clamp_to(rng.gen_range(6.0..=7.0), &HORSE_SPEED),
            jump: clamp_to(rng.gen_range(1.3..=1.6), &HORSE_JUMP),
            endurance: clamp_to(rng.gen_range(11.0..=15.0), &HORSE_ENDURANCE),
        }
    }

    /// What a steed of `kind` is born with in the wild: a horse's gifts (`roll`), a donkey's,
    /// or for a mule, found only when bred, the mean of the two.
    pub fn of_kind(kind: MobKind, rng: &mut impl rand::Rng) -> Self {
        match kind {
            MobKind::Donkey => Self::donkey(rng),
            MobKind::Mule => {
                let (horse, donkey) = (Self::roll(rng), Self::donkey(rng));
                Self::inherit(&horse, &donkey, rng)
            }
            _ => Self::roll(rng),
        }
    }

    /// A foal of `a` and `b`: the coat of one of them, but now and then one of its own, and
    /// each trait the mean of theirs, give or take a little.
    pub fn inherit(a: &Self, b: &Self, rng: &mut impl rand::Rng) -> Self {
        let coat = if rng.gen_bool(0.1) {
            Coat::ALL[rng.gen_range(0..Coat::ALL.len())]
        } else if rng.gen_bool(0.5) {
            a.coat
        } else {
            b.coat
        };
        let speed = (a.speed + b.speed) / 2.0 + rng.gen_range(-0.6..=0.6);
        let jump = (a.jump + b.jump) / 2.0 + rng.gen_range(-0.25..=0.25);
        let endurance = (a.endurance + b.endurance) / 2.0 + rng.gen_range(-1.2..=1.2);
        Self {
            coat,
            speed: clamp_to(speed, &HORSE_SPEED),
            jump: clamp_to(jump, &HORSE_JUMP),
            endurance: clamp_to(endurance, &HORSE_ENDURANCE),
        }
    }
}

/// Ticks a young animal takes to grow up: 10 minutes.
pub const GROW_TICKS: u32 = 10 * 60 * TICKS_PER_SECOND as u32;
/// Ticks an adult fed its food wants a mate: 30 seconds.
pub const LOVE_TICKS: u32 = 30 * TICKS_PER_SECOND as u32;
/// Ticks after giving birth before an animal wants a mate again: 5 minutes.
pub const BREED_WAIT_TICKS: u32 = 5 * 60 * TICKS_PER_SECOND as u32;
/// The share of its time to grow up a young animal fed its food is spared.
pub const FED_YOUNG_SPEED_UP: f32 = 0.1;
/// The chance that a wolf or a cat fed its food is tamed.
pub const TAME_CHANCE: f64 = 1.0 / 3.0;
/// How big a young animal is, against an adult.
pub const YOUNG_SCALE: f32 = 0.5;

/// What the lead of a mob is tied to (`ServerMob::leash`): the hand of a player, by its name,
/// or a fence post, by its cell. Append only: sent by index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leash {
    Player(String),
    Post(IVec3),
}

/// How much horse armour of `points` spares a steed of a blow of `damage`: 4 % a point, the
/// rest rounded to the nearest point, never less than one for a blow that hurts.
pub fn armoured_damage(damage: u32, points: u32) -> u32 {
    if damage == 0 {
        return 0;
    }
    let kept = 25u32.saturating_sub(points);
    ((damage * kept + 12) / 25).max(1)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MobTarget {
    None,
    Position(Vec3),
    Player(PlayerId),
}

/// Append only: the variants are sent by index (see `ENCODING_WARNING`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MobAction {
    /// Walks to its target.
    Walk,
    /// Runs away from its target.
    Flee,
    /// Stands still.
    Idle,
    /// Stands still and eats the grass.
    Graze,
    /// A hostile goes at its prey (`MobTarget::Player`): walks, or rolls for the Roller.
    Chase,
    /// A hostile strikes its prey, or winds up to: the client plays the Attack clip once.
    Attack,
    /// A Stumpling sinks into the ground at dawn, `action_ticks` left before it is gone.
    Burrow,
}

/// The way a walking hostile follows to its prey (`server::mob::path`): the cells its feet go
/// through, the next one to reach, the cell of the prey's feet it was found for, whether it
/// gets there, and the ticks left before it is looked for again. Neither saved nor sent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    pub cells: Vec<IVec3>,
    pub next: usize,
    pub goal: IVec3,
    pub reaches: bool,
    pub wait: u32,
}

/// What the server knows of an arrow, beside its place and its velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowFlight {
    pub shooter: PlayerId,
    pub shot: BowShot,
    /// The cell of the block it is stuck in, if it is.
    pub stuck_in: Option<IVec3>,
    /// How many more creatures it goes through (`BowShot::pierce`).
    pub pierce_left: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerMob {
    pub kind: MobKind,
    pub target: MobTarget,
    pub action: MobAction,
    /// The middle of the hitbox.
    pub position: Vec3,
    pub rotation: Quat,
    pub height: f32,
    pub width: f32,
    pub on_ground: bool,
    pub velocity: Vec3,
    /// Saves written before the rename spell it `deepth`.
    #[serde(alias = "deepth")]
    pub depth: f32,
    /// Ticks left before the mob does something else. Saves from before it read 0.
    #[serde(default)]
    pub action_ticks: u32,
    /// The points of health the mob lost.
    #[serde(default)]
    pub damage: u32,
    /// Whether a sheep waits for its wool to grow back.
    #[serde(default)]
    pub sheared: bool,
    /// How many portals the mob went through, wrapping: a client that sees it change puts the
    /// mob on the other side at once, rather than easing it across the gap. Saves from before
    /// it read 0.
    #[serde(default)]
    pub passes: u32,
    /// Ticks left before a young animal grows up; 0 for an adult. Saves from before it read 0.
    #[serde(default)]
    pub growing: u32,
    /// Ticks left during which an animal fed its food wants a mate (`LOVE_TICKS`); the clients
    /// show hearts over it meanwhile.
    #[serde(default)]
    pub in_love: u32,
    /// The name of the player who tamed the animal, which follows that player.
    #[serde(default)]
    pub owner: Option<String>,
    /// How deep underground a hostile appeared, which sets its strength (`DepthTier`). Saves
    /// from before it read none.
    #[serde(default)]
    pub tier: Option<DepthTier>,
    /// Ticks left before the animal may want a mate again (`BREED_WAIT_TICKS`). Neither saved
    /// nor sent.
    #[serde(skip)]
    pub breed_wait: u32,
    /// The floor portal the mob came out of too slowly to leave, and rests on
    /// (`Portals::pass`). Not saved, as the portals.
    #[serde(skip)]
    pub portal_rest: Option<PortalRest>,
    /// Ticks left before a blow hurts the mob again, after the last one that did. Neither saved
    /// nor sent.
    #[serde(skip)]
    pub hurt_ticks: u32,
    /// Ticks left before a hostile strikes again (`MobKind::strike`). Neither saved nor sent.
    #[serde(skip)]
    pub attack_wait: u32,
    /// The points of health a boss has beyond those of its kind. Neither saved nor sent.
    #[serde(skip)]
    pub extra_health: u32,
    /// The flight of an arrow. Neither saved nor sent.
    #[serde(skip)]
    pub arrow: Option<ArrowFlight>,
    /// The way a walking hostile follows to its prey. Neither saved nor sent.
    #[serde(skip)]
    pub route: Option<Route>,
    /// The line of a float. Neither saved nor sent.
    #[serde(skip)]
    pub bobber: Option<crate::world::fishing::Bobber>,
    /// What a villager remembers; none for the other mobs. Saves from before it read none.
    #[serde(default)]
    pub villager: Option<Villager>,
    /// What made the last blow that hurt the mob special, if anything: the clients show and hear
    /// it with the hurt (`melee::Strike`).
    #[serde(default)]
    pub struck: Option<crate::players::melee::Strike>,
    /// What a horse was born with: its coat, its speed, its jump and its endurance; none for
    /// the other mobs, and for a horse of a save from before them until the server rolls them.
    #[serde(default)]
    pub horse: Option<HorseTraits>,
    /// Whether a horse wears a saddle, which lets its owner ride it.
    #[serde(default)]
    pub saddled: bool,
    /// The players on each seat of a mount (`riding::seats`), which the server keeps in step
    /// with their `Player::riding`. Neither saved nor sent: the riders' states tell it.
    #[serde(skip)]
    pub riders: [Option<PlayerId>; 2],
    /// How many blows the mob parried (a Stumpling's bark, `MobKind::guards`), wrapping: the
    /// clients hear each one.
    #[serde(default)]
    pub parries: u8,
    /// Ticks left before the guard an axe knocked down is up again. Neither saved nor sent.
    #[serde(skip)]
    pub guard_down: u32,
    /// Whether a tamed wolf or cat sits where its owner told it to, rather than following.
    #[serde(default)]
    pub sitting: bool,
    /// The horse armour a horse wears (`ItemId::horse_armour`), which the clients show and which
    /// spares it a part of its blows.
    #[serde(default)]
    pub horse_armour: Option<ItemId>,
    /// What the chest on the back of a donkey or a mule holds, once one was put on it. Saved,
    /// and sent with the mob: the clients know it carries one.
    #[serde(default)]
    pub pack: Option<Box<crate::world::Container>>,
    /// What the lead of the mob is tied to, if it wears one.
    #[serde(default)]
    pub leash: Option<Leash>,
    /// The mob a tamed wolf hunts: what its owner struck, or what goes at its owner. Neither
    /// saved nor sent.
    #[serde(skip)]
    pub prey: Option<MobId>,
    /// The splash potion a `MobKind::ThrownPotion` is, which the clients colour its burst by.
    #[serde(default)]
    pub potion: Option<ItemId>,
}

impl ServerMob {
    /// A mob of `kind` standing still, its hitbox centred on `position`.
    pub fn new(kind: MobKind, position: Vec3) -> Self {
        let size = kind.size();
        Self {
            kind,
            target: MobTarget::None,
            action: MobAction::Idle,
            position,
            rotation: Quat::IDENTITY,
            height: size.y,
            width: size.x,
            on_ground: false,
            velocity: Vec3::ZERO,
            depth: size.z,
            action_ticks: 0,
            damage: 0,
            sheared: false,
            passes: 0,
            growing: 0,
            in_love: 0,
            owner: None,
            tier: None,
            breed_wait: 0,
            portal_rest: None,
            hurt_ticks: 0,
            attack_wait: 0,
            extra_health: 0,
            arrow: None,
            route: None,
            bobber: None,
            villager: kind.is_villager().then(Villager::default),
            struck: None,
            horse: None,
            saddled: false,
            riders: [None; 2],
            parries: 0,
            guard_down: 0,
            sitting: false,
            horse_armour: None,
            pack: None,
            leash: None,
            prey: None,
            potion: None,
        }
    }

    /// A young animal of `kind` just born, half the size of an adult, its hitbox centred on
    /// `position`.
    pub fn young(kind: MobKind, position: Vec3) -> Self {
        let size = kind.size() * YOUNG_SCALE;
        Self {
            height: size.y,
            width: size.x,
            depth: size.z,
            growing: GROW_TICKS,
            ..Self::new(kind, position)
        }
    }

    /// The mob, its strength that of a hostile appeared in `tier`.
    pub fn in_tier(self, tier: Option<DepthTier>) -> Self {
        Self { tier, ..self }
    }

    /// The points of health of the mob unhurt: those of its kind, more in the deep tiers.
    pub fn max_health(&self) -> u32 {
        strengthened(self.kind.health(), self.tier) + self.extra_health
    }

    /// The points of the orb the mob leaves when a player kills it.
    pub fn experience(&self) -> u32 {
        strengthened(self.kind.experience(), self.tier)
    }

    /// Whether the mob is a young animal, not grown up yet.
    pub fn is_young(&self) -> bool {
        self.growing > 0
    }

    /// Grows a young animal up: its hitbox becomes that of an adult, standing where its feet
    /// were.
    pub fn grow_up(&mut self) {
        let size = self.kind.size();
        self.position.y += (size.y - self.height) / 2.0;
        (self.width, self.height, self.depth) = (size.x, size.y, size.z);
        self.growing = 0;
    }

    /// The position of a mob of `kind` standing on the block whose top face is at
    /// `ground_y`, at `(x, z)`.
    pub fn standing_at(kind: MobKind, x: f32, ground_y: f32, z: f32) -> Vec3 {
        Vec3::new(x, ground_y + kind.size().y / 2.0, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blow_takes_the_damage_of_the_weapon_and_one_bare_handed() {
        for kind in MobKind::ALL {
            assert_eq!(kind.weak_to_axes(), kind == MobKind::Stumpling, "{kind:?}");
            assert_eq!(kind.blow_damage(None), 1);
            for item in [ItemId::IronSword, ItemId::Spear] {
                assert_eq!(kind.blow_damage(Some(item)), item.attack_damage());
            }
        }
    }

    #[test]
    fn the_embers_and_the_arrows_are_the_projectiles_neither_hostile_nor_animals() {
        for kind in MobKind::ALL {
            let expected = matches!(kind, MobKind::Ember | MobKind::Arrow | MobKind::Bobber | MobKind::ThrownPotion);
            assert_eq!(kind.is_projectile(), expected, "{kind:?}");
        }
        assert!(!MobKind::Ember.is_hostile());
        assert!(MobKind::Cinderling.is_hostile());
    }

    #[test]
    fn a_foal_takes_after_its_parents_within_the_bounds_of_its_kind() {
        use rand::{rngs::StdRng, SeedableRng};
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let wild = HorseTraits::roll(&mut rng);
            assert!(HORSE_SPEED.contains(&wild.speed), "{wild:?}");
            assert!(HORSE_JUMP.contains(&wild.jump), "{wild:?}");
            assert!(HORSE_ENDURANCE.contains(&wild.endurance), "{wild:?}");
        }
        let fast = HorseTraits {
            coat: Coat::Black,
            speed: 11.5,
            jump: 3.0,
            endurance: 5.0,
        };
        let slow = HorseTraits {
            coat: Coat::Dun,
            speed: 7.0,
            jump: 1.5,
            endurance: 13.0,
        };
        for _ in 0..200 {
            let foal = HorseTraits::inherit(&fast, &slow, &mut rng);
            assert!((foal.speed - 9.25).abs() <= 0.6 + 1e-4, "{foal:?}");
            assert!((foal.jump - 2.25).abs() <= 0.25 + 1e-4, "{foal:?}");
            assert!((foal.endurance - 9.0).abs() <= 1.2 + 1e-4, "{foal:?}");
        }
        // The dark coats are faster on average, the pale ones last longer
        assert!(Coat::Black.base().0 > Coat::Dun.base().0);
        assert!(Coat::Black.base().1 < Coat::Dun.base().1);
    }

    #[test]
    fn an_axe_deals_a_stumpling_half_again_its_damage() {
        let axe = ItemId::DiamondAxe;
        assert_eq!(MobKind::Cow.blow_damage(Some(axe)), axe.attack_damage());
        assert_eq!(
            MobKind::Stumpling.blow_damage(Some(axe)),
            axe.attack_damage() * 3 / 2
        );
        assert_eq!(
            MobKind::Stumpling.blow_damage(Some(ItemId::IronSword)),
            ItemId::IronSword.attack_damage()
        );
    }
}
