use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::messages::PlayerId;

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
}

impl MobKind {
    pub const ALL: [MobKind; 10] = [
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
    ];

    /// The points of health of a mob of this kind.
    pub fn health(self) -> u32 {
        match self {
            MobKind::Fox | MobKind::Pig | MobKind::Goat | MobKind::Cat => 10,
            MobKind::Cow => 12,
            MobKind::Wolf | MobKind::Horse => 16,
            MobKind::Chicken | MobKind::Rabbit => 4,
            MobKind::Sheep => 8,
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
        }
    }

    /// The position of a mob of `kind` standing on the block whose top face is at
    /// `ground_y`, at `(x, z)`.
    pub fn standing_at(kind: MobKind, x: f32, ground_y: f32, z: f32) -> Vec3 {
        Vec3::new(x, ground_y + kind.size().y / 2.0, z)
    }
}
