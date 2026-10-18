//! The themed slider: a label, a sunken track with an accent fill and a raised thumb, and a
//! percentage, for a value from 0 to 1.
//!
//! A click on the track sets the value there, the arrows and the wheel move it by `SLIDER_STEP`;
//! the wheel only moves the slider under the cursor. Callers observe `On<ValueChange<f32>>` on
//! the `slider` of `spawn_slider`; `is_final` is false while it is dragged.

use bevy::{
    input_focus::tab_navigation::TabIndex,
    picking::hover::Hovered,
    prelude::*,
    text::Justify,
    ui::UiSystems,
    ui_widgets::{
        slider_self_update, SetSliderValue, Slider, SliderRange, SliderStep, SliderThumb,
        SliderValue, SliderValueChange, TrackClick,
    },
};

use super::{focus::FocusRing, text_field::sunken_bevel};
use crate::ui::theme::{
    ACCENT, BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_M, RAISE, SLOT, SPACE_M, TEXT,
};

/// How much an arrow key or a notch of the wheel moves a slider.
pub const SLIDER_STEP: f32 = 0.05;
const LABEL_WIDTH: f32 = 180.0;
const TRACK_WIDTH: f32 = 240.0;
const TRACK_HEIGHT: f32 = 20.0;
const THUMB_WIDTH: f32 = 12.0;
const READOUT_WIDTH: f32 = 64.0;

/// What `spawn_slider` builds.
// The settings screen (Lot 1, Task 6) is the first user of the slider
#[allow(dead_code)]
pub struct SliderField<'a> {
    /// The label shown before the slider, which is also its accessible label.
    pub label: &'a str,
    /// From 0 to 1.
    pub value: f32,
}

/// The entities of a slider: `root` to place it, `slider` to observe it.
// The settings screen (Lot 1, Task 6) is the first user of the slider
#[allow(dead_code)]
pub struct SliderParts {
    pub root: Entity,
    pub slider: Entity,
}

/// The percentage shown after a slider.
#[derive(Component)]
pub struct SliderReadout;

/// The parts of a slider that follow its value, on the `slider`.
#[derive(Component)]
struct SliderLook {
    fill: Entity,
    thumb: Entity,
    readout: Entity,
}

/// Spawns a slider row, without a parent: put `root` in place.
// The settings screen (Lot 1, Task 6) is the first user of the slider
#[allow(dead_code)]
pub fn spawn_slider(
    commands: &mut Commands,
    field: &SliderField,
    font: Handle<Font>,
) -> SliderParts {
    let text_font = TextFont {
        font: font.into(),
        font_size: px(FONT_M).into(),
        ..default()
    };
    let root = commands
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(SPACE_M),
            ..default()
        })
        .id();
    commands.spawn((
        Text::new(field.label),
        text_font.clone(),
        TextColor(TEXT),
        Node {
            width: px(LABEL_WIDTH),
            ..default()
        },
        ChildOf(root),
    ));
    let slider = commands
        .spawn((
            // the widget before the label: its `Role::Slider` must win over the label's node
            Slider {
                track_click: TrackClick::Snap,
                ..default()
            },
            SliderValue(field.value),
            SliderRange::new(0., 1.),
            SliderStep(SLIDER_STEP),
            TabIndex(0),
            FocusRing,
            Hovered::default(),
            AccessibleLabel::new(field.label),
            Node {
                width: px(TRACK_WIDTH),
                height: px(TRACK_HEIGHT),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(SLOT),
            sunken_bevel(),
            ChildOf(root),
        ))
        .observe(slider_self_update)
        .observe(wheel_steps)
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                height: percent(100),
                ..default()
            },
            BackgroundColor(ACCENT),
            ChildOf(slider),
        ))
        .id();
    let thumb = commands
        .spawn((
            SliderThumb,
            // over the bevel of the track: the widget measures the track with its border
            Node {
                position_type: PositionType::Absolute,
                top: px(-BEVEL_WIDTH),
                bottom: px(-BEVEL_WIDTH),
                width: px(THUMB_WIDTH),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(RAISE),
            BorderColor {
                top: BEVEL_HI,
                left: BEVEL_HI,
                bottom: BEVEL_LO,
                right: BEVEL_LO,
            },
            ChildOf(slider),
        ))
        .id();
    let readout = commands
        .spawn((
            SliderReadout,
            Text::new(percent_label(field.value)),
            text_font,
            TextColor(TEXT),
            TextLayout::justify(Justify::Right),
            Node {
                width: px(READOUT_WIDTH),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.entity(slider).insert(SliderLook {
        fill,
        thumb,
        readout,
    });
    SliderParts { root, slider }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, place_slider_parts.before(UiSystems::Layout));
}

/// The value as a rounded percentage: "55%".
fn percent_label(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

/// A notch of the wheel over the slider moves it by a step, up raising it, and nothing else
/// scrolls.
fn wheel_steps(mut scroll: On<Pointer<Scroll>>, mut commands: Commands) {
    // a horizontal scroll is for an ancestor
    if scroll.y == 0.0 {
        return;
    }
    scroll.propagate(false);
    commands.trigger(SetSliderValue {
        entity: scroll.entity,
        change: SliderValueChange::RelativeStep(scroll.y.signum()),
    });
}

/// Places the fill and the thumb of each slider whose value changed, and writes its readout.
/// The thumb travels the whole track, bevel included, minus its own width, as the widget assumes
/// for a click or a drag; it is placed from the inner edge of the bevel, as absolute nodes are.
fn place_slider_parts(
    sliders: Query<(&SliderValue, &SliderRange, &SliderLook), Changed<SliderValue>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (value, range, look) in &sliders {
        let position = range.thumb_position(value.0).clamp(0.0, 1.0);
        if let Ok(mut fill) = nodes.get_mut(look.fill) {
            fill.width = percent(position * 100.0);
        }
        if let Ok(mut thumb) = nodes.get_mut(look.thumb) {
            thumb.left = percent(position * 100.0);
            // left = position x (inner + 2 bevels - thumb) - bevel, where inner is the 100 %
            thumb.margin.left = px(position * (2.0 * BEVEL_WIDTH - THUMB_WIDTH) - BEVEL_WIDTH);
        }
        if let Ok(mut readout) = texts.get_mut(look.readout) {
            readout.0 = percent_label(value.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percentage_is_rounded() {
        assert_eq!(percent_label(0.549999), "55%");
        assert_eq!(percent_label(0.), "0%");
        assert_eq!(percent_label(1.), "100%");
    }
}
