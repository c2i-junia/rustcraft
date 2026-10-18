//! Drives the client like a player: the mouse, the keyboard and the typed text.

use bevy::{
    input::{
        keyboard::{Key, KeyboardInput, NativeKey, NativeKeyCode},
        mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
        ButtonState,
    },
    prelude::*,
    window::PrimaryWindow,
};

use super::{harness::Harness, locator::Locator};

impl Harness {
    /// Clicks the centre of the element with the left button.
    pub fn click(&mut self, locator: &Locator) {
        self.click_with(locator, MouseButton::Left);
    }

    pub fn right_click(&mut self, locator: &Locator) {
        self.click_with(locator, MouseButton::Right);
    }

    /// Moves the cursor onto the element.
    pub fn hover(&mut self, locator: &Locator) {
        let center = self.center_of(locator);
        self.move_cursor(center);
        self.frames(1);
    }

    /// Presses a key for a frame, then releases it for a frame.
    pub fn press(&mut self, key: KeyCode) {
        self.key_down(key);
        self.key_up(key);
    }

    /// Holds a key down for a frame.
    pub fn key_down(&mut self, key: KeyCode) {
        self.send_key(
            key,
            Key::Unidentified(NativeKey::Unidentified),
            None,
            ButtonState::Pressed,
        );
    }

    /// Releases a key for a frame.
    pub fn key_up(&mut self, key: KeyCode) {
        self.send_key(
            key,
            Key::Unidentified(NativeKey::Unidentified),
            None,
            ButtonState::Released,
        );
    }

    /// Types the text one character at a time, into whatever has the focus.
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let key_code = key_code_of(c);
            let logical = if c == ' ' {
                Key::Space
            } else {
                Key::Character(c.to_string().into())
            };
            for state in [ButtonState::Pressed, ButtonState::Released] {
                self.send_key(key_code, logical.clone(), Some(c), state);
            }
        }
    }

    /// Scrolls the wheel by `lines`, positive going up.
    pub fn scroll(&mut self, lines: f32) {
        let window = self.window();
        self.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: lines,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        self.frames(1);
    }

    fn click_with(&mut self, locator: &Locator, button: MouseButton) {
        let center = self.center_of(locator);
        self.move_cursor(center);
        // One frame for the hover
        self.frames(1);
        let window = self.window();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            self.world_mut().write_message(MouseButtonInput {
                button,
                state,
                window,
            });
            self.frames(1);
        }
    }

    fn move_cursor(&mut self, position: Vec2) {
        let window = self.window();
        self.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_physical_cursor_position(Some(position.as_dvec2()));
    }

    fn send_key(
        &mut self,
        key_code: KeyCode,
        logical_key: Key,
        text: Option<char>,
        state: ButtonState,
    ) {
        let window = self.window();
        self.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state,
            text: text.map(|c| c.to_string().into()),
            repeat: false,
            window,
        });
        self.frames(1);
    }

    fn window(&mut self) -> Entity {
        self.world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(self.world())
            .unwrap()
    }
}

/// The physical key of an ASCII letter or digit.
fn key_code_of(c: char) -> KeyCode {
    match c.to_ascii_lowercase() {
        'a' => KeyCode::KeyA,
        'b' => KeyCode::KeyB,
        'c' => KeyCode::KeyC,
        'd' => KeyCode::KeyD,
        'e' => KeyCode::KeyE,
        'f' => KeyCode::KeyF,
        'g' => KeyCode::KeyG,
        'h' => KeyCode::KeyH,
        'i' => KeyCode::KeyI,
        'j' => KeyCode::KeyJ,
        'k' => KeyCode::KeyK,
        'l' => KeyCode::KeyL,
        'm' => KeyCode::KeyM,
        'n' => KeyCode::KeyN,
        'o' => KeyCode::KeyO,
        'p' => KeyCode::KeyP,
        'q' => KeyCode::KeyQ,
        'r' => KeyCode::KeyR,
        's' => KeyCode::KeyS,
        't' => KeyCode::KeyT,
        'u' => KeyCode::KeyU,
        'v' => KeyCode::KeyV,
        'w' => KeyCode::KeyW,
        'x' => KeyCode::KeyX,
        'y' => KeyCode::KeyY,
        'z' => KeyCode::KeyZ,
        '0' => KeyCode::Digit0,
        '1' => KeyCode::Digit1,
        '2' => KeyCode::Digit2,
        '3' => KeyCode::Digit3,
        '4' => KeyCode::Digit4,
        '5' => KeyCode::Digit5,
        '6' => KeyCode::Digit6,
        '7' => KeyCode::Digit7,
        '8' => KeyCode::Digit8,
        '9' => KeyCode::Digit9,
        _ => KeyCode::Unidentified(NativeKeyCode::Unidentified),
    }
}

#[cfg(test)]
mod tests {
    use bevy_simple_text_input::TextInputValue;

    use super::*;
    use crate::e2e::harness::SCREEN;
    use crate::ui::button::MenuState;
    use crate::ui::menus::solo::WorldNameInput;

    fn main_menu() -> Harness {
        let mut h = Harness::new();
        h.wait_until("the main menu", SCREEN, |w| {
            *w.resource::<State<MenuState>>().get() == MenuState::Main
        });
        h.frames(3);
        h
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn clicking_a_button_presses_it() {
        let mut h = main_menu();
        h.click(&Locator::text("Settings"));
        assert_eq!(h.menu_state(), MenuState::Settings);
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn typed_text_goes_to_the_focused_input() {
        let mut h = main_menu();
        h.click(&Locator::text("Singleplayer"));
        h.wait_until("the solo screen", SCREEN, |w| {
            *w.resource::<State<MenuState>>().get() == MenuState::Solo
        });
        h.frames(3);
        h.type_text("My world 2");
        let input = h.get(&Locator::marker::<WorldNameInput>());
        let value = h.world().get::<TextInputValue>(input).unwrap();
        assert_eq!(value.0, "My world 2");
    }
}
