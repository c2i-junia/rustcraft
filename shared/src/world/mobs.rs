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
}

impl MobKind {
    pub const ALL: [MobKind; 23] = [
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
        )
    }

    /// Whether this kind is a boss: hostile, but neither born, pushed, removed nor saved with the
    /// others.
    pub fn is_boss(self) -> bool {
        self == MobKind::EmberMother
    }

    /// Whether mobs of this kind are projectiles the server flies (`MobKind::Ember`, `MobKind::Arrow`): they do
    /// not walk, are not pushed nor struck, are not counted with the hostiles, and are not
    /// saved.
    pub fn is_projectile(self) -> bool {
        matches!(self, MobKind::Ember | MobKind::Arrow)
    }

    /// Whether mobs of this kind are villagers: they trade (`trade::offers`), live in a house,
    /// and nothing hurts them.
    pub fn is_villager(self) -> bool {
        matches!(
            self,
            MobKind::Farmer | MobKind::Smith | MobKind::Cartographer
        )
    }

    /// The points of health of a mob of this kind.
    pub fn health(self) -> u32 {
        match self {
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
            MobKind::Ember | MobKind::Arrow => 1,
            MobKind::Farmer | MobKind::Smith | MobKind::Cartographer => 20,
            MobKind::Cow => 12,
            MobKind::Wolf | MobKind::Horse => 16,
            MobKind::Chicken | MobKind::Rabbit => 4,
            MobKind::Sheep => 8,
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
        !self.is_hostile() && !self.is_projectile() && !self.is_villager()
    }

    /// Whether a player tames mobs of this kind by feeding them (`MobKind::eats`): the wolf
    /// and the cat. A tamed one follows its owner, and only tamed ones breed.
    pub fn is_tameable(self) -> bool {
        matches!(self, MobKind::Wolf | MobKind::Cat)
    }

    /// Whether mobs of this kind eat `item` from a player's hand: fed to an adult, it makes it
    /// want a mate (`LOVE_TICKS`), fed to a young one it grows it up faster, and fed to a wolf
    /// or a cat not yet tamed it may tame it (`TAME_CHANCE`).
    pub fn eats(self, item: ItemId) -> bool {
        use ItemId as I;
        match self {
            MobKind::Cow | MobKind::Sheep | MobKind::Goat | MobKind::Chicken => item == I::Rye,
            MobKind::Horse => matches!(item, I::Carrot | I::Rye),
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
    /// What a villager remembers; none for the other mobs. Saves from before it read none.
    #[serde(default)]
    pub villager: Option<Villager>,
    /// What made the last blow that hurt the mob special, if anything: the clients show and hear
    /// it with the hurt (`melee::Strike`).
    #[serde(default)]
    pub struck: Option<crate::players::melee::Strike>,
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
            villager: kind.is_villager().then(Villager::default),
            struck: None,
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
            let expected = matches!(kind, MobKind::Ember | MobKind::Arrow);
            assert_eq!(kind.is_projectile(), expected, "{kind:?}");
        }
        assert!(!MobKind::Ember.is_hostile());
        assert!(MobKind::Cinderling.is_hostile());
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
