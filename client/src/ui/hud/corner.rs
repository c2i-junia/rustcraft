//! The top corners of the game, each one column so that what they hold never overlaps: at the
//! right the minimap, then the FPS counter, with the toasts below them; at the left the debug
//! overlay (F3), then the status effects, then the hint of the first steps.

use bevy::prelude::*;

use crate::state::GameState;
use crate::ui::theme;
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
            row_gap: px(theme::space_s()),
            ..default()
        },
    ));
}

/// The column at the top-left of the game.
#[derive(Component)]
pub struct TopLeftColumn;

/// Spawns the column, which the debug overlay, the effects and the first-step hint go into.
pub fn spawn_top_left_column(mut commands: Commands) {
    commands.spawn((
        TopLeftColumn,
        DespawnOnExit(GameState::Game),
        // under the panels, like the column at the right
        GlobalZIndex(-1),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(1.),
            top: Val::Percent(1.),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: px(theme::space_s()),
            ..default()
        },
    ));
}
