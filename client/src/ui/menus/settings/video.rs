//! The Video tab of the settings: the render distance, applied and saved as soon as it is set,
//! and a note under it, which says when the server of the game grants less.

use bevy::{prelude::*, ui_widgets::ValueChange};
use shared::{GameFolderPaths, MIN_RENDER_DISTANCE};

use super::{SettingsContext, SettingsRoot};
use crate::constants::MAX_CHUNK_RENDER_DISTANCE_RADIUS;
use crate::ui::theme::{FONT_S, MUTED, SPACE_M, SPACE_XS};
use crate::ui::widgets::slider::{spawn_slider, SliderField, SliderScale, LABEL_WIDTH};
use crate::world::{write_render_distance, RenderDistance, ServerRenderDistanceLimit};

/// The slider of the render distance.
#[derive(Component)]
pub struct RenderDistanceSlider;

/// The note under the render distance.
#[derive(Component)]
pub struct RenderDistanceNote;

/// A chunk a step, from the least render distance to the largest the player may choose.
const RENDER_DISTANCE_SCALE: SliderScale = SliderScale {
    min: MIN_RENDER_DISTANCE as f32,
    max: MAX_CHUNK_RENDER_DISTANCE_RADIUS as f32,
    step: 1.,
    whole: true,
    readout: chunks_label,
};

/// The render distance as a number of chunks: "6".
fn chunks_label(value: f32) -> String {
    format!("{}", value.round())
}

/// Spawns the render distance slider and its note under `panel`.
pub fn spawn_video_tab(
    commands: &mut Commands,
    panel: Entity,
    render_distance: &RenderDistance,
    font: Handle<Font>,
) {
    // the slider, and its note under it
    let column = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_XS),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let field = SliderField {
        label: "Render distance",
        value: render_distance.distance as f32,
        scale: RENDER_DISTANCE_SCALE,
    };
    let parts = spawn_slider(commands, &field, font.clone());
    commands.entity(parts.root).insert(ChildOf(column));
    commands
        .entity(parts.slider)
        .insert(RenderDistanceSlider)
        .observe(set_render_distance)
        .observe(save_on_release);
    // under the slider, written by `show_render_distance_note`
    commands.spawn((
        RenderDistanceNote,
        Text::default(),
        TextFont {
            font: font.into(),
            font_size: px(FONT_S).into(),
            ..default()
        },
        TextColor(MUTED),
        Node {
            margin: UiRect::left(px(LABEL_WIDTH + SPACE_M)),
            ..default()
        },
        ChildOf(column),
    ));
}

/// Applies the render distance of the slider, and saves it once the player is done with it. In
/// game, the server is told by `send_render_distance_system`.
fn set_render_distance(
    change: On<ValueChange<f32>>,
    mut render_distance: ResMut<RenderDistance>,
    paths: Res<GameFolderPaths>,
) {
    let distance = change.value.round() as u32;
    if render_distance.distance != distance {
        render_distance.set(distance);
    }
    if change.is_final {
        write_render_distance(&render_distance, &paths);
    }
}

/// Saves the render distance when the button is released on the slider: a click on the track
/// sets a value that the widget never reports as final.
fn save_on_release(
    _: On<Pointer<Release>>,
    render_distance: Res<RenderDistance>,
    paths: Res<GameFolderPaths>,
) {
    write_render_distance(&render_distance, &paths);
}

/// What the note under the render distance says: what the distance is, or that the server of
/// the game grants less, when its `limit` is below the `distance` chosen.
fn render_distance_note(distance: u32, limit: Option<u32>) -> String {
    match limit {
        Some(limit) if limit < distance => format!("Limited to {limit} by this server"),
        _ => "Chunks loaded in every direction around you".to_string(),
    }
}

/// Writes the note under the render distance. The limit of the server is only told in game:
/// from the main menu, no server is joined.
pub(super) fn show_render_distance_note(
    render_distance: Res<RenderDistance>,
    limit: Option<Res<ServerRenderDistanceLimit>>,
    roots: Query<&SettingsRoot>,
    added: Query<(), Added<RenderDistanceNote>>,
    mut notes: Query<&mut Text, With<RenderDistanceNote>>,
) {
    if !(render_distance.is_changed() || !added.is_empty()) {
        return;
    }
    let in_game = roots
        .iter()
        .any(|root| root.context == SettingsContext::Game);
    let limit = limit.filter(|_| in_game).map(|limit| limit.0);
    let note = render_distance_note(render_distance.distance, limit);
    for mut text in &mut notes {
        text.0.clone_from(&note);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_says_when_the_server_grants_less_than_chosen() {
        let plain = "Chunks loaded in every direction around you";
        assert_eq!(
            render_distance_note(8, Some(6)),
            "Limited to 6 by this server"
        );
        assert_eq!(render_distance_note(6, Some(6)), plain);
        assert_eq!(render_distance_note(3, Some(6)), plain);
        assert_eq!(render_distance_note(8, None), plain);
    }

    #[test]
    fn the_render_distance_reads_as_a_whole_number_of_chunks() {
        assert_eq!(chunks_label(6.), "6");
        assert_eq!(chunks_label(5.6), "6");
        assert_eq!(chunks_label(1.), "1");
    }
}
