//! The Interface tab of the settings: the UI scale, with a note when the window is too small for
//! it, the FPS counter, the minimap and the first hints, each saved as soon as it is chosen.

use crate::i18n::{tr, trf};
use bevy::{prelude::*, window::PrimaryWindow};
use shared::GameFolderPaths;

use super::{spawn_settings_on, SettingsRoot, SettingsTab};
use crate::i18n::{set_language, Language};
use crate::ui::settings::{
    write_interface_settings, FpsCounterChoice, HintsChoice, InterfaceSettings, MinimapChoice,
    UiScaleChoice,
};
use crate::ui::theme;
use crate::ui::widgets::scale::ui_scale_for;
use crate::ui::widgets::segmented::{spawn_segmented, SegmentChange, Segmented};
use crate::ui::widgets::slider::LABEL_WIDTH;

/// The note saying that the window is too small for the chosen UI scale.
#[derive(Component)]
pub struct ScaleLimitNote;

fn text_font(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone().into(),
        font_size: px(size).into(),
        ..default()
    }
}

/// Spawns a row under `parent` with `label` on the left, as wide as the labels of the sliders,
/// and returns it to take the control.
pub(super) fn spawn_row(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    font: &Handle<Font>,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(theme::space_m()),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(label),
        text_font(font, theme::font_m()),
        TextColor(theme::text()),
        // measured as wrapped on two lines otherwise, which puts it above the middle of the row
        TextLayout::no_wrap(),
        Node {
            width: px(LABEL_WIDTH),
            ..default()
        },
        ChildOf(row),
    ));
    row
}

/// Spawns the UI scale control and its note, then the FPS counter, the minimap and the hints controls,
/// under `panel`.
pub fn spawn_interface_tab(
    commands: &mut Commands,
    panel: Entity,
    settings: &InterfaceSettings,
    font: Handle<Font>,
) {
    let row = spawn_row(commands, panel, tr("interface.language"), &font);
    let labels = Language::ALL.map(Language::native_name);
    let selected = Language::ALL
        .iter()
        .position(|&language| language == settings.language())
        .unwrap_or_default();
    let segmented = Segmented {
        label: tr("interface.language"),
        options: &labels,
        selected,
    };
    let group = spawn_segmented(commands, &segmented, font.clone());
    commands
        .entity(group)
        .insert(ChildOf(row))
        .observe(choose_language);

    // the control, and its note under it
    let column = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_xs()),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let row = spawn_row(commands, column, tr("interface.ui_scale"), &font);
    let labels = UiScaleChoice::ALL.map(UiScaleChoice::label);
    let selected = UiScaleChoice::ALL
        .iter()
        .position(|&choice| choice == settings.ui_scale)
        .unwrap_or_default();
    let segmented = Segmented {
        label: tr("interface.ui_scale"),
        options: &labels,
        selected,
    };
    let group = spawn_segmented(commands, &segmented, font.clone());
    commands
        .entity(group)
        .insert(ChildOf(row))
        .observe(choose_scale);
    // under the control, shown by `show_scale_limit`
    commands.spawn((
        ScaleLimitNote,
        Text::default(),
        text_font(&font, theme::font_s()),
        TextColor(theme::muted()),
        Node {
            display: Display::None,
            margin: UiRect::left(px(LABEL_WIDTH + theme::space_m())),
            ..default()
        },
        ChildOf(column),
    ));

    let row = spawn_row(commands, panel, tr("interface.fps_counter"), &font);
    let labels = FpsCounterChoice::ALL.map(FpsCounterChoice::label);
    let selected = FpsCounterChoice::ALL
        .iter()
        .position(|&choice| choice == settings.fps_counter)
        .unwrap_or_default();
    let segmented = Segmented {
        label: tr("interface.fps_counter"),
        options: &labels,
        selected,
    };
    let group = spawn_segmented(commands, &segmented, font.clone());
    commands
        .entity(group)
        .insert(ChildOf(row))
        .observe(choose_fps_counter);

    let row = spawn_row(commands, panel, tr("interface.minimap"), &font);
    let labels = MinimapChoice::ALL.map(MinimapChoice::label);
    let selected = MinimapChoice::ALL
        .iter()
        .position(|&choice| choice == settings.minimap)
        .unwrap_or_default();
    let segmented = Segmented {
        label: tr("interface.minimap"),
        options: &labels,
        selected,
    };
    let group = spawn_segmented(commands, &segmented, font.clone());
    commands
        .entity(group)
        .insert(ChildOf(row))
        .observe(choose_minimap);

    let row = spawn_row(commands, panel, tr("interface.hints"), &font);
    let labels = HintsChoice::ALL.map(HintsChoice::label);
    let selected = HintsChoice::ALL
        .iter()
        .position(|&choice| choice == settings.hints)
        .unwrap_or_default();
    let segmented = Segmented {
        label: tr("interface.hints"),
        options: &labels,
        selected,
    };
    let group = spawn_segmented(commands, &segmented, font);
    commands
        .entity(group)
        .insert(ChildOf(row))
        .observe(choose_hints);
}

