use super::loaded_stats::ChunksNumberText;
use super::loaded_stats::TimeText;
use super::targeted_block::BlockText;
use super::{CoordsText, FpsText};
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::{GameState, KeyMap};
use crate::ui::assets::MONO_FONT_PATH;
use crate::ui::hud::UIMode;
use crate::ui::theme::{FONT_S, PANEL};
use bevy::prelude::*;

/// Marker to find the container entity so we can show/hide the FPS counter
#[derive(Component)]
pub struct HudRoot;

/// The font of the overlay, monospaced so that the numbers do not jitter.
fn debug_font(assets: &AssetServer) -> TextFont {
    TextFont {
        font: assets.load(MONO_FONT_PATH).into(),
        ..TextFont::from_font_size(FONT_S)
    }
}

/// One line of the debug overlay, showing `text` until its system updates it.
fn debug_line(text: &str, color: Color, font: TextFont) -> impl Bundle {
    (Text::new(text), font, TextColor(color))
}

/// The debug overlay in the top-left corner: FPS, coordinates, loaded chunks, targeted block
/// and world time, each updated by its own system.
pub fn setup_hud(mut commands: Commands, assets: Res<AssetServer>) {
    let font = debug_font(&assets);
    let root = commands
        .spawn((
            HudRoot,
            DespawnOnExit(GameState::Game),
            BackgroundColor(PANEL),
            // Hidden until F3, and under the HUD panel (1), the held stack (3), the pause menu
            // (5) and the chat
            Visibility::Hidden,
            GlobalZIndex(-1),
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
        .spawn(debug_line("FPS: ", Color::WHITE, font.clone()))
        .with_child((
            FpsText,
            TextSpan::new("N/A"),
            font.clone(),
            TextColor(Color::WHITE),
        ))
        .id();
    let coords_text = commands
        .spawn((CoordsText, debug_line("...", Color::WHITE, font.clone())))
        .id();
    let chunks_number_text = commands
        .spawn((
            ChunksNumberText,
            debug_line("...", Color::WHITE, font.clone()),
        ))
        .id();
    let block_text = commands
        .spawn((
            BlockText,
            debug_line(
                "Selected block : ",
                Color::srgb(0.2, 0.2, 0.2),
                font.clone(),
            ),
        ))
        .id();
    let time_text = commands
        .spawn((
            TimeText,
            debug_line("Time: N/A", Color::WHITE, font.clone()),
        ))
        .id();

    commands.entity(root).add_children(&[
        text_fps,
        coords_text,
        chunks_number_text,
        block_text,
        time_text,
    ]);
}

/// Whether the player acts on the world: no dialog is open and nothing is being typed. The run
/// condition of the game keys.
pub fn in_gameplay(ui_mode: Res<UIMode>) -> bool {
    *ui_mode == UIMode::Closed
}

/// Whether the debug overlay is shown: the run condition of the egui inspector, which opens
/// and closes with it.
#[cfg(debug_assertions)]
pub fn debug_overlay_shown(overlay: Query<&Visibility, With<HudRoot>>) -> bool {
    overlay
        .iter()
        .any(|visibility| *visibility == Visibility::Visible)
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[cfg(debug_assertions)]
    #[test]
    fn the_inspector_runs_only_while_the_debug_overlay_is_shown() {
        let mut world = World::new();
        assert!(!world.run_system_once(debug_overlay_shown).unwrap());

        let root = world.spawn((HudRoot, Visibility::Hidden)).id();
        assert!(!world.run_system_once(debug_overlay_shown).unwrap());

        world.entity_mut(root).insert(Visibility::Visible);
        assert!(world.run_system_once(debug_overlay_shown).unwrap());
    }
}
