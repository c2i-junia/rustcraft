//! Drives the client like a player: the mouse, the keyboard and the typed text.

use bevy::{
    input::{
        keyboard::{Key, KeyboardInput, NativeKey, NativeKeyCode},
        mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
        ButtonState,
    },
    prelude::*,
    window::{CursorMoved, PrimaryWindow, WindowEvent},
};

use super::{harness::Harness, locator::Locator};

impl Harness {
    /// Clicks the centre of the element with the left button.
    #[track_caller]
    pub fn click(&mut self, locator: &Locator) {
        self.click_with(locator, MouseButton::Left);
    }

    /// Clicks the centre of the element with the right button.
    #[track_caller]
    pub fn right_click(&mut self, locator: &Locator) {
        self.click_with(locator, MouseButton::Right);
    }

    /// Clicks the centre of `entity` with `button`: one of elements that no locator tells apart.
    pub fn click_entity(&mut self, entity: Entity, button: MouseButton) {
        let center = self.entity_center(entity);
        self.click_point(center, button);
    }

    /// Moves the cursor onto `entity`, for a frame.
    pub fn hover_entity(&mut self, entity: Entity) {
        let center = self.entity_center(entity);
        self.hover_at(center);
    }

    fn entity_center(&self, entity: Entity) -> Vec2 {
        self.world()
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .unwrap()
            .translation
    }

    /// Clicks a point of the window, in physical pixels, with the left button.
    pub fn click_at(&mut self, position: Vec2) {
        self.click_point(position, MouseButton::Left);
    }

    /// Moves the cursor onto the element.
    #[track_caller]
    pub fn hover(&mut self, locator: &Locator) {
        let center = self.center_of(locator);
        self.hover_at(center);
    }

    /// Moves the cursor to a point of the window, in physical pixels, for a frame, without a click.
    pub fn hover_at(&mut self, position: Vec2) {
        self.move_cursor(position);
        self.frames(1);
    }

    /// Presses a key for a frame, then releases it for a frame.
    pub fn press(&mut self, key: KeyCode) {
        self.key_down(key);
        self.key_up(key);
    }

    /// Holds a key down for a frame.
    pub fn key_down(&mut self, key: KeyCode) {
        self.send_key(key, logical_key_of(key), None, ButtonState::Pressed);
    }

