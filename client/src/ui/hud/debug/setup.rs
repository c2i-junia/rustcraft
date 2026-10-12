use super::loaded_stats::ChunksNumberText;
use super::loaded_stats::TimeText;
use super::targeted_block::BlockText;
use super::{CoordsText, FpsText};
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::{GameState, KeyMap};
use bevy::prelude::*;

/// Marker to find the container entity so we can show/hide the FPS counter
#[derive(Component)]
pub struct HudRoot;

const DEBUG_FONT_SIZE: f32 = 16.0;

/// One line of the debug overlay, showing `text` until its system updates it.
fn debug_line(text: &str, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(DEBUG_FONT_SIZE),
        TextColor(color),
    )
}

/// The debug overlay in the top-left corner: FPS, coordinates, loaded chunks, targeted block
/// and world time, each updated by its own system.
pub fn setup_hud(mut commands: Commands) {
    let root = commands
        .spawn((
            HudRoot,
            DespawnOnExit(GameState::Game),
            // A dark background for readability
            BackgroundColor(Color::BLACK.with_alpha(0.5)),
            // Displayed over all other UI
            GlobalZIndex(i32::MAX),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(1.),
                top: Val::Percent(1.),
                // Sized by the text
                bottom: Val::Auto,
                right: Val::Auto,
                padding: UiRect::all(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .id();

    let text_fps = commands
        .spawn(debug_line("FPS: ", Color::WHITE))
        .with_child((
            FpsText,
            TextSpan::new("N/A"),
            TextFont::from_font_size(DEBUG_FONT_SIZE),
            TextColor(Color::WHITE),
        ))
        .id();
    let coords_text = commands
        .spawn((CoordsText, debug_line("...", Color::WHITE)))
        .id();
    let chunks_number_text = commands
        .spawn((ChunksNumberText, debug_line("...", Color::WHITE)))
        .id();
    let block_text = commands
        .spawn((
            BlockText,
            debug_line("Selected block : ", Color::srgb(0.2, 0.2, 0.2)),
        ))
        .id();
    let time_text = commands
        .spawn((TimeText, debug_line("Time: N/A", Color::WHITE)))
        .id();

    commands.entity(root).add_children(&[
        text_fps,
        coords_text,
        chunks_number_text,
        block_text,
        time_text,
    ]);
}

/// Toggle the FPS counter when pressing F3
pub fn toggle_hud_system(
    mut q: Query<&mut Visibility, With<HudRoot>>,
    kbd: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if !is_action_just_pressed(GameAction::ToggleFps, &kbd, &key_map) {
        return;
    }
    if let Ok(mut vis) = q.single_mut() {
        *vis = match *vis {
            Visibility::Hidden => Visibility::Visible,
            _ => Visibility::Hidden,
        };
    }
}
