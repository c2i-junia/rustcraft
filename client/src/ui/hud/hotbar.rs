use bevy::prelude::*;

/// The keys that select a hotbar slot, and name it over a slot of the inventory: `Digit1` is
/// slot 0. Hard-wired, they are not part of the `KeyMap`.
pub const HOTBAR_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// The hotbar slot of the first digit key pressed this frame.
pub fn hotbar_key_index(input: &ButtonInput<KeyCode>) -> Option<u32> {
    HOTBAR_KEYS
        .iter()
        .position(|key| input.just_pressed(*key))
        .map(|index| index as u32)
}

#[derive(Component)]
pub struct Hotbar {
    pub selected: u32,
}
