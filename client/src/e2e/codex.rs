//! The Codex in the game: F1 opens it, its sections list their pages, a link clicked opens its
//! page, Backspace goes back, and Escape closes it without pausing; the pause menu opens it too,
//! and gets the focus back.

use accesskit::Role;
use bevy::prelude::*;

use super::game::enter_world;
use super::harness::SCREEN;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::codex::{CodexList, CodexPage, CodexPageBody, CodexRoot, CodexState};
use crate::ui::menus::pause::PauseMenu;
use shared::world::ItemId;

fn page(w: &World) -> Option<CodexPage> {
    w.resource::<CodexState>().page.clone()
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn f1_opens_the_codex_whose_links_lead_from_page_to_page() {
    let mut h = enter_world(GameFolder::new().with_world("Codex"), "Codex");
    h.press(KeyCode::F1);
    h.wait_for(&L::marker::<CodexRoot>(), SCREEN);
    h.snapshot("items");

    h.click(&L::role(Role::Button, "Biomes"));
    h.wait_for(&L::text("Plains").within(L::marker::<CodexList>()), SCREEN);
    h.click(&L::text("Plains").within(L::marker::<CodexList>()));
    h.wait_for(&L::text_containing("Open grassland"), SCREEN);
    h.snapshot("plains");

    // the ground of the plains is a link to its item
    // its cell is wider than the word, which starts at its left
    let grass = h.rect_of(&L::text("Grass").within(L::marker::<CodexPageBody>()));
    h.click_at(Vec2::new(grass.min.x + 8.0, grass.center().y));
    h.wait_until("the page of grass", SCREEN, |w| {
        page(w) == Some(CodexPage::Item(ItemId::Grass))
    });
    h.snapshot("grass");
    h.press(KeyCode::Backspace);
    h.wait_until("back on the plains", SCREEN, |w| {
        page(w) == Some(CodexPage::Biome("plains".into()))
    });

    // Escape closes it, and does not pause
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(h.find(&L::marker::<CodexRoot>()).is_empty());
    assert!(h.find(&L::marker::<PauseMenu>()).is_empty());

    // from the pause menu, which gets the focus back on its button
    h.press(KeyCode::Escape);
    h.wait_for(&L::marker::<PauseMenu>(), SCREEN);
    h.click(&L::role(Role::Button, "Codex"));
    h.wait_for(&L::marker::<CodexRoot>(), SCREEN);
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(h.find(&L::marker::<CodexRoot>()).is_empty());
    assert!(h.is_visible(&L::marker::<PauseMenu>()));
    assert_eq!(h.focus_label().as_deref(), Some("Codex"));
}
