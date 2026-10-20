use bevy::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::messages::PlayerId;

pub type MobId = u128;

/// Append only: the variants are sent by index (see `ENCODING_WARNING`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MobKind {
    Fox,
    Sheep,
}

impl MobKind {
    pub const ALL: [MobKind; 2] = [MobKind::Fox, MobKind::Sheep];

    /// Width, height and depth of the hitbox, in blocks. It does not turn with the mob, so it
    /// is as wide as it is deep.
    pub fn size(self) -> Vec3 {
        match self {
            MobKind::Fox => Vec3::new(0.7, 0.7, 0.7),
            MobKind::Sheep => Vec3::new(0.9, 1.25, 0.9),
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
        }
    }

    /// The position of a mob of `kind` standing on the block whose top face is at
    /// `ground_y`, at `(x, z)`.
    pub fn standing_at(kind: MobKind, x: f32, ground_y: f32, z: f32) -> Vec3 {
        Vec3::new(x, ground_y + kind.size().y / 2.0, z)
    }
}
