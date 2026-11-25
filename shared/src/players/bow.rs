//! The bow: drawn while the right button is held, it shoots an arrow when the button is
//! released. The draw is simulated by the client and the server alike, as it slows the player;
//! the arrow is flown by the server alone (`MobKind::Arrow`).

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{blocks::held_item, Player},
    world::{ItemId, ItemStack},
    MAX_INVENTORY_SLOTS,
};

use super::movement::MAX_FRAME_DELTA_MS;

/// How long the bow takes to be fully drawn, in ms.
pub const FULL_DRAW_MS: u32 = 1000;
/// A bow released before this shoots nothing, in ms.
pub const MIN_DRAW_MS: u32 = 150;
/// The speed of the player against its walk while it draws a bow.
pub const DRAW_SPEED_MULTIPLIER: f32 = 0.4;
/// The speed of an arrow shot with a full draw, in blocks/s; it is proportional to the power.
pub const ARROW_SPEED: f32 = 42.0;
/// The points of health an arrow takes, from the weakest draw to the full one.
const ARROW_DAMAGE: (f32, f32) = (1.0, 9.0);

/// An arrow a player let fly: the server adds it to the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BowShot {
    /// How far the bow was drawn (`power`).
    pub power: f32,
    /// The points of health the arrow takes from what it hits.
    pub damage: u32,
    /// Whether the arrow came out of an inventory, and may go back into one.
    pub recoverable: bool,
}

impl BowShot {
    /// A shot of `bow`, drawn for `draw_ms`.
    pub fn new(bow: &ItemStack, draw_ms: u32, recoverable: bool) -> Self {
        let power = power(draw_ms);
        let damage = ARROW_DAMAGE.0 + (ARROW_DAMAGE.1 - ARROW_DAMAGE.0) * power;
        Self {
            power,
            damage: damage.round() as u32 + bow.bonus_arrow_damage(),
            recoverable,
        }
    }
}

/// How far a bow drawn for `draw_ms` is, from 0 to 1: slow at first, then quicker.
pub fn power(draw_ms: u32) -> f32 {
    let t = (draw_ms as f32 / FULL_DRAW_MS as f32).min(1.0);
    (t * t + 2.0 * t) / 3.0
}

/// Which of the three pictures of a drawn bow shows after `draw_ms`, if it is drawn: from 1.
pub fn draw_stage(draw_ms: u32) -> Option<u8> {
    match draw_ms {
        0 => None,
        ms if ms < FULL_DRAW_MS * 2 / 5 => Some(1),
        ms if ms < FULL_DRAW_MS * 9 / 10 => Some(2),
        _ => Some(3),
    }
}

/// The slot of the first stack of arrows of `player`.
fn arrow_slot(player: &Player) -> Option<u32> {
    let is_arrow = |slot: &u32| {
        let stack = player.inventory.inner.get(slot);
        stack.is_some_and(|stack| stack.item_id == ItemId::Arrow && stack.nb > 0)
    };
    (0..MAX_INVENTORY_SLOTS).find(is_arrow)
}

/// Draws the bow in the selected slot while the right button is down, and lets it down
/// otherwise: part of the movement, which a client replays. A press a block took
/// (`Player::place_wait_ms`) draws nothing.
pub fn draw_bow(player: &mut Player, action: &PlayerFrameInput) {
    let drawn_ms = player.bow_draw_ms;
    let draws = action.is_pressed(NetworkAction::RightClick)
        && !action.is_pressed(NetworkAction::LowerBow)
        && held_item(player, action) == Some(ItemId::Bow)
        && (drawn_ms > 0 || player.place_wait_ms == 0)
        && (player.game_mode.is_creative() || arrow_slot(player).is_some());
    player.bow_draw_ms = if draws {
        let delta = action.delta_ms.min(MAX_FRAME_DELTA_MS) as u32;
        (drawn_ms + delta).min(FULL_DRAW_MS)
    } else {
        0
    };
}