    /// Releases a key for a frame.
    pub fn key_up(&mut self, key: KeyCode) {
        self.send_key(key, logical_key_of(key), None, ButtonState::Released);
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

    /// Types `c` from the physical key `key`, as a layout other than QWERTY would: on AZERTY,
    /// `/` is Shift with the physical key `Period`.
    pub fn type_on(&mut self, key: KeyCode, c: char) {
        self.key_down_on(key, c);
        self.key_up_on(key, c);
    }

    /// Holds the physical key `key` down for a frame, as the layout's character `c`.
    pub fn key_down_on(&mut self, key: KeyCode, c: char) {
        let logical = Key::Character(c.to_string().into());
        self.send_key(key, logical, Some(c), ButtonState::Pressed);
    }

    /// Releases the physical key `key` for a frame, as the layout's character `c`: with the
    /// modifiers of the release, which may differ from those of the press.
    pub fn key_up_on(&mut self, key: KeyCode, c: char) {
        let logical = Key::Character(c.to_string().into());
        self.send_key(key, logical, Some(c), ButtonState::Released);
    }

    /// Scrolls the wheel by `lines`, positive going up.
    pub fn scroll(&mut self, lines: f32) {
        self.wheel(MouseScrollUnit::Line, lines);
    }

    /// Scrolls the wheel as a trackpad does, by `pixels`, positive going up.
    pub fn scroll_pixels(&mut self, pixels: f32) {
        self.wheel(MouseScrollUnit::Pixel, pixels);
    }

    fn wheel(&mut self, unit: MouseScrollUnit, y: f32) {
        let window = self.window();
        let wheel = MouseWheel {
            unit,
            x: 0.0,
            y,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        };
        self.world_mut().write_message(wheel);
        // Picking reads the window events only
        self.world_mut()
            .write_message(WindowEvent::MouseWheel(wheel));
        self.frames(1);
    }

    #[track_caller]
    fn click_with(&mut self, locator: &Locator, button: MouseButton) {
        let center = self.center_of(locator);
        self.click_point(center, button);
    }

    fn click_point(&mut self, position: Vec2, button: MouseButton) {
        self.move_cursor(position);
        // One frame for the hover
        self.frames(1);
        for state in [ButtonState::Pressed, ButtonState::Released] {
            self.mouse_button(button, state);
        }
    }

    /// Moves the cursor onto the element and holds the left button down on it.
    #[track_caller]
    pub fn mouse_down(&mut self, locator: &Locator) {
        self.mouse_down_with(locator, MouseButton::Left);
    }

    /// Moves the cursor onto the element and holds `button` down on it.
    #[track_caller]
    pub fn mouse_down_with(&mut self, locator: &Locator, button: MouseButton) {
        self.hover(locator);
        self.button_down(button);
    }

    /// Releases the left button where the cursor is.
    pub fn mouse_up(&mut self) {
        self.button_up(MouseButton::Left);
    }

    /// Presses `button` where the cursor is, for a frame.
    pub fn button_down(&mut self, button: MouseButton) {
        self.mouse_button(button, ButtonState::Pressed);
    }

    /// Releases `button` where the cursor is, for a frame.
    pub fn button_up(&mut self, button: MouseButton) {
        self.mouse_button(button, ButtonState::Released);
    }

    /// Presses or releases a mouse button for a frame.
    fn mouse_button(&mut self, button: MouseButton, state: ButtonState) {
        let window = self.window();
        let input = MouseButtonInput {
            button,
            state,
            window,
        };
        self.world_mut().write_message(input);
        // Picking reads the window events only
        self.world_mut()
            .write_message(WindowEvent::MouseButtonInput(input));
        self.frames(1);
    }

    /// Moves the cursor to `position`, in physical pixels: on the window, for what follows the
    /// cursor (the held stack, the tooltips), and for picking, as a window event in logical pixels.
    pub(super) fn move_cursor(&mut self, position: Vec2) {
        let window = self.window();
        let mut state = self.world_mut().get_mut::<Window>(window).unwrap();
        state.set_physical_cursor_position(Some(position.as_dvec2()));
        let scale_factor = state.scale_factor();
        self.world_mut()
            .write_message(WindowEvent::CursorMoved(CursorMoved {
                window,
                position: position / scale_factor,
                delta: None,
            }));
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

/// The logical key of a physical key, on a QWERTY layout without modifiers: the named keys
/// (Enter, the arrows, the modifiers...), and the letters and digits as their character.
/// Anything else is unidentified.
fn logical_key_of(key: KeyCode) -> Key {
    match key {
        KeyCode::Enter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Tab => Key::Tab,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Key::Shift,
        KeyCode::ControlLeft | KeyCode::ControlRight => Key::Control,
        KeyCode::AltLeft | KeyCode::AltRight => Key::Alt,
        KeyCode::Space => Key::Space,
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        _ => match character_of(key) {
            Some(c) => Key::Character(c.to_string().into()),
            None => Key::Unidentified(NativeKey::Unidentified),
        },
    }
}

/// The character of a letter or digit key, in lower case.
fn character_of(key: KeyCode) -> Option<char> {
    ('a'..='z')
        .chain('0'..='9')
        .find(|&c| key_code_of(c) == key)
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
    use accesskit::Role;
    use bevy::{
        a11y::AccessibilityNode,
        text::EditableText,
        ui::prelude::AccessibleLabel,
        ui_widgets::{Activate, Button as UiButton, ScrollArea},
    };

    use bevy::input_focus::tab_navigation::TabIndex;

    use super::*;
    use crate::e2e::harness::SCREEN;
    use crate::e2e::helpers::{ctrl_n, main_menu};
    use crate::ui::menus::solo::dialogs::WorldNameInput;
    use crate::ui::menus::MenuState;
    use Locator as L;

    /// How many times the probe button was activated.
    #[derive(Resource, Default)]
    struct Activations(u32);

    /// Spawns `probe` in a corner of the screen, above the main menu, and lays it out.
    fn spawn_probe(h: &mut Harness, probe: impl Bundle) -> Entity {
        let root = h
            .world_mut()
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(20.0),
                    ..default()
                },
                GlobalZIndex(1),
            ))
            .id();
        let probe = h.world_mut().spawn((probe, ChildOf(root))).id();
        h.frames(3);
        probe
    }

