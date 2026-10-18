//! Steps shared by the tests of the screens.

use bevy::{prelude::Entity, ui::UiGlobalTransform};

use super::harness::{Harness, SCREEN};
use super::locator::Locator as L;

impl Harness {
    pub fn click_when_ready(&mut self, text: &str) {
        self.wait_for(&L::text(text), SCREEN);
        self.click(&L::text(text));
    }

    /// Waits until the icons of a row are laid out side by side: they are only borders until
    /// their images are loaded, and a click would then hit the wrong one.
    pub fn wait_for_icons(&mut self, world: &str) {
        let play = self.wait_for_named(&format!("Play {world}"));
        let delete = self.wait_for_named(&format!("Delete {world}"));
        self.wait_until("the icons of the row", SCREEN, |w| {
            let x = |e| w.get::<UiGlobalTransform>(e).unwrap().translation.x;
            x(delete) - x(play) > 20.0
        });
    }

    pub fn wait_for_named(&mut self, name: &str) -> Entity {
        self.wait_for(&L::name(name), SCREEN);
        self.get(&L::name(name))
    }
}
