//! The hunger of a player in survival: its efforts tire it, which takes its saturation and then
//! its food; well fed it heals, starving it dies, and at 6 food or less it cannot sprint.

use super::game_mode::{DamageSource, MAX_HEALTH};
use super::Player;

/// The food of a player who is not hungry at all: 10 drumsticks of 2 points.
pub const MAX_FOOD: u8 = 20;
/// The saturation a new or respawned player has.
pub const RESPAWN_SATURATION: f32 = 5.0;
/// The exhaustion that takes a point of saturation, or of food once the saturation is gone.
pub const EXHAUSTION_WRAP: f32 = 4.0;
/// The least food a player needs to sprint: it cannot at 6 or less.
pub const SPRINT_MIN_FOOD: u8 = 7;
/// The least food with which a player's health comes back by itself.
pub const REGENERATION_MIN_FOOD: u8 = 18;
/// The damage a starving player takes at each metabolism.
pub const STARVATION_DAMAGE: u32 = 1;
/// The drumsticks of a full food row.
pub const DRUMSTICKS: u8 = MAX_FOOD / 2;

/// The exhaustion of a block run along the ground while sprinting.
pub const SPRINT_EXHAUSTION: f32 = 0.1;
/// The exhaustion of a block moved in a liquid.
pub const SWIM_EXHAUSTION: f32 = 0.01;
/// The exhaustion of a jump, and of a jump while sprinting.
pub const JUMP_EXHAUSTION: f32 = 0.05;
pub const SPRINT_JUMP_EXHAUSTION: f32 = 0.2;
/// The exhaustion of a broken block.
pub const BREAK_EXHAUSTION: f32 = 0.005;
/// The exhaustion of a blow that lands on a mob.
pub const BLOW_EXHAUSTION: f32 = 0.1;
/// The exhaustion of a hit that takes health.
pub const DAMAGE_EXHAUSTION: f32 = 0.1;
/// The exhaustion of a point of health regenerated.
pub const REGENERATION_EXHAUSTION: f32 = 6.0;

/// How much of a drumstick is filled.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Drumstick {
    Empty,
    Half,
    Full,
}

/// The drumstick `index` of the row for `food` points, from the right.
pub fn drumstick(food: u8, index: u8) -> Drumstick {
    match food.saturating_sub(index * 2) {
        0 => Drumstick::Empty,
        1 => Drumstick::Half,
        _ => Drumstick::Full,
    }
}

pub(super) fn full_food() -> u8 {
    MAX_FOOD
}

pub(super) fn respawn_saturation() -> f32 {
    RESPAWN_SATURATION
}

