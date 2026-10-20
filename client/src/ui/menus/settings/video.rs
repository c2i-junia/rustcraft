//! The Video tab of the settings: the render distance, applied and saved as soon as it is set,
//! and a note under it, which says when the server of the game grants less; then the clouds,
//! with a note saying what each style draws, and the biome blend.

use bevy::{prelude::*, ui_widgets::ValueChange};
use shared::{GameFolderPaths, MIN_RENDER_DISTANCE};

use super::interface::spawn_row;
use super::{SettingsContext, SettingsRoot};
use crate::constants::MAX_CHUNK_RENDER_DISTANCE_RADIUS;
use crate::ui::theme::{FONT_S, MUTED, SPACE_M, SPACE_XS};
use crate::ui::widgets::segmented::{spawn_segmented, SegmentChange, Segmented};
use crate::ui::widgets::slider::{spawn_slider, SliderField, SliderScale, LABEL_WIDTH};
use crate::world::clouds::mesh::CloudStyle;
use crate::world::{
    write_video_settings, BiomeBlend, RenderDistance, ServerRenderDistanceLimit, MAX_BIOME_BLEND,
};

/// The slider of the render distance.
#[derive(Component)]
pub struct RenderDistanceSlider;

/// The slider of the biome blend.
#[derive(Component)]
pub struct BiomeBlendSlider;

/// The note under the render distance.
#[derive(Component)]
pub struct RenderDistanceNote;

/// The note under the clouds.
#[derive(Component)]
pub struct CloudsNote;

/// The styles of the clouds, in the order of their control.
const CLOUD_STYLES: [CloudStyle; 3] = [CloudStyle::Fancy, CloudStyle::Fast, CloudStyle::Off];

fn clouds_label(style: CloudStyle) -> &'static str {
    match style {
        CloudStyle::Fancy => "Fancy",
        CloudStyle::Fast => "Fast",
        CloudStyle::Off => "Off",
    }
}

/// What the note under the clouds says of `style`.
fn clouds_note(style: CloudStyle) -> &'static str {
    match style {
        CloudStyle::Fancy => "Clouds made of blocks",
        CloudStyle::Fast => "A flat layer of clouds",
        CloudStyle::Off => "No clouds",
    }
}

fn note_font(font: &Handle<Font>) -> TextFont {
    TextFont {
        font: font.clone().into(),
        font_size: px(FONT_S).into(),
        ..default()
    }
}

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

/// A column a step, from no blend to the largest.
const BIOME_BLEND_SCALE: SliderScale = SliderScale {
    min: 0.,
    max: MAX_BIOME_BLEND as f32,
    step: 1.,
    whole: true,
    readout: blend_label,
};

/// The biome blend as the square of columns a column takes the mean of: "5x5", as Minecraft
/// shows it.
fn blend_label(value: f32) -> String {
    let side = BiomeBlend {
        radius: value.round() as u32,
    }
    .side();
    format!("{side}x{side}")
}

/// Spawns the render distance slider and the clouds control, each with its note, then the
/// biome blend slider, under `panel`.
pub fn spawn_video_tab(
    commands: &mut Commands,
    panel: Entity,
    (render_distance, clouds, biome_blend): (&RenderDistance, CloudStyle, &BiomeBlend),
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
    let note_node = || Node {
        margin: UiRect::left(px(LABEL_WIDTH + SPACE_M)),
        ..default()
    };
    commands.spawn((
        RenderDistanceNote,
        Text::default(),
        note_font(&font),
        TextColor(MUTED),
        note_node(),
        ChildOf(column),
    ));

    // the clouds, and their note under them
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
    let row = spawn_row(commands, column, "Clouds", &font);
    let labels = CLOUD_STYLES.map(clouds_label);
    let segmented = Segmented {
        label: "Clouds",
        options: &labels,
        selected: CLOUD_STYLES
            .iter()
            .position(|&style| style == clouds)
            .unwrap_or_default(),
    };
    let group = spawn_segmented(commands, &segmented, font.clone());
    commands
        .entity(group)
        .insert(ChildOf(row))
        .observe(choose_clouds);
    commands.spawn((
        CloudsNote,
        Text::new(clouds_note(clouds)),
        note_font(&font),
        TextColor(MUTED),
        note_node(),
        ChildOf(column),
    ));

    let field = SliderField {
        label: "Biome blend",
        value: biome_blend.radius as f32,
        scale: BIOME_BLEND_SCALE,
    };
    let parts = spawn_slider(commands, &field, font);
    commands.entity(parts.root).insert(ChildOf(panel));
    commands
        .entity(parts.slider)
        .insert(BiomeBlendSlider)
        .observe(set_biome_blend)
        .observe(save_on_release);
}

