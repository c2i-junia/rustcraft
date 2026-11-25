//! The depths compass: its needle points to the lair of the Ember Mother from where the local
//! player looks, in hand and in the slots. It spins right over the lair, and in a world that has
//! none.

use std::f32::consts::TAU;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use shared::world::ItemId;

use crate::camera::CameraController;
use crate::world::MaterialResource;

/// The pictures of the needle, `DepthsCompass_Draw1` (ahead) to this one, clockwise.
pub const PICTURES: u8 = 16;
/// The needle spins this near the column of the middle of the lair, in blocks.
const OVER: f32 = 4.0;
/// Pictures a second of a needle that spins.
const SPIN: f32 = 10.0;

/// Where the lair of the world is, if it has one.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LairPlace(pub Option<Vec3>);

/// The picture of the needle of the moment, from 1 to `PICTURES`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompassNeedle(pub u8);

impl Default for CompassNeedle {
    fn default() -> Self {
        Self(1)
    }
}

/// The picture of a needle from `eye` that looks toward `forward`, `elapsed` seconds into the
/// game.
fn needle(lair: Option<Vec3>, eye: Vec3, forward: Vec3, elapsed: f32) -> u8 {
    let towards = lair.map(|lair| (lair - eye).xz());
    let Some(towards) = towards.filter(|towards| towards.length() > OVER) else {
        return (elapsed * SPIN) as u32 as u8 % PICTURES + 1;
    };
    let ahead = forward.xz().normalize_or(Vec2::NEG_Y);
    let right = Vec2::new(-ahead.y, ahead.x);
    let turn = towards.dot(right).atan2(towards.dot(ahead)) / TAU;
    ((turn * f32::from(PICTURES)).round() as i32).rem_euclid(i32::from(PICTURES)) as u8 + 1
}

pub fn turn_the_needle(
    lair: Res<LairPlace>,
    camera: Query<&GlobalTransform, With<CameraController>>,
    time: Res<Time>,
    mut shown: ResMut<CompassNeedle>,
) {
    if let Ok(camera) = camera.single() {
        let up = camera.up().as_vec3();
        let forward = camera.forward().as_vec3();
        // Straight down, ahead is the top of the view
        let forward = if forward.y < -0.99 { up } else { forward };
        let turned = needle(lair.0, camera.translation(), forward, time.elapsed_secs());
        shown.set_if_neq(CompassNeedle(turned));
    }
}

/// The pictures of the items in the slots.
#[derive(SystemParam)]
pub struct SlotPictures<'w> {
    materials: Res<'w, MaterialResource>,
    needle: Option<Res<'w, CompassNeedle>>,
}

impl SlotPictures<'_> {
    pub fn sprite(&self, item: ItemId) -> Option<TextureAtlas> {
        let atlas = self.materials.items.as_ref()?;
        let turned = self
            .needle
            .as_ref()
            .filter(|_| item == ItemId::DepthsCompass);
        match turned {
            Some(needle) => atlas.sprite(&format!("{item:?}_Draw{}", needle.0)),
            None => atlas.sprite(&format!("{item:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_needle_points_to_the_lair_from_where_the_player_looks() {
        let lair = Some(Vec3::new(0.0, 11.0, -500.0));
        let at = |forward| needle(lair, Vec3::new(0.0, 70.0, 0.0), forward, 0.0);
        assert_eq!(at(Vec3::NEG_Z), 1, "ahead");
        assert_eq!(at(Vec3::NEG_X), 5, "on the right");
        assert_eq!(at(Vec3::Z), 9, "behind");
        assert_eq!(at(Vec3::X), 13, "on the left");
    }
}
