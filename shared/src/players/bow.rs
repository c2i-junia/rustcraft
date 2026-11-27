//! The bow: drawn while the right button is held, it shoots an arrow when the button is
//! released. The crossbow is loaded the same way, slower, and shoots a piercing bolt only once
//! loaded; a throwing axe is swung back and thrown, and falls back as itself. The draw is simulated by the client and the server alike, as it slows the player;
//! the arrow is flown by the server alone (`MobKind::Arrow`).

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{blocks::held_item, Player},
    world::{ItemId, ItemStack, ItemType},
    MAX_INVENTORY_SLOTS,
};

use super::movement::MAX_FRAME_DELTA_MS;

/// How long the bow takes to be fully drawn, in ms.
pub const FULL_DRAW_MS: u32 = 1000;
/// How long a crossbow takes to be loaded, in ms: it shoots only once loaded.
pub const CROSSBOW_LOAD_MS: u32 = 1250;
/// How long a throwing axe takes to be swung back fully, in ms.
pub const THROW_FULL_MS: u32 = 600;
/// A bow released before this shoots nothing, in ms.
pub const MIN_DRAW_MS: u32 = 150;
/// The speed of the player against its walk while it draws a bow.
pub const DRAW_SPEED_MULTIPLIER: f32 = 0.4;
/// The speed of an arrow shot with a full draw, in blocks/s; it is proportional to the power.
pub const ARROW_SPEED: f32 = 42.0;
/// The points of health an arrow takes, from the weakest draw to the full one.
const ARROW_DAMAGE: (f32, f32) = (1.0, 9.0);
/// The points of health a bolt takes: more than the best arrow.
pub const BOLT_DAMAGE: u32 = 11;
/// How many creatures a bolt goes through before the one it stops in.
pub const BOLT_PIERCE: u8 = 2;
/// The points of health a throwing axe takes, from the weakest throw to the full one.
const THROWING_AXE_DAMAGE: (f32, f32) = (3.0, 8.0);

/// An arrow, a bolt or a throwing axe a player let fly: the server adds it to the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BowShot {
    /// How far the bow was drawn (`power`).
    pub power: f32,
    /// The points of health the arrow takes from what it hits.
    pub damage: u32,
    /// Whether the arrow came out of an inventory, and may go back into one.
    pub recoverable: bool,
    /// What flies: `ItemId::Arrow`, `ItemId::Bolt` or `ItemId::ThrowingAxe`, and what is
    /// picked up again.
    pub ammo: ItemId,
    /// How many creatures it goes through before the one it stops in.
    pub pierce: u8,
    /// The type of what flies: the modifiers of a splash potion (`alchemy::Brew`).
    pub ammo_type: ItemType,
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
            ammo: ItemId::Arrow,
            pierce: 0,
            ammo_type: ItemType::Generic,
        }
    }

    /// The bolt of a loaded `crossbow`: always at full power, and piercing.
    pub fn bolt(crossbow: &ItemStack, recoverable: bool) -> Self {
        Self {
            power: 1.0,
            damage: BOLT_DAMAGE + crossbow.bonus_arrow_damage(),
            recoverable,
            ammo: ItemId::Bolt,
            pierce: BOLT_PIERCE,
            ammo_type: ItemType::Generic,
        }
    }

    /// A throwing axe swung back for `draw_ms`.
    pub fn throwing_axe(draw_ms: u32, recoverable: bool) -> Self {
        let power = power_of(draw_ms, THROW_FULL_MS);
        let damage =
            THROWING_AXE_DAMAGE.0 + (THROWING_AXE_DAMAGE.1 - THROWING_AXE_DAMAGE.0) * power;
        Self {
            power,
            damage: damage.round() as u32,
            recoverable,
            ammo: ItemId::ThrowingAxe,
            pierce: 0,
            ammo_type: ItemType::Generic,
        }
    }
}

impl BowShot {
    /// A splash potion swung back for `draw_ms`: it hurts nothing, it bursts
    /// (`alchemy::SPLASH_RADIUS`).
    pub fn splash(potion: &ItemStack, draw_ms: u32) -> Self {
        Self {
            power: power_of(draw_ms, THROW_FULL_MS),
            damage: 0,
            recoverable: false,
            ammo: potion.item_id,
            pierce: 0,
            ammo_type: potion.item_type,
        }
    }
}

