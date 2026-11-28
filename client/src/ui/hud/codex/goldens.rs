//! Golden screenshots of the Codex, on a page of each kind. See `crate::ui::golden`. They need
//! lavapipe, which is only installed on Linux.

use shared::players::effects::EffectKind;
use shared::world::{ItemId, MobKind};

use super::{show_codex_list, show_codex_page, CodexPage, CodexState, OpenCodex};
use crate::i18n::{in_language, Language};
use crate::ui::golden::UiGolden;
use crate::ui::hud::progress::PlayerProgress;
use crate::ui::widgets::list::hide_idle_scrollbars;

/// The Codex open on `page`, in a window of `width` by `height`.
fn open_codex((width, height): (u32, u32), page: CodexPage) -> UiGolden {
    let mut golden = UiGolden::with_size(width, height);
    golden.world_mut().init_resource::<PlayerProgress>();
    golden.world_mut().init_resource::<CodexState>();
    golden.world_mut().commands().queue(OpenCodex {
        page: Some(page),
        opener: None,
    });
    golden.world_mut().flush();
    // the caret of the search blinks
    if let Some(mut focus) = golden
        .world_mut()
        .get_resource_mut::<bevy::input_focus::InputFocus>()
    {
        focus.clear();
    }
    golden.wait_for_fonts();
    for _ in 0..2 {
        golden.frames(1);
        golden.run_system(show_codex_list);
        golden.run_system(show_codex_page);
        golden.run_system(hide_idle_scrollbars);
    }
    golden
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_wolf_at_1280x720() {
    open_codex((1280, 720), CodexPage::Mob(MobKind::Wolf)).assert_matches("codex_wolf_1280x720");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_furnace_in_french_at_1280x720() {
    in_language(Language::French, || {
        open_codex((1280, 720), CodexPage::Item(ItemId::Furnace))
            .assert_matches("codex_furnace_fr_1280x720");
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_plains_and_poison_at_854x480() {
    open_codex((854, 480), CodexPage::Biome("plains".into()))
        .assert_matches("codex_plains_854x480");
    open_codex((854, 480), CodexPage::Effect(EffectKind::Poison))
        .assert_matches("codex_poison_854x480");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_fishing_guide_at_1280x720() {
    open_codex((1280, 720), CodexPage::Guide("fishing".into()))
        .assert_matches("codex_guide_fishing_1280x720");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_combat_guide_in_french_at_1280x720() {
    in_language(Language::French, || {
        open_codex((1280, 720), CodexPage::Guide("combat".into()))
            .assert_matches("codex_guide_combat_fr_1280x720");
    });
}
