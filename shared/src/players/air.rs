//! The air of a player: it runs out while the eyes are in a liquid, and then the player drowns.

use super::constants::{EYE_HEIGHT, SNEAK_BODY_SCALE};
use super::game_mode::DamageSource;
use super::Player;
use bevy::math::Vec3;

/// The air of a player who breathes, in ticks: 15 s, 10 bubbles of 30.
pub const MAX_AIR: u16 = 300;
/// The air a player gets back each tick it breathes.
pub const AIR_REFILL: u16 = 6;
/// The damage a player with no air left takes every `DROWNING_INTERVAL` ticks.
pub const DROWNING_DAMAGE: u32 = 2;
/// The ticks between two hits of a player with no air left.
pub const DROWNING_INTERVAL: u16 = 20;
/// The bubbles of a full air.
pub const BUBBLES: u16 = 10;

/// What a bubble of the air row shows.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Bubble {
    Full,
    /// The last bubble, about to burst.
    Bursting,
    /// Gone: nothing is drawn.
    Empty,
}

/// The bubble `index` of the row for `air`, from the right. As in Minecraft, a bubble bursts
/// for the last 2 ticks of its 30 before it goes.
pub fn bubble(air: u16, index: u16) -> Bubble {
    let per_bubble = MAX_AIR / BUBBLES;
    let full = air.saturating_sub(2).div_ceil(per_bubble);
    let shown = air.div_ceil(per_bubble);
    if index < full {
        Bubble::Full
    } else if index < shown {
        Bubble::Bursting
    } else {
        Bubble::Empty
    }
}

pub(super) fn full_air() -> u16 {
    MAX_AIR
}

impl Player {
    /// Where the eyes of the player are, once its stance has settled: `EYE_HEIGHT` above the
    /// centre of its box, lower while it sneaks, as the camera of the client.
    pub fn eye_position(&self) -> Vec3 {
        let scale = if self.is_sneaking {
            SNEAK_BODY_SCALE
        } else {
            1.0
        };
        // The body is squashed toward the feet, the eyes with it
        let body_centre = -self.height * (1.0 - scale) / 2.0;
        self.position + Vec3::Y * (body_centre + EYE_HEIGHT * scale)
    }

