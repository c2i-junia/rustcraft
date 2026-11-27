use super::loaded_stats::ChunksNumberText;
use super::loaded_stats::TimeText;
use super::targeted_block::BlockText;
use super::{BiomeText, CoordsText, FpsText, WeatherText};
use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::state::{GameState, KeyMap};
use crate::ui::assets::MONO_FONT_PATH;
use crate::ui::hud::corner::TopLeftColumn;
use crate::ui::hud::UIMode;
use crate::ui::theme;
use bevy::prelude::*;

/// Marker to find the container entity so we can show/hide the FPS counter
#[derive(Component)]
pub struct HudRoot;

/// The font of the overlay, monospaced so that the numbers do not jitter.
pub(crate) fn debug_font(assets: &AssetServer) -> TextFont {
    TextFont {
        font: assets.load(MONO_FONT_PATH).into(),
        ..TextFont::from_font_size(theme::font_s())
    }
}

/// One line of the debug overlay, showing `text` until its system updates it.
pub(crate) fn debug_line(text: &str, color: Color, font: TextFont) -> impl Bundle {
    (Text::new(text), font, TextColor(color))
}

/// The debug overlay in the top-left corner: FPS, coordinates, biome, loaded chunks, targeted
/// block, world time and weather, each updated by its own system.
pub fn setup_hud(
    mut commands: Commands,
    assets: Res<AssetServer>,
    column: Single<Entity, With<TopLeftColumn>>,
) {
    let font = debug_font(&assets);
    let root = commands
        .spawn((
            HudRoot,
            DespawnOnExit(GameState::Game),
            BackgroundColor(theme::panel()),
            // Hidden until F3, and under the HUD panel (1), the held stack (3), the pause menu
            // (5) and the chat
            Visibility::Hidden,
            GlobalZIndex(-1),
            // First in the column, above the effects and the first-step hint
            ChildOf(*column),
            Node {
                // Out of the layout while hidden, so the rest of the column moves up
                display: Display::None,
                padding: UiRect::all(px(theme::space_xs())),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .id();

    let text_fps = commands
        .spawn(debug_line("FPS: ", theme::text(), font.clone()))
        .with_child((
            FpsText,
            TextSpan::new("N/A"),
            font.clone(),
            TextColor(theme::text()),
        ))
        .id();
    let coords_text = commands
        .spawn((CoordsText, debug_line("...", theme::text(), font.clone())))
        .id();
    let biome_text = commands
        .spawn((
            BiomeText,
            debug_line("Biome: N/A", theme::text(), font.clone()),
        ))
        .id();
    let chunks_number_text = commands
        .spawn((
            ChunksNumberText,
            debug_line("...", theme::text(), font.clone()),
        ))
        .id();
    let block_text = commands
        .spawn((
            BlockText,
            debug_line("Selected block : ", theme::text(), font.clone()),
        ))
        .id();
    let time_text = commands
        .spawn((
            TimeText,
            debug_line("Time: N/A", theme::text(), font.clone()),
        ))
        .id();
    let weather_text = commands
        .spawn((
            WeatherText,
            debug_line("Weather: N/A", theme::text(), font.clone()),
        ))
        .id();

    commands.entity(root).add_children(&[
        text_fps,
        coords_text,
        biome_text,
        chunks_number_text,
        block_text,
        time_text,
        weather_text,
    ]);
}

/// Whether the player acts on the world: no dialog is open and nothing is being typed. The run
/// condition of the game keys.
pub fn in_gameplay(ui_mode: Res<UIMode>) -> bool {
    *ui_mode == UIMode::Closed
}

/// Whether the debug overlay is shown: the run condition of its texts, and of the egui
/// inspector, which opens and closes with it.
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

/// Takes the hidden overlay out of the layout, so that it leaves no gap at the top of its
/// column, and puts it back once shown.
pub fn lay_out_debug_overlay(
    mut overlay: Query<(&Visibility, &mut Node), (With<HudRoot>, Changed<Visibility>)>,
) {
    for (visibility, mut node) in &mut overlay {
        node.display = match visibility {
            Visibility::Hidden => Display::None,
            _ => Display::Flex,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn the_overlay_texts_and_the_inspector_run_only_while_the_overlay_is_shown() {
        let mut world = World::new();
        assert!(!world.run_system_once(debug_overlay_shown).unwrap());

        let root = world.spawn((HudRoot, Visibility::Hidden)).id();
        assert!(!world.run_system_once(debug_overlay_shown).unwrap());

        world.entity_mut(root).insert(Visibility::Visible);
        assert!(world.run_system_once(debug_overlay_shown).unwrap());
    }
}
