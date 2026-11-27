//! The leads of the mobs (`ServerMob::leash`): a rope from the neck of the animal to the hand
//! of the player who leads it, or to the top of the fence post it is tied to, sagging a
//! little, as the line of a fishing rod.

use bevy::prelude::*;
use shared::players::Player;
use shared::world::Leash;

use super::fishing::line_point;
use super::Mob;

/// The segments a lead is drawn in.
const LEAD_SEGMENTS: usize = 12;
const LEAD_COLOUR: Color = Color::srgb(0.55, 0.42, 0.26);

/// Where the lead of a mob is held: the hand of its player, the top of its post.
pub fn lead_end(leash: &Leash, players: &Query<&Player>) -> Option<Vec3> {
    match leash {
        Leash::Player(name) => players
            .iter()
            .find(|player| &player.name == name)
            .map(|player| player.position + Vec3::Y * 0.2),
        Leash::Post(cell) => Some(cell.as_vec3() + Vec3::new(0.5, 0.9, 0.5)),
    }
}

/// Draws the lead of each leashed mob.
pub fn draw_leads(mut gizmos: Gizmos, mobs: Query<(&Mob, &Transform)>, players: Query<&Player>) {
    for (mob, transform) in &mobs {
        let Some(end) = mob
            .leash
            .as_ref()
            .and_then(|leash| lead_end(leash, &players))
        else {
            continue;
        };
        // The neck: high on the box, toward its front
        let size = mob.kind.size() * mob.scale;
        let neck = transform.translation
            + Vec3::Y * (size.y * 0.3)
            + transform.rotation * (Vec3::Z * size.z * 0.4);
        let points =
            (0..=LEAD_SEGMENTS).map(|i| line_point(end, neck, i as f32 / LEAD_SEGMENTS as f32));
        gizmos.linestrip(points, LEAD_COLOUR);
    }
}
