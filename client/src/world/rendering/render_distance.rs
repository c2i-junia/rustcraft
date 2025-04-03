use crate::{
    constants::DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
    input::{data::GameAction, keyboard::is_action_just_pressed},
    KeyMap,
};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct RenderDistance {
    pub distance: u32,
}

pub fn render_distance_update_system(
    mut render_distance: ResMut<RenderDistance>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if render_distance.distance == 0 {
        render_distance.distance = DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS;
    }

    if is_action_just_pressed(GameAction::RenderDistanceMinus, &keyboard_input, &key_map) {
        render_distance.distance -= 1;
    }

    if is_action_just_pressed(GameAction::RenderDistancePlus, &keyboard_input, &key_map) {
        render_distance.distance += 1;
    }
}
