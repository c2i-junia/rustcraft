//! The achievements: the toast of one just earned, and the screen over the pause menu.

use accesskit::Role;
use bevy::prelude::*;
use shared::players::progress::Achievement;
use shared::world::{ItemId, ItemStack};

use super::game::enter_world_with;
use super::harness::{Options, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::progress::PlayerProgress;
use crate::ui::menus::pause::PauseMenu;
use crate::ui::menus::progress::ProgressRoot;

const SIZES: [(u32, u32); 2] = [(1280, 720), (800, 600)];

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_achievement_earned_is_told_then_listed_with_the_others() {
    let folder = GameFolder::new()
        .with_world("Feats")
        .with_player("Feats", "Tester", |player| {
            player.progress.achievements = vec![Achievement::Tool, Achievement::Night];
            let item_id = ItemId::IronIngot;
            let item_type = item_id.get_default_type();
            let nb = 1;
            let stack = ItemStack {
                item_id,
                item_type,
                nb,
            };
            player.inventory.inner.insert(0, stack);
        });
    let mut h = enter_world_with(folder, "Feats", Options::default());
    h.wait_for(&L::text("Achievement: Worth the Climb Down"), SERVER);
    for (width, height) in SIZES {
        h.resize(width, height);
        h.snapshot("toast");
    }
    h.wait_until("the progress", SERVER, |w| {
        let progress = w.resource::<PlayerProgress>().0.as_ref();
        progress.is_some_and(|progress| progress.achievements.len() == 3)
    });
    h.resize(1280, 720);
    h.press(KeyCode::Escape);
    h.wait_for(&L::marker::<PauseMenu>(), SCREEN);
    for (width, height) in SIZES {
        h.resize(width, height);
        h.snapshot("pause");
    }
    h.click(&L::role(Role::Button, "Progress"));
    h.wait_for(&L::marker::<ProgressRoot>(), SCREEN);
    h.wait_for(&L::text("3 of 21 earned"), SCREEN);
    h.assert_focus_rings();
    for (width, height) in SIZES {
        h.resize(width, height);
        h.snapshot("achievements");
    }
    h.press(KeyCode::Escape);
    assert!(h.find(&L::marker::<ProgressRoot>()).is_empty());
    assert_eq!(h.focus_label().as_deref(), Some("Progress"));
}