/// Applies and saves the chosen language, and spawns the settings again in it, on this tab. The
/// rest of the UI takes it when it is spawned again.
fn choose_language(
    change: On<SegmentChange>,
    mut settings: ResMut<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
    root: Single<(Entity, &SettingsRoot)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let language = Language::ALL[change.index];
    if language == crate::i18n::language() {
        return;
    }
    settings.language = Some(language);
    set_language(language);
    write_interface_settings(&settings, &paths);
    let (entity, root) = root.into_inner();
    let (context, restore_focus) = (root.context, root.restore_focus);
    commands.entity(entity).despawn();
    let tab = SettingsTab::ALL
        .iter()
        .position(|&tab| tab == SettingsTab::Interface)
        .unwrap_or_default();
    spawn_settings_on(&mut commands, &assets, context, restore_focus, tab);
}

/// Applies and saves the chosen UI scale.
fn choose_scale(
    change: On<SegmentChange>,
    mut settings: ResMut<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
) {
    settings.ui_scale = UiScaleChoice::ALL[change.index];
    write_interface_settings(&settings, &paths);
}

/// Applies and saves what the FPS counter shows.
fn choose_fps_counter(
    change: On<SegmentChange>,
    mut settings: ResMut<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
) {
    settings.fps_counter = FpsCounterChoice::ALL[change.index];
    write_interface_settings(&settings, &paths);
}

/// Applies and saves whether the minimap shows.
fn choose_minimap(
    change: On<SegmentChange>,
    mut settings: ResMut<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
) {
    settings.minimap = MinimapChoice::ALL[change.index];
    write_interface_settings(&settings, &paths);
}

/// Applies and saves whether the hints of the first minutes show.
fn choose_hints(
    change: On<SegmentChange>,
    mut settings: ResMut<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
) {
    settings.hints = HintsChoice::ALL[change.index];
    write_interface_settings(&settings, &paths);
}

/// What the note says for the `choice` of the player in a window of this logical height:
/// nothing when the scale is the one chosen.
fn scale_limit(choice: UiScaleChoice, window_height: f32) -> Option<String> {
    let factor = choice.factor()?;
    let scale = ui_scale_for(choice, window_height);
    (scale < factor).then(|| trf("interface.ui_scale.limited", &[("scale", &scale)]))
}

/// Shows the note while the window is too small for the chosen scale, and says to what the
/// scale is limited.
pub(super) fn show_scale_limit(
    ui_scale: Res<UiScale>,
    settings: Res<InterfaceSettings>,
    window: Single<&Window, With<PrimaryWindow>>,
    added: Query<(), Added<ScaleLimitNote>>,
    mut notes: Query<(&mut Text, &mut Node), With<ScaleLimitNote>>,
) {
    if !(ui_scale.is_changed() || settings.is_changed() || !added.is_empty()) {
        return;
    }
    let limit = scale_limit(settings.ui_scale, window.height());
    for (mut text, mut node) in &mut notes {
        match &limit {
            Some(note) => {
                text.0.clone_from(note);
                node.display = Display::Flex;
            }
            None => node.display = Display::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_says_when_the_window_limits_the_chosen_scale() {
        use UiScaleChoice::*;
        let note = |scale: &str| Some(format!("Limited to {scale} by the window height"));
        assert_eq!(scale_limit(X3, 1080.), note("1.5"));
        assert_eq!(scale_limit(X3, 600.), note("1"));
        assert_eq!(scale_limit(X1_5, 720.), note("1"));
        assert_eq!(scale_limit(X3, 2160.), None);
        assert_eq!(scale_limit(X1, 600.), None);
        assert_eq!(scale_limit(Auto, 600.), None);
    }
}
