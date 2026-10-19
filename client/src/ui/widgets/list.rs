//! The scroll list: a recessed well whose items scroll under the wheel, with a scrollbar beside
//! it. Its items can be cards, which the player selects by clicking or focusing them.
//!
//! A list of cards is a single tab stop: only its selected card takes Tab, and the arrow keys,
//! Home and End move the focus, and with it the selection, between the cards. The caller gives
//! the list its role and label, and observes `On<CardSelected>` on it.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        tab_navigation::TabIndex, FocusCause, FocusGained, FocusedInput, InputFocus,
        InputFocusVisible,
    },
    picking::hover::Hovered,
    prelude::*,
    ui::{Selectable, Selected},
    ui_widgets::{
        ControlOrientation, ScrollArea, ScrollIntoView, Scrollbar, ScrollbarDragState,
        ScrollbarThumb,
    },
};

use super::{focus::FocusRing, text_field::sunken_bevel};
use crate::ui::theme::{
    ACCENT, BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH, MUTED, RAISE,
    RAISE_HOVER, SLOT, SPACE_M, SPACE_XS,
};

/// The width of the scrollbar of a list.
pub const SCROLLBAR_WIDTH: f32 = 8.0;
/// The shortest the thumb of the scrollbar gets.
pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;
/// The room around the items for their focus ring, which the list would clip.
pub const RING_ROOM: f32 = FOCUS_RING_WIDTH + FOCUS_RING_OFFSET;

/// What `spawn_scroll_list` builds.
pub struct ScrollListParts {
    /// The row to place, which grows to fill its parent: the well, then the scrollbar.
    pub frame: Entity,
    /// The vertical `ScrollArea` in a sunken well, padded by `RING_ROOM`, where the caller spawns
    /// the items.
    pub list: Entity,
}

/// An item of a list of cards, a direct child of `ScrollListParts::list`.
#[allow(dead_code)] // the world cards (Lot 2, Task 5)
#[derive(Component)]
pub struct ListCard;

/// Triggered on the list when another card is selected.
#[allow(dead_code)] // the world cards (Lot 2, Task 5)
#[derive(EntityEvent)]
pub struct CardSelected {
    /// The list.
    pub entity: Entity,
    /// The card now selected.
    pub card: Entity,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            style_thumb.run_if(any_with_component::<ScrollbarThumb>),
            style_cards.run_if(any_with_component::<ListCard>),
        ),
    )
    .add_observer(select_focused_card)
    .add_observer(card_keys);
}

/// The bevel of a raised surface: light at the top and left.
fn raised_bevel() -> BorderColor {
    BorderColor {
        top: BEVEL_HI,
        left: BEVEL_HI,
        bottom: BEVEL_LO,
        right: BEVEL_LO,
    }
}

