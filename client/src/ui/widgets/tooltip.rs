//! The tooltip: one floating panel that shows the `Tooltip` of the element under the cursor.

use accesskit::Role;
use bevy::{a11y::AccessibilityNode, picking::hover::Hovered, prelude::*, window::PrimaryWindow};

use crate::ui::{
    assets::load_font,
    theme::{BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_S, MUTED, PANEL, SPACE_S, TEXT},
};

/// How far the tooltip sits right of and below the cursor, in logical px.
const TOOLTIP_OFFSET: f32 = 12.0;

/// What to show while the cursor is over this element. An empty title shows nothing.
#[derive(Component, Clone, Default, PartialEq, Debug)]
#[require(Hovered)]
pub struct Tooltip {
    pub title: String,
    pub detail: String,
    pub hint: String,
}

/// The one floating panel that shows the tooltips.
#[derive(Component)]
pub struct TooltipRoot;

/// A line of the tooltip: 0 is the title, 1 the detail and 2 the hint.
#[derive(Component)]
struct TooltipLine(usize);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_tooltip)
        .add_systems(Update, show_tooltip);
}

fn spawn_tooltip(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = load_font(&asset_server);
    let line = |index: usize, color: Color| {
        (
            TooltipLine(index),
            Text::new(""),
            TextFont {
                font: font.clone().into(),
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
            Node {
                display: Display::None,
                ..default()
            },
        )
    };
    commands.spawn((
        TooltipRoot,
        // the role before the label: the label's default node must not win
        AccessibilityNode(accesskit::Node::new(Role::Tooltip)),
        GlobalZIndex(100),
        Pickable::IGNORE,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(SPACE_S)),
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor {
            top: BEVEL_HI,
            left: BEVEL_HI,
            bottom: BEVEL_LO,
            right: BEVEL_LO,
        },
        children![line(0, TEXT), line(1, MUTED), line(2, MUTED)],
    ));
}

/// The top-left corner of a tooltip of `size` for a cursor, in logical UI px: right of and below
/// the cursor, flipped to its other side when it would leave the window, then clamped to it.
fn tooltip_position(cursor: Vec2, size: Vec2, window: Vec2) -> Vec2 {
    let place = |cursor: f32, size: f32, window: f32| {
        let after = cursor + TOOLTIP_OFFSET;
        let position = if after + size > window {
            cursor - TOOLTIP_OFFSET - size
        } else {
            after
        };
        position.clamp(0.0, (window - size).max(0.0))
    };
    Vec2::new(
        place(cursor.x, size.x, window.x),
        place(cursor.y, size.y, window.y),
    )
}

/// Shows the tooltip of the first hovered element, or hides the panel.
///
/// The panel stays laid out while hidden. A change of content is written while it is hidden, and
/// the panel is placed and shown on the next frame, from the size that content was laid out with.
fn show_tooltip(
    hovered: Query<(&Tooltip, &Hovered, &InheritedVisibility)>,
    mut root: Single<(&mut Node, &mut Visibility, &ComputedNode, &Children), With<TooltipRoot>>,
    mut lines: Query<(&TooltipLine, &mut Text, &mut Node), Without<TooltipRoot>>,
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
) {
    let tooltip = hovered
        .iter()
        .find(|(tooltip, hovered, visible)| hovered.0 && visible.get() && !tooltip.title.is_empty())
        .map(|(tooltip, ..)| tooltip);
    let (node, visibility, computed, children) = &mut *root;
    let (Some(tooltip), Some(cursor)) = (tooltip, window.cursor_position()) else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };
    let mut up_to_date = true;
    for child in children.iter() {
        let Ok((TooltipLine(index), mut text, mut line)) = lines.get_mut(child) else {
            continue;
        };
        let content = match index {
            0 => &tooltip.title,
            1 => &tooltip.detail,
            _ => &tooltip.hint,
        };
        if text.0 != *content {
            text.0.clone_from(content);
            up_to_date = false;
        }
        let display = if content.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if line.display != display {
            line.display = display;
            up_to_date = false;
        }
    }
    if !up_to_date {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    let size = computed.size() * computed.inverse_scale_factor();
    let position = tooltip_position(
        cursor / ui_scale.0,
        size,
        Vec2::new(window.width(), window.height()) / ui_scale.0,
    );
    let (left, top) = (px(position.x), px(position.y));
    if node.left != left || node.top != top {
        node.left = left;
        node.top = top;
    }
    visibility.set_if_neq(Visibility::Inherited);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tooltip_bigger_than_the_window_sticks_to_its_corner() {
        let window = Vec2::new(800.0, 600.0);
        let huge = Vec2::new(900.0, 700.0);
        assert_eq!(
            tooltip_position(Vec2::new(400.0, 300.0), huge, window),
            Vec2::ZERO
        );
        assert_eq!(
            tooltip_position(Vec2::new(400.0, 300.0), Vec2::ZERO, window),
            Vec2::new(412.0, 312.0)
        );
    }

    #[test]
    fn the_tooltip_stays_inside_the_window() {
        let (window, size) = (Vec2::new(800.0, 600.0), Vec2::new(100.0, 40.0));
        for (cursor, expected) in [
            ((10.0, 10.0), (22.0, 22.0)),
            ((790.0, 300.0), (678.0, 312.0)),
            ((790.0, 590.0), (678.0, 538.0)),
            ((50.0, 590.0), (62.0, 538.0)),
        ] {
            assert_eq!(
                tooltip_position(Vec2::from(cursor), size, window),
                Vec2::from(expected),
                "cursor {cursor:?}"
            );
        }
    }
}
