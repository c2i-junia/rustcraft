//! The end-to-end tests of the widgets in `ui/widgets/`.

use bevy::prelude::*;

use super::harness::{Harness, Options, SCREEN};
use super::locator::Locator as L;
use super::seed::GameFolder;

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_ui_scale_follows_the_window_height() {
    let mut h = Harness::start(
        GameFolder::new(),
        Options {
            width: 1920,
            height: 1080,
            ..default()
        },
    );
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.world().resource::<UiScale>().0, 1.5);
    h.resize(800, 600);
    h.frames(2);
    assert_eq!(h.world().resource::<UiScale>().0, 1.0);
}