/// How long `item` takes to be drawn fully, if it is drawn and let fly: the bow, the crossbow
/// and the throwing axe.
pub fn full_draw_ms(item: ItemId) -> Option<u32> {
    match item {
        ItemId::Bow => Some(FULL_DRAW_MS),
        ItemId::Crossbow => Some(CROSSBOW_LOAD_MS),
        ItemId::ThrowingAxe => Some(THROW_FULL_MS),
        item if item.is_splash_potion() => Some(THROW_FULL_MS),
        _ => None,
    }
}

/// What `item`, drawn, lets fly from the inventory.
fn ammo_of(item: ItemId) -> ItemId {
    match item {
        ItemId::Crossbow => ItemId::Bolt,
        ItemId::ThrowingAxe => ItemId::ThrowingAxe,
        item if item.is_splash_potion() => item,
        _ => ItemId::Arrow,
    }
}

/// How far a bow drawn for `draw_ms` is, from 0 to 1: slow at first, then quicker.
pub fn power(draw_ms: u32) -> f32 {
    power_of(draw_ms, FULL_DRAW_MS)
}

/// How far a weapon drawn fully in `full_ms` is after `draw_ms`, from 0 to 1.
pub fn power_of(draw_ms: u32, full_ms: u32) -> f32 {
    let t = (draw_ms as f32 / full_ms as f32).min(1.0);
    (t * t + 2.0 * t) / 3.0
}

/// Which of the three pictures of a drawn bow shows after `draw_ms`, if it is drawn: from 1.
pub fn draw_stage(draw_ms: u32) -> Option<u8> {
    stage(draw_ms, FULL_DRAW_MS)
}

/// Which of the three pictures of `item` shows after `draw_ms`, if it has them and is drawn:
/// the bow's, and the crossbow's, whose third is the loaded one.
pub fn draw_stage_of(item: ItemId, draw_ms: u32) -> Option<u8> {
    match item {
        ItemId::Bow => stage(draw_ms, FULL_DRAW_MS),
        ItemId::Crossbow => stage(draw_ms, CROSSBOW_LOAD_MS),
        _ => None,
    }
}

fn stage(draw_ms: u32, full_ms: u32) -> Option<u8> {
    match draw_ms {
        0 => None,
        ms if ms < full_ms * 2 / 5 => Some(1),
        ms if ms < full_ms * 9 / 10 => Some(2),
        _ => Some(3),
    }
}

/// The slot of the first stack of `ammo` of `player`.
fn ammo_slot(player: &Player, ammo: ItemId) -> Option<u32> {
    let is_ammo = |slot: &u32| {
        let stack = player.inventory.inner.get(slot);
        stack.is_some_and(|stack| stack.item_id == ammo && stack.nb > 0)
    };
    (0..MAX_INVENTORY_SLOTS).find(is_ammo)
}

/// Draws the bow, the crossbow or the throwing axe in the selected slot while the right button
/// is down, and lets it down otherwise: part of the movement, which a client replays. A press a
/// block took (`Player::place_wait_ms`) draws nothing.
pub fn draw_bow(player: &mut Player, action: &PlayerFrameInput) {
    let drawn_ms = player.bow_draw_ms;
    let held = held_item(player, action);
    let full = held.and_then(full_draw_ms);
    let draws = action.is_pressed(NetworkAction::RightClick)
        && !action.is_pressed(NetworkAction::LowerBow)
        && full.is_some()
        && (drawn_ms > 0 || player.place_wait_ms == 0)
        && (player.game_mode.is_creative()
            || held.is_some_and(|held| ammo_slot(player, ammo_of(held)).is_some()));
    player.bow_draw_ms = match (draws, full) {
        (true, Some(full)) => {
            let delta = action.delta_ms.min(MAX_FRAME_DELTA_MS) as u32;
            (drawn_ms + delta).min(full)
        }
        _ => 0,
    };
}