/// Applies and saves the chosen clouds, and says what they are. In game, the clouds are drawn
/// again at once by `rebuild_clouds_system`.
fn choose_clouds(
    change: On<SegmentChange>,
    mut clouds: ResMut<CloudStyle>,
    render_distance: Res<RenderDistance>,
    biome_blend: Res<BiomeBlend>,
    paths: Res<GameFolderPaths>,
    mut notes: Query<&mut Text, With<CloudsNote>>,
) {
    *clouds = CLOUD_STYLES[change.index];
    write_video_settings(&render_distance, *clouds, &biome_blend, &paths);
    for mut text in &mut notes {
        text.0 = clouds_note(*clouds).to_string();
    }
}

/// Applies the render distance of the slider, and saves it once the player is done with it. In
/// game, the server is told by `send_render_distance_system`.
fn set_render_distance(
    change: On<ValueChange<f32>>,
    mut render_distance: ResMut<RenderDistance>,
    clouds: Res<CloudStyle>,
    biome_blend: Res<BiomeBlend>,
    paths: Res<GameFolderPaths>,
) {
    let distance = change.value.round() as u32;
    if render_distance.distance != distance {
        render_distance.set(distance);
    }
    if change.is_final {
        write_video_settings(&render_distance, *clouds, &biome_blend, &paths);
    }
}

/// Applies the biome blend of the slider, which colours the grass again (`climate.rs`), and
/// saves it once the player is done with it.
fn set_biome_blend(
    change: On<ValueChange<f32>>,
    render_distance: Res<RenderDistance>,
    clouds: Res<CloudStyle>,
    mut biome_blend: ResMut<BiomeBlend>,
    paths: Res<GameFolderPaths>,
) {
    let radius = change.value.round() as u32;
    if biome_blend.radius != radius {
        biome_blend.set(radius);
    }
    if change.is_final {
        write_video_settings(&render_distance, *clouds, &biome_blend, &paths);
    }
}

/// Saves the video settings when the button is released on a slider: a click on the track
/// sets a value that the widget never reports as final.
fn save_on_release(
    _: On<Pointer<Release>>,
    render_distance: Res<RenderDistance>,
    clouds: Res<CloudStyle>,
    biome_blend: Res<BiomeBlend>,
    paths: Res<GameFolderPaths>,
) {
    write_video_settings(&render_distance, *clouds, &biome_blend, &paths);
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
    fn each_style_of_the_clouds_has_its_label_and_note() {
        let labels = CLOUD_STYLES.map(clouds_label);
        assert_eq!(labels, ["Fancy", "Fast", "Off"]);
        let notes = CLOUD_STYLES.map(clouds_note);
        assert_eq!(
            notes,
            [
                "Clouds made of blocks",
                "A flat layer of clouds",
                "No clouds"
            ]
        );
    }

    #[test]
    fn the_biome_blend_reads_as_the_square_of_columns_it_takes() {
        assert_eq!(blend_label(0.), "1x1");
        assert_eq!(blend_label(2.), "5x5");
        assert_eq!(blend_label(6.6), "15x15");
    }

    #[test]
    fn the_render_distance_reads_as_a_whole_number_of_chunks() {
        assert_eq!(chunks_label(6.), "6");
        assert_eq!(chunks_label(5.6), "6");
        assert_eq!(chunks_label(1.), "1");
    }
}