/// Shoots with the bow in the selected slot when the right button was just released, before
/// `draw_bow` lets it down: the bow wears and an arrow is gone, but in creative. A UI opened over
/// the draw (`NetworkAction::LowerBow`) releases nothing.
pub fn shoot_bow(player: &mut Player, action: &PlayerFrameInput) -> Option<BowShot> {
    if player.bow_draw_ms < MIN_DRAW_MS
        || action.is_pressed(NetworkAction::RightClick)
        || action.is_pressed(NetworkAction::LowerBow)
    {
        return None;
    }
    let bow = *player.inventory.inner.get(&action.hotbar_slot)?;
    if bow.item_id != ItemId::Bow {
        return None;
    }
    let creative = player.game_mode.is_creative();
    if !creative {
        let slot = arrow_slot(player)?;
        player.inventory.remove_item_from_stack(slot, 1);
        player.inventory.wear_tool(action.hotbar_slot, 1);
    }
    Some(BowShot::new(&bow, player.bow_draw_ms, !creative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::GameMode;
    use crate::world::ItemType;
    use bevy::prelude::{Transform, Vec3};

    fn archer(arrows: u32) -> Player {
        let mut player = Player::new(1, "archer".into(), Vec3::ZERO, Transform::default());
        for (slot, item_id, nb) in [(0, ItemId::Bow, 1), (12, ItemId::Arrow, arrows)] {
            let item_type = item_id.get_default_type();
            let stack = ItemStack {
                item_id,
                item_type,
                nb,
            };
            if nb > 0 {
                player.inventory.inner.insert(slot, stack);
            }
        }
        player
    }

    /// The shots of `player` holding the right button for `ms` then releasing it.
    fn shoot(player: &mut Player, ms: u64) -> Option<BowShot> {
        let mut input = PlayerFrameInput {
            delta_ms: 50,
            ..Default::default()
        };
        input.inputs.insert(NetworkAction::RightClick);
        for _ in 0..ms / 50 {
            assert_eq!(shoot_bow(player, &input), None);
            draw_bow(player, &input);
        }
        input.inputs.clear();
        let shot = shoot_bow(player, &input);
        draw_bow(player, &input);
        shot
    }

    #[test]
    fn a_bow_shoots_harder_the_longer_it_is_drawn_and_spends_an_arrow() {
        let mut player = archer(2);
        assert_eq!(shoot(&mut player, 100), None, "hardly drawn");
        let weak = shoot(&mut player, 300).unwrap();
        let full = shoot(&mut player, 1500).unwrap();
        assert!(weak.power < full.power && weak.damage < full.damage);
        assert_eq!((full.power, full.damage), (1.0, 9));
        assert!(full.recoverable);
        assert_eq!(player.inventory.inner.len(), 1, "both arrows are gone");
        assert_eq!(
            player.inventory.inner[&0].item_type,
            ItemType::Tool { wear: 2 }
        );
        assert_eq!(shoot(&mut player, 1500), None, "no arrow left");
        // In creative it needs none, and does not wear
        player.game_mode = GameMode::Creative;
        assert!(!shoot(&mut player, 1500).unwrap().recoverable);
        assert_eq!(
            player.inventory.inner[&0].item_type,
            ItemType::Tool { wear: 2 }
        );
    }

    #[test]
    fn a_ui_opened_over_the_draw_lets_the_bow_down_without_shooting() {
        let mut player = archer(1);
        let mut input = PlayerFrameInput {
            delta_ms: 50,
            ..Default::default()
        };
        input.inputs.insert(NetworkAction::RightClick);
        for _ in 0..20 {
            draw_bow(&mut player, &input);
        }
        assert_eq!(player.bow_draw_ms, FULL_DRAW_MS);
        input.inputs = [NetworkAction::LowerBow].into();
        for _ in 0..2 {
            assert_eq!(shoot_bow(&mut player, &input), None);
            draw_bow(&mut player, &input);
            assert_eq!(player.bow_draw_ms, 0);
            // The UI closes
            input.inputs.clear();
        }
        assert!(arrow_slot(&player).is_some());
    }
}
