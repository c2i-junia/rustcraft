//! The tooltip: one floating panel that shows the `Tooltip` of the element under the cursor, or
//! of the focused element while the focus ring shows.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{InputFocus, InputFocusVisible},
    picking::hover::Hovered,
    prelude::*,
    ui::UiGlobalTransform,
    window::PrimaryWindow,
};

use crate::ui::{
    assets::load_font,
    theme::{BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_S, MUTED, PANEL, SPACE_S, TEXT},
};

/// How far the tooltip sits right of and below the cursor or the focused element, in logical px.
const TOOLTIP_OFFSET: f32 = 12.0;

/// What to show while the cursor is over this element, or while it has the focus ring. An empty
/// title shows nothing.
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

/// The top-left corner of a tooltip of `size` next to `anchor`, the cursor or the focused element,
/// in logical UI px: right of and below it, flipped to its other side when it would leave the
/// window, then clamped to it.
fn tooltip_position(anchor: Rect, size: Vec2, window: Vec2) -> Vec2 {
    let place = |min: f32, max: f32, size: f32, window: f32| {
        let after = max + TOOLTIP_OFFSET;
        let position = if after + size > window {
            min - TOOLTIP_OFFSET - size
        } else {
            after
        };
        position.clamp(0.0, (window - size).max(0.0))
    };
    Vec2::new(
        place(anchor.min.x, anchor.max.x, size.x, window.x),
        place(anchor.min.y, anchor.max.y, size.y, window.y),
    )
}

