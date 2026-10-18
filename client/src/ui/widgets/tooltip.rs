//! The tooltip: one floating panel that shows the `Tooltip` of the element under the cursor.

use accesskit::Role;
use bevy::{a11y::AccessibilityNode, picking::hover::Hovered, prelude::*, window::PrimaryWindow};

use crate::ui::theme::{BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_S, MUTED, PANEL, SPACE_S, TEXT};

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

fn spawn_tooltip(mut commands: Commands) {
    let line = |index: usize, color: Color| {
        (
            TooltipLine(index),
            Text::new(""),
            TextFont {
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(color),
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
        Node {
            display: Display::None,
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
fn show_tooltip(
    hovered: Query<(&Tooltip, &Hovered, &InheritedVisibility)>,
    mut root: Single<(&mut Node, &ComputedNode, &Children), With<TooltipRoot>>,
    mut lines: Query<(&TooltipLine, &mut Text, &mut Node), Without<TooltipRoot>>,
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
) {
    let tooltip = hovered
        .iter()
        .find(|(tooltip, hovered, visible)| hovered.0 && visible.get() && !tooltip.title.is_empty())
        .map(|(tooltip, ..)| tooltip);
    let cursor = window.cursor_position();
    let (Some(tooltip), Some(cursor)) = (tooltip, cursor) else {
        hide(&mut root.0);
        return;
    };
    let (node, computed, children) = &mut *root;
    // the size of the previous frame: the panel is laid out once it is shown
    let size = computed.size() * computed.inverse_scale_factor();
    let position = tooltip_position(
        cursor / ui_scale.0,
        size,
        Vec2::new(window.width(), window.height()) / ui_scale.0,
    );
    set_if_neq(&mut node.display, Display::Flex);
    set_if_neq(&mut node.left, px(position.x));
    set_if_neq(&mut node.top, px(position.y));
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
        }
        set_if_neq(
            &mut line.display,
            if content.is_empty() {
                Display::None
            } else {
                Display::Flex
            },
        );
    }
}

fn hide(node: &mut Mut<Node>) {
    set_if_neq(&mut node.display, Display::None);
}

fn set_if_neq<T: PartialEq>(field: &mut T, value: T) {
    if *field != value {
        *field = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
