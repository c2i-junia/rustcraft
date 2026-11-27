//! The HUD in the game: the hotbar selection by number keys and trackpad, and the item name. And
//! the HUD spawned again, as a hot patch does.

use bevy::prelude::*;
use shared::world::ItemId;

use super::game::{enter_world, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::chat::ChatRoot;
use crate::ui::hud::food::FoodBar;
use crate::ui::hud::health::HealthBar;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::minimap::MinimapRoot;
use crate::ui::hud::{respawn_hud, HudBottom};

fn selected(h: &mut Harness) -> u32 {
    h.world_mut()
        .query::<&Hotbar>()
        .single(h.world())
        .unwrap()
        .selected
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn number_keys_the_trackpad_and_the_item_name_drive_the_hotbar() {
    let folder = GameFolder::new().with_world("Keys").with_items(
        "Keys",
        "Tester",
        &[(2, ItemId::OakPlanks, 4)],
    );
    let mut h = enter_world(folder, "Keys");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 2) == Some((ItemId::OakPlanks, 4))
    });
    let name = L::text("Oak Planks");
    assert!(!h.is_visible(&name));

    h.press(KeyCode::Digit3);
    assert_eq!(selected(&mut h), 2);
    h.wait_for(&name, SCREEN);

    // An empty slot shows no name
    h.press(KeyCode::Digit9);
    assert_eq!(selected(&mut h), 8);
    assert!(!h.is_visible(&name));

    // A trackpad needs 20 px for a slot: four small scrolls make one, and the selection wraps
    for _ in 0..4 {
        h.scroll_pixels(-5.);
    }
    assert_eq!(selected(&mut h), 0);
    h.scroll(-1.);
    assert_eq!(selected(&mut h), 1);

    // With the inventory open, a digit is for the slot under the cursor, not the selection
    h.press(KeyCode::KeyE);
    h.press(KeyCode::Digit5);
    assert_eq!(selected(&mut h), 1);
    h.press(KeyCode::KeyE);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_hud_spawned_again_shows_what_it_showed_keeps_the_slot_and_the_chat_keeps_its_lines() {
    let mut h = enter_world(GameFolder::new().with_world("Again"), "Again");
    // A new player is in survival: its hearts and drumsticks show once they are drawn
    h.wait_for(&L::marker::<HealthBar>(), SCREEN);
    h.wait_for(&L::marker::<FoodBar>(), SCREEN);
    h.press(KeyCode::Digit4);
    assert_eq!(selected(&mut h), 3);
    let chat = the::<ChatRoot>(&mut h);
    let bottom = the::<HudBottom>(&mut h);

    respawn_hud(h.world_mut());
    h.frames(2);

    // One HUD, a new one, drawn by the systems that draw only what changed
    assert_ne!(the::<HudBottom>(&mut h), bottom);
    // The slot held stays selected
    assert_eq!(selected(&mut h), 3);
    the::<MinimapRoot>(&mut h);
    assert!(h.is_visible(&L::marker::<HealthBar>()));
    assert!(h.is_visible(&L::marker::<FoodBar>()));
    // The chat is not part of it
    assert_eq!(the::<ChatRoot>(&mut h), chat);
}

/// The one entity with `C`, shown or not.
#[track_caller]
fn the<C: Component>(h: &mut Harness) -> Entity {
    let mut entities = h.world_mut().query_filtered::<Entity, With<C>>();
    entities.single(h.world()).unwrap()
}
