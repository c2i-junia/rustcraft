use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::messages::PlayerId;
use crate::world::{ItemId, PortalRest, ToolKind};

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
}

impl MobKind {
    pub const ALL: [MobKind; 14] = [
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
    ];

    /// Whether mobs of this kind hunt the players (`crate::world::hostiles`), and appear in the
    /// dark rather than with the chunks.
    pub fn is_hostile(self) -> bool {
        matches!(
            self,
            MobKind::Gnawer | MobKind::Bramble | MobKind::Roller | MobKind::Stumpling
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
        }
    }
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
            portal_rest: None,
            hurt_ticks: 0,
            attack_wait: 0,
        }
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