    /// One tick of breathing: with `eyes_in_liquid`, a survival player loses a tick of air,
    /// and with none left takes `DROWNING_DAMAGE` every `DROWNING_INTERVAL` ticks; else, or in
    /// creative, the air comes back `AIR_REFILL` a tick. A dead player does not breathe.
    pub fn breathe(&mut self, eyes_in_liquid: bool) {
        if self.is_dead() {
            return;
        }
        if !eyes_in_liquid || !self.game_mode.takes_damage() {
            self.air = (self.air + AIR_REFILL).min(MAX_AIR);
            self.drowning_ticks = 0;
            return;
        }
        if self.air > 0 {
            self.air -= 1;
            return;
        }
        self.drowning_ticks += 1;
        if self.drowning_ticks >= DROWNING_INTERVAL {
            self.drowning_ticks = 0;
            self.hurt(DROWNING_DAMAGE, DamageSource::Drowning);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::{DeathCause, GameMode, MAX_HEALTH};
    use bevy::prelude::Transform;

    fn survivor() -> Player {
        Player::new(1, "diver".into(), Vec3::ZERO, Transform::default())
    }

    /// `ticks` of breathing, the damage timers counted down first as on the server.
    fn breathe(player: &mut Player, eyes_in_liquid: bool, ticks: u32) {
        for _ in 0..ticks {
            player.tick_damage_timers();
            player.breathe(eyes_in_liquid);
        }
    }

    #[test]
    fn a_new_player_has_all_its_air() {
        assert_eq!(survivor().air, MAX_AIR);
    }

    #[test]
    fn eyes_in_a_liquid_lose_a_tick_of_air_each_tick() {
        let mut player = survivor();
        breathe(&mut player, true, 1);
        assert_eq!(player.air, MAX_AIR - 1);
        breathe(&mut player, true, 99);
        assert_eq!(player.air, MAX_AIR - 100);
        assert_eq!(player.health, MAX_HEALTH);
    }

    #[test]
    fn eyes_out_of_a_liquid_lose_no_air() {
        let mut player = survivor();
        breathe(&mut player, false, 50);
        assert_eq!(player.air, MAX_AIR);
    }

    #[test]
    fn the_air_comes_back_six_a_tick_up_to_the_full() {
        let mut player = survivor();
        player.air = 10;
        breathe(&mut player, false, 1);
        assert_eq!(player.air, 16);
        breathe(&mut player, false, 100);
        assert_eq!(player.air, MAX_AIR);
    }

    #[test]
    fn with_no_air_left_two_damage_every_twenty_ticks() {
        let mut player = survivor();
        breathe(&mut player, true, MAX_AIR as u32);
        assert_eq!(player.air, 0);
        assert_eq!(player.health, MAX_HEALTH);

        breathe(&mut player, true, DROWNING_INTERVAL as u32 - 1);
        assert_eq!(player.health, MAX_HEALTH, "a hit before its time");
        breathe(&mut player, true, 1);
        assert_eq!(player.health, MAX_HEALTH - DROWNING_DAMAGE);
        breathe(&mut player, true, DROWNING_INTERVAL as u32);
        assert_eq!(player.health, MAX_HEALTH - 2 * DROWNING_DAMAGE);
    }

    #[test]
    fn a_breath_starts_the_count_to_the_next_hit_again() {
        let mut player = survivor();
        player.air = 0;
        breathe(&mut player, true, DROWNING_INTERVAL as u32 - 1);
        breathe(&mut player, false, 1);
        player.air = 0;
        breathe(&mut player, true, DROWNING_INTERVAL as u32 - 1);
        assert_eq!(player.health, MAX_HEALTH);
    }

    #[test]
    fn a_player_who_drowns_dies_of_drowning() {
        let mut player = survivor();
        player.air = 0;
        player.health = DROWNING_DAMAGE;
        breathe(&mut player, true, DROWNING_INTERVAL as u32);
        assert!(player.is_dead());
        assert_eq!(player.death_cause, Some(DeathCause::Drowning));

        // The dead breathe no more
        breathe(&mut player, false, 10);
        assert_eq!(player.air, 0);
    }

    #[test]
    fn creative_never_loses_air() {
        let mut player = Player {
            game_mode: GameMode::Creative,
            ..survivor()
        };
        breathe(&mut player, true, 1000);
        assert_eq!(player.air, MAX_AIR);
        assert_eq!(player.health, MAX_HEALTH);
    }

    #[test]
    fn a_respawn_refills_the_air() {
        let mut player = survivor();
        player.air = 0;
        breathe(&mut player, true, 5);
        player.health = 0;
        player.respawn(Vec3::Y * 70.0);
        assert_eq!(player.air, MAX_AIR);
        assert_eq!(player.drowning_ticks, 0);
    }

    #[test]
    fn the_bubbles_go_from_the_right_and_each_bursts_before_it_goes() {
        let row = |air| (0..BUBBLES).map(|i| bubble(air, i)).collect::<Vec<_>>();
        assert!(row(MAX_AIR).iter().all(|&b| b == Bubble::Full));
        assert!(row(0).iter().all(|&b| b == Bubble::Empty));
        // 271 to 300 is the tenth bubble: it bursts for its last 2 ticks
        assert_eq!(row(MAX_AIR - 1)[9], Bubble::Full);
        assert_eq!(row(273)[9], Bubble::Full);
        assert_eq!(row(272)[9], Bubble::Bursting);
        assert_eq!(row(271)[9], Bubble::Bursting);
        assert_eq!(row(270)[9], Bubble::Empty);
        assert_eq!(row(270)[8], Bubble::Full);
        assert_eq!(&row(1)[..2], [Bubble::Bursting, Bubble::Empty]);
    }

    #[test]
    fn the_eyes_are_lower_while_sneaking() {
        let mut player = Player {
            position: Vec3::new(0.5, 65.0, 0.5),
            ..survivor()
        };
        assert_eq!(
            player.eye_position(),
            Vec3::new(0.5, 65.0 + EYE_HEIGHT, 0.5)
        );
        player.is_sneaking = true;
        let feet = player.position.y - player.height / 2.0;
        let eye = player.eye_position().y;
        assert!(eye < 65.0 + EYE_HEIGHT && eye > feet, "{eye}");
        // The squashed body keeps its feet: the eyes keep their height above them, scaled
        let standing = EYE_HEIGHT + player.height / 2.0;
        assert!((eye - feet - standing * SNEAK_BODY_SCALE).abs() < 1e-5);
    }
}