/// Shoots with the weapon in the selected slot when the right button was just released, before
/// `draw_bow` lets it down: the bow or the crossbow wears and an arrow or a bolt is gone, or the
/// throwing axe itself, but in creative. A crossbow shoots only once loaded. A UI opened over the
/// draw (`NetworkAction::LowerBow`) releases nothing.
pub fn shoot_bow(player: &mut Player, action: &PlayerFrameInput) -> Option<BowShot> {
    if player.bow_draw_ms < MIN_DRAW_MS
        || action.is_pressed(NetworkAction::RightClick)
        || action.is_pressed(NetworkAction::LowerBow)
    {
        return None;
    }
    let weapon = *player.inventory.inner.get(&action.hotbar_slot)?;
    let full = full_draw_ms(weapon.item_id)?;
    if weapon.item_id == ItemId::Crossbow && player.bow_draw_ms < full {
        return None;
    }
    let creative = player.game_mode.is_creative();
    if !creative {
        if weapon.item_id == ItemId::ThrowingAxe || weapon.item_id.is_splash_potion() {
            player
                .inventory
                .remove_item_from_stack(action.hotbar_slot, 1);
        } else {
            let slot = ammo_slot(player, ammo_of(weapon.item_id))?;
            player.inventory.remove_item_from_stack(slot, 1);
            player.inventory.wear_tool(action.hotbar_slot, 1);
        }
    }
    Some(match weapon.item_id {
        ItemId::Crossbow => BowShot::bolt(&weapon, !creative),
        ItemId::ThrowingAxe => BowShot::throwing_axe(player.bow_draw_ms, !creative),
        item if item.is_splash_potion() => BowShot::splash(&weapon, player.bow_draw_ms),
        _ => BowShot::new(&weapon, player.bow_draw_ms, !creative),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::GameMode;
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
    fn a_crossbow_shoots_only_once_loaded_a_bolt_that_pierces_and_hurts_more() {
        let mut player = archer(0);
        for (slot, item_id, nb) in [(0, ItemId::Crossbow, 1), (13, ItemId::Bolt, 2)] {
            let item_type = item_id.get_default_type();
            player.inventory.inner.insert(
                slot,
                ItemStack {
                    item_id,
                    item_type,
                    nb,
                },
            );
        }
        assert_eq!(shoot(&mut player, 1000), None, "not loaded yet");
        assert_eq!(player.inventory.inner[&13].nb, 2, "no bolt spent");
        let bolt = shoot(&mut player, 1500).unwrap();
        assert_eq!(bolt.ammo, ItemId::Bolt);
        assert_eq!(bolt.pierce, BOLT_PIERCE);
        assert!(bolt.damage > BowShot::new(&player.inventory.inner[&0], FULL_DRAW_MS, true).damage);
        assert_eq!(player.inventory.inner[&13].nb, 1);
    }

    #[test]
    fn a_throwing_axe_is_thrown_out_of_its_stack() {
        let mut player = archer(0);
        player.inventory.inner.insert(
            0,
            ItemStack {
                item_id: ItemId::ThrowingAxe,
                item_type: ItemId::ThrowingAxe.get_default_type(),
                nb: 2,
            },
        );
        let axe = shoot(&mut player, 800).unwrap();
        assert_eq!(
            (axe.ammo, axe.pierce, axe.power),
            (ItemId::ThrowingAxe, 0, 1.0)
        );
        assert_eq!(player.inventory.inner[&0].nb, 1);
    }

    #[test]
    fn a_splash_potion_is_thrown_with_its_modifiers() {
        let mut player = archer(0);
        let brewed = ItemType::Brewed {
            longer: true,
            stronger: false,
        };
        player.inventory.inner.insert(
            0,
            ItemStack {
                item_id: ItemId::SplashWeaknessPotion,
                item_type: brewed,
                nb: 1,
            },
        );
        let potion = shoot(&mut player, 800).unwrap();
        assert_eq!(
            (potion.ammo, potion.ammo_type, potion.damage),
            (ItemId::SplashWeaknessPotion, brewed, 0)
        );
        assert!(!player.inventory.inner.contains_key(&0));
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
        assert!(ammo_slot(&player, ItemId::Arrow).is_some());
    }
}
