//! The top-right corner of the game: one column holding the minimap, then the FPS counter, so
//! that they never overlap, with the toasts below them.

use bevy::prelude::*;

use crate::state::GameState;
use crate::ui::theme::SPACE_S;
use crate::ui::widgets::toast::ToastsBelow;

/// The column at the top-right of the game.
#[derive(Component)]
pub struct TopRightColumn;

/// Spawns the column, which the minimap and the FPS counter go into.
pub fn spawn_top_right_column(mut commands: Commands) {
    commands.spawn((
        TopRightColumn,
        ToastsBelow,
        DespawnOnExit(GameState::Game),
        // under the panels, like the debug overlay
        GlobalZIndex(-1),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Percent(1.),
            top: Val::Percent(1.),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(SPACE_S),
            ..default()
        },
    ));
}