/// Spawns a scroll list, without a parent: a recessed well holding the list, and its scrollbar.
/// The wheel scrolls the list under the cursor only.
pub fn spawn_scroll_list(commands: &mut Commands) -> ScrollListParts {
    let frame = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_height: px(0),
            column_gap: px(SPACE_XS),
            ..default()
        })
        .id();
    // a recessed well, whose edge clips the items going out of view. The bevel is around the
    // list, not on it: `ScrollIntoView` does not count the border of the list
    let well = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: px(0),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(SLOT),
            sunken_bevel(),
            ChildOf(frame),
        ))
        .id();
    let list = commands
        .spawn((
            ScrollArea,
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: px(0),
                overflow: Overflow::scroll_y(),
                padding: UiRect::horizontal(px(RING_ROOM)),
                ..default()
            },
            ChildOf(well),
        ))
        .id();
    commands.spawn((
        Scrollbar::new(list, ControlOrientation::Vertical, SCROLLBAR_MIN_THUMB),
        Node {
            width: px(SCROLLBAR_WIDTH),
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(SLOT),
        sunken_bevel(),
        ChildOf(frame),
        children![(
            ScrollbarThumb {
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            Hovered::default(),
            BackgroundColor(BEVEL_HI),
            raised_bevel(),
        )],
    ));
    ScrollListParts { frame, list }
}

/// A card of a list, labelled `label` for screen readers. The caller spawns it as a child of
/// `ScrollListParts::list` and spawns what it shows under it.
#[allow(dead_code)] // the world cards (Lot 2, Task 5)
pub fn list_card(label: &str) -> impl Bundle {
    (
        ListCard,
        // the role before the label: the label's default node must not win
        AccessibilityNode(accesskit::Node::new(Role::ListItem)),
        AccessibleLabel::new(label),
        Selectable,
        // only the selected card takes Tab: the list is one tab stop
        TabIndex(-1),
        FocusRing,
        Hovered::default(),
        Node {
            border: UiRect::all(px(BEVEL_WIDTH)),
            padding: UiRect::all(px(SPACE_M)),
            column_gap: px(SPACE_M),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(RAISE),
        raised_bevel(),
    )
}

/// Selects the card gaining the focus, which becomes the tab stop of its list in place of the
/// card selected before, and brings it into view. The list is told when the selection changes.
fn select_focused_card(
    gained: On<FocusGained>,
    cards: Query<(&ChildOf, Has<Selected>), With<ListCard>>,
    children: Query<&Children>,
    mut commands: Commands,
) {
    // the event bubbles: only the card itself is handled
    let card = gained.entity;
    let Ok((child_of, selected)) = cards.get(card) else {
        return;
    };
    let list = child_of.parent();
    for &sibling in children.get(list).into_iter().flatten() {
        if sibling != card && cards.contains(sibling) {
            commands
                .entity(sibling)
                .remove::<Selected>()
                .insert(TabIndex(-1));
        }
    }
    commands.entity(card).insert((Selected, TabIndex(0)));
    commands.trigger(ScrollIntoView { entity: card });
    if !selected {
        commands.trigger(CardSelected { entity: list, card });
    }
}

/// Moves the focus from a focused card to the previous or next card of its list with Up or Down,
/// and to the first or last with Home or End, without wrapping. A key that moves nothing goes on
/// to the ancestors.
fn card_keys(
    mut event: On<FocusedInput<KeyboardInput>>,
    cards: Query<&ChildOf, With<ListCard>>,
    children: Query<&Children>,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
) {
    // the event bubbles: only the card itself is handled
    let card = event.focused_entity;
    let Ok(child_of) = cards.get(card) else {
        return;
    };
    if event.input.state != ButtonState::Pressed {
        return;
    }
    let siblings: Vec<Entity> = children
        .get(child_of.parent())
        .into_iter()
        .flatten()
        .copied()
        .filter(|&sibling| cards.contains(sibling))
        .collect();
    let Some(index) = siblings.iter().position(|&sibling| sibling == card) else {
        return;
    };
    let target = match event.input.key_code {
        KeyCode::ArrowUp => index.saturating_sub(1),
        KeyCode::ArrowDown => (index + 1).min(siblings.len() - 1),
        KeyCode::Home => 0,
        KeyCode::End => siblings.len() - 1,
        _ => return,
    };
    if target != index {
        event.propagate(false);
        focus.set(siblings[target], FocusCause::Navigated);
        visible.0 = true;
    }
}

/// Lightens the card under the cursor, and gives the selected card an accent border in place of
/// its bevel.
fn style_cards(
    mut cards: Query<
        (
            &Hovered,
            Has<Selected>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<ListCard>,
    >,
) {
    for (hovered, selected, mut background, mut border) in &mut cards {
        let surface = if hovered.get() { RAISE_HOVER } else { RAISE };
        background.set_if_neq(BackgroundColor(surface));
        let edge = if selected {
            BorderColor::all(ACCENT)
        } else {
            raised_bevel()
        };
        border.set_if_neq(edge);
    }
}

/// Lightens the thumb of a scrollbar while it is under the cursor or dragged.
fn style_thumb(
    mut thumbs: Query<(&Hovered, &ScrollbarDragState, &mut BackgroundColor), With<ScrollbarThumb>>,
) {
    for (hovered, drag, mut background) in &mut thumbs {
        let fill = if hovered.get() || drag.dragging {
            MUTED
        } else {
            BEVEL_HI
        };
        background.set_if_neq(BackgroundColor(fill));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_is_a_list_item_with_its_label() {
        let mut world = World::new();
        let card = world.spawn(list_card("Probe")).id();
        let node = world.get::<AccessibilityNode>(card).unwrap();
        assert_eq!(node.role(), Role::ListItem);
        assert_eq!(node.label(), Some("Probe"));
        assert_eq!(world.get::<TabIndex>(card), Some(&TabIndex(-1)));
    }
}