/// Shows the tooltip of the focused element next to it while the focus ring shows, else the
/// tooltip of the first hovered element next to the cursor, or hides the panel.
///
/// The panel stays laid out while hidden. A change of content is written while it is hidden, and
/// the panel is placed and shown on the next frame, from the size that content was laid out with.
fn show_tooltip(
    tooltips: Query<(
        &Tooltip,
        &Hovered,
        &InheritedVisibility,
        Option<(&ComputedNode, &UiGlobalTransform)>,
    )>,
    focus: Res<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
    mut root: Single<(&mut Node, &mut Visibility, &ComputedNode, &Children), With<TooltipRoot>>,
    mut lines: Query<(&TooltipLine, &mut Text, &mut Node), Without<TooltipRoot>>,
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
) {
    let shown = |tooltip: &Tooltip, visible: &InheritedVisibility| {
        visible.get() && !tooltip.title.is_empty()
    };
    let focused = focus
        .get()
        .filter(|_| focus_visible.0)
        .and_then(|focused| tooltips.get(focused).ok());
    // Next to the focused element, or next to the cursor, in logical UI px
    let shown_tooltip = match focused {
        Some((tooltip, _, visible, node)) => {
            node.filter(|_| shown(tooltip, visible))
                .map(|(node, transform)| {
                    let scale = node.inverse_scale_factor();
                    let rect =
                        Rect::from_center_size(transform.translation * scale, node.size() * scale);
                    (tooltip, rect)
                })
        }
        None => tooltips
            .iter()
            .find(|(tooltip, hovered, visible, _)| hovered.0 && shown(tooltip, visible))
            .zip(window.cursor_position())
            .map(|((tooltip, ..), cursor)| {
                let cursor = cursor / ui_scale.0;
                (tooltip, Rect::from_center_size(cursor, Vec2::ZERO))
            }),
    };
    let (node, visibility, computed, children) = &mut *root;
    let Some((tooltip, anchor)) = shown_tooltip else {
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
        anchor,
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
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input_focus::{InputFocus, InputFocusVisible};
    use bevy::math::Affine2;

    /// A window of 800 by 600 with the cursor at its centre, over an element whose tooltip says
    /// "Hovered", and a 40 px cell from (100, 100) whose tooltip says "Focused".
    fn world_with_two_tooltips() -> (World, Entity) {
        let mut world = World::new();
        let mut window = Window {
            resolution: (800, 600).into(),
            ..default()
        };
        window.set_cursor_position(Some(Vec2::new(400.0, 300.0)));
        world.spawn((window, PrimaryWindow));
        world.init_resource::<UiScale>();
        world.init_resource::<InputFocus>();
        world.init_resource::<InputFocusVisible>();
        let line = |index: usize| (TooltipLine(index), Text::new(""), Node::default());
        world.spawn((
            TooltipRoot,
            Node::default(),
            Visibility::Hidden,
            ComputedNode {
                size: Vec2::new(100.0, 40.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            children![line(0), line(1), line(2)],
        ));
        let tooltip = |title: &str| Tooltip {
            title: title.into(),
            ..default()
        };
        world.spawn((
            tooltip("Hovered"),
            Hovered(true),
            InheritedVisibility::VISIBLE,
        ));
        let cell = world
            .spawn((
                tooltip("Focused"),
                InheritedVisibility::VISIBLE,
                ComputedNode {
                    size: Vec2::splat(40.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from(Affine2::from_translation(Vec2::splat(120.0))),
            ))
            .id();
        world.insert_resource(InputFocus::from_entity(cell));
        (world, cell)
    }

    /// The title the tooltip shows and the top-left corner it is placed at, once it shows.
    fn shown_tooltip(world: &mut World) -> (String, Vec2) {
        // The content is written on the first frame, the panel placed on the second
        for _ in 0..2 {
            world.run_system_once(show_tooltip).unwrap();
        }
        let mut root = world.query_filtered::<(&Node, &Visibility), With<TooltipRoot>>();
        let (node, visibility) = root.single(world).unwrap();
        assert_eq!(*visibility, Visibility::Inherited, "the tooltip is hidden");
        let (Val::Px(left), Val::Px(top)) = (node.left, node.top) else {
            panic!("the tooltip is not placed");
        };
        let mut lines = world.query::<(&TooltipLine, &Text)>();
        let title = lines
            .iter(world)
            .find(|(TooltipLine(index), _)| *index == 0)
            .unwrap()
            .1
             .0
            .clone();
        (title, Vec2::new(left, top))
    }

    #[test]
    fn the_focused_element_shows_its_tooltip_next_to_it_while_its_ring_shows() {
        let (mut world, _) = world_with_two_tooltips();

        // Without the ring, the focus is not one the player sees: the cursor wins
        assert_eq!(
            shown_tooltip(&mut world),
            ("Hovered".to_string(), Vec2::new(412.0, 312.0))
        );

        // With it, the focused cell wins, its tooltip right of and below it
        world.resource_mut::<InputFocusVisible>().0 = true;
        assert_eq!(
            shown_tooltip(&mut world),
            ("Focused".to_string(), Vec2::new(152.0, 152.0))
        );
    }

    #[test]
    fn the_tooltip_of_a_cell_stays_inside_the_window_and_off_the_cell() {
        let (window, size) = (Vec2::new(800.0, 600.0), Vec2::new(100.0, 40.0));
        // A cell in the bottom-right corner: the tooltip goes left of and above it
        let cell = Rect::new(740.0, 560.0, 780.0, 600.0);
        let position = tooltip_position(cell, size, window);
        assert_eq!(position, Vec2::new(628.0, 508.0));
        assert!(Rect::from_corners(position, position + size)
            .intersect(cell)
            .is_empty());
    }

    /// The anchor of a tooltip shown at the cursor.
    fn at(x: f32, y: f32) -> Rect {
        Rect::from_center_size(Vec2::new(x, y), Vec2::ZERO)
    }

    #[test]
    fn a_tooltip_bigger_than_the_window_sticks_to_its_corner() {
        let window = Vec2::new(800.0, 600.0);
        let huge = Vec2::new(900.0, 700.0);
        assert_eq!(tooltip_position(at(400.0, 300.0), huge, window), Vec2::ZERO);
        assert_eq!(
            tooltip_position(at(400.0, 300.0), Vec2::ZERO, window),
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
                tooltip_position(at(cursor.0, cursor.1), size, window),
                Vec2::from(expected),
                "cursor {cursor:?}"
            );
        }
    }
}