    fn size(width: f32, height: f32) -> Node {
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            ..default()
        }
    }

    #[test]
    fn logical_keys_are_the_named_keys() {
        let cases = [
            (KeyCode::Enter, Key::Enter),
            (KeyCode::Escape, Key::Escape),
            (KeyCode::Backspace, Key::Backspace),
            (KeyCode::Tab, Key::Tab),
            (KeyCode::ArrowLeft, Key::ArrowLeft),
            (KeyCode::ShiftLeft, Key::Shift),
            (KeyCode::ControlRight, Key::Control),
            (KeyCode::Space, Key::Space),
            (KeyCode::F3, Key::F3),
            (KeyCode::KeyE, Key::Character("e".into())),
            (KeyCode::Digit1, Key::Character("1".into())),
        ];
        for (key_code, logical) in cases {
            assert_eq!(logical_key_of(key_code), logical, "{key_code:?}");
        }
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn clicking_a_widget_button_activates_it() {
        let mut h = main_menu();
        h.world_mut().init_resource::<Activations>();
        let button = spawn_probe(
            &mut h,
            (UiButton, AccessibleLabel::new("Probe"), size(120.0, 40.0)),
        );
        h.world_mut()
            .entity_mut(button)
            .observe(|_: On<Activate>, mut activations: ResMut<Activations>| activations.0 += 1);
        h.click(&L::role(Role::Button, "Probe"));
        assert_eq!(h.world().resource::<Activations>().0, 1);
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn the_wheel_scrolls_a_scroll_area_under_the_cursor() {
        let mut h = main_menu();
        let list = spawn_probe(
            &mut h,
            (
                ScrollArea,
                Name::new("Probe list"),
                Node {
                    overflow: Overflow::scroll_y(),
                    ..size(200.0, 100.0)
                },
                children![size(200.0, 1000.0)],
            ),
        );
        h.hover(&L::name("Probe list"));
        h.scroll(-1.0);
        h.frames(1);
        let position = h.world().get::<ScrollPosition>(list).unwrap();
        assert!(position.y > 0.0, "the list is at {position:?}");
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn named_keys_edit_an_editable_text() {
        let mut h = main_menu();
        let field = spawn_probe(
            &mut h,
            (
                EditableText::default(),
                // the Tab navigation clears the focus of a press on a node with no `TabIndex`
                TabIndex(0),
                AccessibilityNode(accesskit::Node::new(Role::TextInput)),
                AccessibleLabel::new("Probe field"),
                size(200.0, 30.0),
            ),
        );
        h.click(&L::role(Role::TextInput, "Probe field"));
        h.type_text("abcd");
        h.press(KeyCode::Backspace);
        h.press(KeyCode::ArrowLeft);
        h.type_text("x");
        let value = h.world().get::<EditableText>(field).unwrap().value();
        assert_eq!(value.to_string(), "abxc");
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
        ctrl_n(&mut h);
        h.type_text("My world 2");
        assert_eq!(h.value_of::<WorldNameInput>(), "My world 2");
    }
}
