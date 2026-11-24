//! A player on fire: a fire block it touches sets it burning for a while, which hurts it until
//! it is over or water or rain puts it out.

use bevy::math::{bounding::Aabb3d, Vec3};

use super::game_mode::DamageSource;
use super::Player;
use crate::world::{ticks::sees_sky, BlockId, ChunkMap};

/// The ticks a player burns once out of a fire: 4 s.
pub const BURNING_TICKS: u16 = 80;
/// The damage a burning player takes every `FIRE_DAMAGE_INTERVAL` ticks, the first at once.
pub const FIRE_DAMAGE: u32 = 1;
/// The ticks between two hits of a burning player.
pub const FIRE_DAMAGE_INTERVAL: u16 = 20;

impl Player {
    /// The collision box of the player, where it is.
    fn body(&self) -> Aabb3d {
        Aabb3d::new(
            self.position,
            Vec3::new(self.width, self.height, self.width) / 2.0,
        )
    }

    /// Whether the player is on fire.
    pub fn is_burning(&self) -> bool {
        self.burning_ticks > 0
    }

    /// One tick of fire. `doused`, the fire is put out; else `in_fire` sets the player burning
    /// for at least `BURNING_TICKS`, and a burning player takes `FIRE_DAMAGE` at once, then every
    /// `FIRE_DAMAGE_INTERVAL` ticks while it burns. Creative and dead players do not burn.
    pub fn burn(&mut self, in_fire: bool, doused: bool) {
        if doused || self.is_dead() || !self.game_mode.takes_damage() {
            self.burning_ticks = 0;
            self.fire_damage_wait = 0;
            return;
        }
        if in_fire {
            self.burning_ticks = self.burning_ticks.max(BURNING_TICKS);
        }
        if self.burning_ticks == 0 {
            self.fire_damage_wait = 0;
            return;
        }
        if self.fire_damage_wait == 0 {
            self.hurt(FIRE_DAMAGE, DamageSource::Fire);
            self.fire_damage_wait = FIRE_DAMAGE_INTERVAL;
        }
        self.fire_damage_wait -= 1;
        self.burning_ticks -= 1;
    }
}

/// Whether the body of `player` touches a fire block.
pub fn body_in_fire(world: &ChunkMap, player: &Player) -> bool {
    world.touches_block(&player.body(), BlockId::Fire)
}

/// Whether what burns on `player` is put out: its body touches water, or it rains (`raining`)
/// on its head under the open sky.
pub fn is_doused(world: &ChunkMap, player: &Player, raining: bool) -> bool {
    world.touches_block(&player.body(), BlockId::Water)
        || raining && sees_sky(world, player.eye_position().floor().as_ivec3())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::{DeathCause, GameMode, MAX_HEALTH};
    use crate::players::PLAYER_HEIGHT;
    use bevy::math::IVec3;
    use bevy::prelude::Transform;

    fn survivor() -> Player {
        Player::new(1, "torch".into(), Vec3::ZERO, Transform::default())
    }

    /// `ticks` of fire, the damage timers counted down first as on the server.
    fn burn(player: &mut Player, in_fire: bool, doused: bool, ticks: u32) {
        for _ in 0..ticks {
            player.tick_damage_timers();
            player.burn(in_fire, doused);
        }
    }

    #[test]
    fn a_player_out_of_a_fire_burns_four_seconds_one_damage_a_second() {
        let mut player = survivor();
        burn(&mut player, true, false, 1);
        assert!(player.is_burning());
        assert_eq!(player.health, MAX_HEALTH - 1, "the first hit at once");
        burn(&mut player, false, false, u32::from(BURNING_TICKS) - 1);
        assert!(!player.is_burning());
        assert_eq!(player.health, MAX_HEALTH - 4);
        burn(&mut player, false, false, 100);
        assert_eq!(player.health, MAX_HEALTH - 4);
    }

    #[test]
    fn a_player_standing_in_a_fire_is_hit_once_a_second_and_can_die_of_it() {
        let mut player = survivor();
        burn(&mut player, true, false, 41);
        assert_eq!(player.health, MAX_HEALTH - 3);
        assert_eq!(player.burning_ticks, BURNING_TICKS - 1);
        burn(&mut player, true, false, 20 * 20);
        assert!(player.is_dead());
        assert_eq!(player.death_cause, Some(DeathCause::Fire));
        assert!(!player.is_burning());
    }

    #[test]
    fn water_or_rain_puts_a_burning_player_out() {
        let mut player = survivor();
        burn(&mut player, true, false, 1);
        burn(&mut player, false, true, 1);
        assert!(!player.is_burning());
        burn(&mut player, false, false, 100);
        assert_eq!(player.health, MAX_HEALTH - 1);
    }

    #[test]
    fn creative_players_do_not_burn_and_a_respawn_puts_the_fire_out() {
        let mut player = survivor();
        player.game_mode = GameMode::Creative;
        burn(&mut player, true, false, 30);
        assert!(!player.is_burning());
        assert_eq!(player.health, MAX_HEALTH);

        let mut player = survivor();
        burn(&mut player, true, false, 5);
        player.health = 0;
        player.respawn(Vec3::Y * 70.0);
        assert!(!player.is_burning());
        assert_eq!(player.fire_damage_wait, 0);
    }

    /// A stone floor at y = 0, a player standing on it at x, z = 4.5.
    fn standing() -> (ChunkMap, Player) {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(IVec3::splat(-16), IVec3::new(31, 0, 31), BlockId::Stone);
        let player = Player {
            position: Vec3::new(4.5, 1.0 + PLAYER_HEIGHT / 2.0, 4.5),
            ..survivor()
        };
        (world, player)
    }

    #[test]
    fn a_fire_at_the_feet_or_in_the_cell_beside_the_body_touches_it() {
        let (mut world, player) = standing();
        assert!(!body_in_fire(&world, &player));
        world.fill_box(IVec3::new(4, 1, 4), IVec3::new(4, 1, 4), BlockId::Fire);
        assert!(body_in_fire(&world, &player));
        // a fire one block away does not
        let (mut world, player) = standing();
        world.fill_box(IVec3::new(6, 1, 4), IVec3::new(6, 1, 4), BlockId::Fire);
        assert!(!body_in_fire(&world, &player));
    }

    #[test]
    fn water_at_the_feet_and_rain_on_the_head_douse_and_a_roof_keeps_the_rain_off() {
        let (mut world, player) = standing();
        assert!(!is_doused(&world, &player, false));
        assert!(is_doused(&world, &player, true));
        world.fill_box(IVec3::new(4, 6, 4), IVec3::new(4, 6, 4), BlockId::Stone);
        assert!(!is_doused(&world, &player, true));
        world.fill_box(IVec3::new(4, 1, 4), IVec3::new(4, 1, 4), BlockId::Water);
        assert!(is_doused(&world, &player, false));
    }
}
