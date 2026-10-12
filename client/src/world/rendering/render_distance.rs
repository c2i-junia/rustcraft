use crate::{
    constants::DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
    input::{data::GameAction, keyboard::is_action_just_pressed},
    state::KeyMap,
    ui::hud::UIMode,
};
use bevy::prelude::*;

/// How far away, in chunks, the mobs are drawn.
#[derive(Resource, Reflect)]
pub struct RenderDistance {
    pub distance: u32,
}

const MIN_RENDER_DISTANCE: u32 = 1;

impl Default for RenderDistance {
    fn default() -> Self {
        Self {
            distance: DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
        }
    }
}

impl RenderDistance {
    fn decrease(&mut self) {
        self.distance = self.distance.saturating_sub(1).max(MIN_RENDER_DISTANCE);
    }

    fn increase(&mut self) {
        self.distance = self.distance.saturating_add(1);
    }
}

pub fn render_distance_update_system(
    mut render_distance: ResMut<RenderDistance>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
) {
    if *ui_mode != UIMode::Closed {
        return;
    }

    if is_action_just_pressed(GameAction::RenderDistanceMinus, &keyboard_input, &key_map) {
        render_distance.decrease();
    }

    if is_action_just_pressed(GameAction::RenderDistancePlus, &keyboard_input, &key_map) {
        render_distance.increase();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distance_starts_at_the_default_and_never_goes_below_one_chunk() {
        let mut render_distance = RenderDistance::default();
        assert_eq!(
            render_distance.distance,
            DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS
        );

        for _ in 0..DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS + 3 {
            render_distance.decrease();
        }
        assert_eq!(render_distance.distance, MIN_RENDER_DISTANCE);

        render_distance.increase();
        assert_eq!(render_distance.distance, MIN_RENDER_DISTANCE + 1);
    }
}
