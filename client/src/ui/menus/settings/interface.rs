//! The Interface tab of the settings: the UI scale, saved as soon as it is chosen, and a note
//! when the window is too small for it.

use bevy::{prelude::*, window::PrimaryWindow};
use shared::GameFolderPaths;

use crate::ui::settings::{write_interface_settings, InterfaceSettings, UiScaleChoice};
use crate::ui::theme::{FONT_M, FONT_S, MUTED, SPACE_M, SPACE_XS, TEXT};
use crate::ui::widgets::scale::ui_scale_for;
use crate::ui::widgets::segmented::{spawn_segmented, SegmentChange, Segmented};
use crate::ui::widgets::slider::LABEL_WIDTH;

/// The note saying that the window is too small for the chosen UI scale.
#[derive(Component)]
pub struct ScaleLimitNote;

/// Spawns the UI scale control and its note under `panel`.
pub fn spawn_interface_tab(
    commands: &mut Commands,
    panel: Entity,
    settings: &InterfaceSettings,
    font: Handle<Font>,
) {
    let text_font = |size: f32| TextFont {
        font: font.clone().into(),
        font_size: px(size).into(),
        ..default()
    };
    // the control, and its note under it
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
    // the label on the left, as wide as the labels of the sliders
    let row = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(SPACE_M),
                ..default()
            },
            ChildOf(column),
        ))
        .id();
    commands.spawn((
        Text::new("UI scale"),
        text_font(FONT_M),
        TextColor(TEXT),
        // measured as wrapped on two lines otherwise, which puts it above the middle of the row
        TextLayout::no_wrap(),
        Node {
            width: px(LABEL_WIDTH),
            ..default()
        },
        ChildOf(row),
    ));
    let labels = UiScaleChoice::ALL.map(UiScaleChoice::label);
    let selected = UiScaleChoice::ALL
        .iter()
        .position(|&choice| choice == settings.ui_scale)
        .unwrap_or_default();
    let segmented = Segmented {
        label: "UI scale",
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
        text_font(FONT_S),
        TextColor(MUTED),
        Node {
            display: Display::None,
            margin: UiRect::left(px(LABEL_WIDTH + SPACE_M)),
            ..default()
        },
        ChildOf(column),
    ));
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

/// What the note says for the `choice` of the player in a window of this logical height:
/// nothing when the scale is the one chosen.
fn scale_limit(choice: UiScaleChoice, window_height: f32) -> Option<String> {
    let factor = choice.factor()?;
    let scale = ui_scale_for(choice, window_height);
    (scale < factor).then(|| format!("Limited to {scale} by the window height"))
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