impl Player {
    /// Tires a living survival player by `amount`: each `EXHAUSTION_WRAP` of it takes a point
    /// of saturation, or of food once the saturation is gone. Creative never tires.
    pub fn exhaust(&mut self, amount: f32) {
        if self.is_dead() || !self.game_mode.takes_damage() {
            return;
        }
        self.exhaustion += amount;
        while self.exhaustion >= EXHAUSTION_WRAP {
            self.exhaustion -= EXHAUSTION_WRAP;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.food = self.food.saturating_sub(1);
            }
        }
    }

    /// Whether the player may start or go on sprinting: in survival only with more than 6
    /// food. The client's prediction and the server read it alike.
    pub fn may_sprint(&self) -> bool {
        !self.game_mode.takes_damage() || self.food >= SPRINT_MIN_FOOD
    }

    /// The metabolism, every `REGENERATION_TICKS`: a survival player with
    /// `REGENERATION_MIN_FOOD` or more gets a point of health back, which tires it, and one with
    /// no food starves by `STARVATION_DAMAGE`, to death. In creative the health comes back
    /// without food.
    pub fn metabolise(&mut self) {
        if self.is_dead() {
            return;
        }
        let hungry = self.game_mode.takes_damage();
        if self.health < MAX_HEALTH && (!hungry || self.food >= REGENERATION_MIN_FOOD) {
            self.health += 1;
            self.exhaust(REGENERATION_EXHAUSTION);
        } else if hungry && self.food == 0 {
            self.hurt(STARVATION_DAMAGE, DamageSource::Starvation);
        }
    }

    /// Eats a food of `food` points and `saturation`: the food goes up to `MAX_FOOD`, the
    /// saturation up to the food.
    pub fn eat(&mut self, food: u8, saturation: f32) {
        self.food = (self.food + food).min(MAX_FOOD);
        self.saturation = (self.saturation + saturation).min(self.food as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::{DeathCause, GameMode};
    use bevy::prelude::{Transform, Vec3};

    fn survivor() -> Player {
        Player::new(1, "eater".into(), Vec3::ZERO, Transform::default())
    }

    #[test]
    fn a_new_player_is_fed() {
        let player = survivor();
        assert_eq!(player.food, MAX_FOOD);
        assert_eq!(player.saturation, RESPAWN_SATURATION);
        assert_eq!(player.exhaustion, 0.0);
    }

    #[test]
    fn exhaustion_takes_the_saturation_first_then_the_food() {
        let mut player = survivor();
        player.saturation = 1.5;
        player.exhaust(3.9);
        assert_eq!(player.saturation, 1.5);
        player.exhaust(0.2);
        assert_eq!(player.saturation, 0.5);
        assert!((player.exhaustion - 0.1).abs() < 1e-5);
        // Half a point of saturation left is taken whole
        player.exhaust(4.0);
        assert_eq!(player.saturation, 0.0);
        assert_eq!(player.food, MAX_FOOD);
        player.exhaust(4.0);
        assert_eq!(player.food, MAX_FOOD - 1);
        // An amount that holds the wrap twice wraps twice
        player.exhaust(8.0);
        assert_eq!(player.food, MAX_FOOD - 3);
        player.food = 0;
        player.exhaust(4.0);
        assert_eq!(player.food, 0);
    }

    #[test]
    fn creative_and_dead_players_do_not_tire() {
        let mut creative = Player {
            game_mode: GameMode::Creative,
            saturation: 0.0,
            ..survivor()
        };
        creative.exhaust(100.0);
        assert_eq!(creative.food, MAX_FOOD);
        assert_eq!(creative.exhaustion, 0.0);
        let mut dead = Player {
            health: 0,
            ..survivor()
        };
        dead.exhaust(100.0);
        assert_eq!(dead.food, MAX_FOOD);
    }

    #[test]
    fn a_hit_that_takes_health_tires() {
        let mut player = survivor();
        player.hurt(2, DamageSource::Fall);
        assert!((player.exhaustion - DAMAGE_EXHAUSTION).abs() < 1e-6);
    }

    #[test]
    fn health_comes_back_only_from_eighteen_food_and_tires() {
        let mut player = survivor();
        player.health = 10;
        player.food = REGENERATION_MIN_FOOD;
        player.metabolise();
        assert_eq!(player.health, 11);
        // 6 of exhaustion: a point of saturation, 2 left
        assert_eq!(player.saturation, RESPAWN_SATURATION - 1.0);
        assert!((player.exhaustion - 2.0).abs() < 1e-5);

        player.food = REGENERATION_MIN_FOOD - 1;
        player.metabolise();
        assert_eq!(player.health, 11);

        // In full health, nothing is regenerated nor tired
        let mut full = survivor();
        full.metabolise();
        assert_eq!(full.exhaustion, 0.0);
    }

    #[test]
    fn creative_heals_without_food() {
        let mut player = Player {
            game_mode: GameMode::Creative,
            food: 0,
            health: 5,
            ..survivor()
        };
        player.metabolise();
        assert_eq!(player.health, 6);
        assert_eq!(player.food, 0);
    }

    #[test]
    fn a_starving_player_loses_health_to_death() {
        let mut player = Player {
            food: 0,
            saturation: 0.0,
            ..survivor()
        };
        player.metabolise();
        assert_eq!(player.health, MAX_HEALTH - STARVATION_DAMAGE);
        // It pierces the invulnerability of the hit just taken
        player.metabolise();
        assert_eq!(player.health, MAX_HEALTH - 2 * STARVATION_DAMAGE);
        for _ in 0..MAX_HEALTH {
            player.metabolise();
        }
        assert!(player.is_dead());
        assert_eq!(player.death_cause, Some(DeathCause::Starvation));

        // A creative player with no food does not starve
        let mut creative = Player {
            game_mode: GameMode::Creative,
            food: 0,
            ..survivor()
        };
        creative.metabolise();
        assert_eq!(creative.health, MAX_HEALTH);
    }

    #[test]
    fn sprinting_needs_more_than_six_food_in_survival() {
        let mut player = survivor();
        player.food = 7;
        assert!(player.may_sprint());
        player.food = 6;
        assert!(!player.may_sprint());
        player.game_mode = GameMode::Creative;
        player.food = 0;
        assert!(player.may_sprint());
    }

    #[test]
    fn eating_adds_food_and_saturation_up_to_the_food() {
        let mut player = Player {
            food: 4,
            saturation: 0.0,
            ..survivor()
        };
        player.eat(8, 12.8);
        assert_eq!(player.food, 12);
        assert_eq!(player.saturation, 12.0);
        player.eat(10, 1.0);
        assert_eq!(player.food, MAX_FOOD);
        assert_eq!(player.saturation, 13.0);
    }

    #[test]
    fn a_respawn_feeds_the_player() {
        let mut player = Player {
            food: 0,
            saturation: 0.0,
            exhaustion: 3.0,
            health: 0,
            ..survivor()
        };
        player.respawn(Vec3::Y * 70.0);
        assert_eq!(player.food, MAX_FOOD);
        assert_eq!(player.saturation, RESPAWN_SATURATION);
        assert_eq!(player.exhaustion, 0.0);
    }

    #[test]
    fn an_old_player_file_loads_fed() {
        let mut saved = ron::ser::to_string(&survivor()).unwrap();
        for field in ["food", "saturation", "exhaustion"] {
            let start = saved.find(&format!("{field}:")).expect(field);
            let end = start + saved[start..].find(',').unwrap() + 1;
            saved.replace_range(start..end, "");
        }
        let player: Player = ron::from_str(&saved).unwrap();
        assert_eq!(player.food, MAX_FOOD);
        assert_eq!(player.saturation, RESPAWN_SATURATION);
        assert_eq!(player.exhaustion, 0.0);
    }

    #[test]
    fn the_drumsticks_fill_from_the_right_two_points_each() {
        let row = |food| {
            (0..DRUMSTICKS)
                .map(|i| drumstick(food, i))
                .collect::<Vec<_>>()
        };
        assert!(row(MAX_FOOD).iter().all(|&d| d == Drumstick::Full));
        assert!(row(0).iter().all(|&d| d == Drumstick::Empty));
        let seven = row(7);
        assert_eq!(
            &seven[..4],
            [
                Drumstick::Full,
                Drumstick::Full,
                Drumstick::Full,
                Drumstick::Half
            ]
        );
        assert!(seven[4..].iter().all(|&d| d == Drumstick::Empty));
    }
}
