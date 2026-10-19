//! The scroll list: a recessed well whose items scroll under the wheel, with a scrollbar beside
//! it. Its items can be cards, which the player selects by clicking or focusing them.
//!
//! A list of cards is a single tab stop: only its selected card takes Tab, or its first card while
//! none is selected, and the arrow keys, Home and End move the focus, and with it the selection,
//! between the cards. The caller gives the list its role and label, and observes
//! `On<CardSelected>` on it.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    ecs::entity::EntityHashSet,
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

use super::{focus::FocusRing, raised_bevel, sunken_bevel};
use crate::ui::theme::{
    ACCENT, BEVEL_HI, BEVEL_WIDTH, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH, MUTED, RAISE, RAISE_HOVER,
    SLOT, SPACE_M, SPACE_XS,
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
    /// The vertical `ScrollArea` in a sunken well, padded by `RING_ROOM` on its sides, where the
    /// caller spawns the items. An item carries the room of its ring above and below itself, as
    /// `spawn_list_card` does.
    pub list: Entity,
}

/// A card of a list, in its `CardSlot`.
#[derive(Component)]
pub struct ListCard;

/// The item of a list that holds a card and, above and below it, the room of the card's ring,
/// which the list would clip: the list brings the slot into view, ring included.
#[derive(Component)]
pub struct CardSlot;

/// Triggered on the list when another card is selected.
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
            (first_card_takes_tab, style_cards).run_if(any_with_component::<ListCard>),
        ),
    )
    .add_observer(select_focused_card)
    .add_observer(card_keys);
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

/// Spawns a card labelled `label` for screen readers at the end of `list`, in its slot, and
/// returns the card, under which the caller spawns what it shows.
pub fn spawn_list_card(commands: &mut Commands, list: Entity, label: &str) -> Entity {
    let slot = commands
        .spawn((
            CardSlot,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::vertical(px(RING_ROOM)),
                ..default()
            },
            ChildOf(list),
        ))
        .id();
    commands
        .spawn((
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
            ChildOf(slot),
        ))
        .id()
}

/// The list holding `card`, and the slot of the card in it.
fn list_of(
    card: Entity,
    parents: &Query<&ChildOf>,
    slots: &Query<(), With<CardSlot>>,
) -> Option<(Entity, Entity)> {
    let slot = parents.get(card).ok()?.parent();
    if !slots.contains(slot) {
        return None;
    }
    Some((parents.get(slot).ok()?.parent(), slot))
}

/// The cards of `list`, in the order of their slots.
fn cards_of(
    list: Entity,
    children: &Query<&Children>,
    is_card: impl Fn(Entity) -> bool,
) -> Vec<Entity> {
    children
        .get(list)
        .into_iter()
        .flatten()
        .filter_map(|&slot| {
            children
                .get(slot)
                .ok()?
                .iter()
                .find(|&child| is_card(child))
        })
        .collect()
}

/// Selects the card gaining the focus, which becomes the tab stop of its list in place of the
/// card selected before, and brings its slot into view. The list is told when the selection
/// changes.
fn select_focused_card(
    gained: On<FocusGained>,
    cards: Query<Has<Selected>, With<ListCard>>,
    parents: Query<&ChildOf>,
    slots: Query<(), With<CardSlot>>,
    children: Query<&Children>,
    mut commands: Commands,
) {
    // the event bubbles up from the focused entity: handle it once, on its first step
    let card = gained.original_event_target();
    if gained.entity != card {
        return;
    }
    let Ok(selected) = cards.get(card) else {
        return;
    };
    let Some((list, slot)) = list_of(card, &parents, &slots) else {
        return;
    };
    for other in cards_of(list, &children, |entity| cards.contains(entity)) {
        if other != card {
            commands
                .entity(other)
                .remove::<Selected>()
                .insert(TabIndex(-1));
        }
    }
    commands.entity(card).insert((Selected, TabIndex(0)));
    commands.trigger(ScrollIntoView { entity: slot });
    if !selected {
        commands.trigger(CardSelected { entity: list, card });
    }
}

/// Moves the focus from a focused card to the previous or next card of its list with Up or Down,
/// and to the first or last with Home or End, without wrapping. A key that moves nothing goes on
/// to the ancestors.
fn card_keys(
    mut event: On<FocusedInput<KeyboardInput>>,
    cards: Query<(), With<ListCard>>,
    parents: Query<&ChildOf>,
    slots: Query<(), With<CardSlot>>,
    children: Query<&Children>,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
) {
    // the event bubbles: only the step on the card itself is handled
    let card = event.focused_entity;
    if !cards.contains(card) || event.input.state != ButtonState::Pressed {
        return;
    }
    let Some((list, _)) = list_of(card, &parents, &slots) else {
        return;
    };
    let order = cards_of(list, &children, |entity| cards.contains(entity));
    let Some(index) = order.iter().position(|&other| other == card) else {
        return;
    };
    let target = match event.input.key_code {
        KeyCode::ArrowUp => index.saturating_sub(1),
        KeyCode::ArrowDown => (index + 1).min(order.len() - 1),
        KeyCode::Home => 0,
        KeyCode::End => order.len() - 1,
        _ => return,
    };
    if target != index {
        event.propagate(false);
        focus.set(order[target], FocusCause::Navigated);
        visible.0 = true;
    }
}

/// Makes the first card of a list without a selected card its tab stop, so that Tab enters the
/// list before anything is selected.
fn first_card_takes_tab(
    slots: Query<&ChildOf, With<CardSlot>>,
    children: Query<&Children>,
    mut cards: Query<(&mut TabIndex, Has<Selected>), With<ListCard>>,
) {
    let lists: EntityHashSet = slots.iter().map(ChildOf::parent).collect();
    for list in lists {
        let order = cards_of(list, &children, |entity| cards.contains(entity));
        if order
            .iter()
            .any(|&card| cards.get(card).is_ok_and(|(_, selected)| selected))
        {
            continue;
        }
        if let Some(Ok((mut tab_index, _))) = order.first().map(|&card| cards.get_mut(card)) {
            if tab_index.0 != 0 {
                tab_index.0 = 0;
            }
        }
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
        let list = world.spawn_empty().id();
        let card = spawn_list_card(&mut world.commands(), list, "Probe");
        world.flush();
        // in a slot, the room of its ring, which is the item of the list
        let slot = world.get::<ChildOf>(card).unwrap().parent();
        assert!(world.get::<CardSlot>(slot).is_some());
        assert_eq!(world.get::<ChildOf>(slot).unwrap().parent(), list);
        let node = world.get::<AccessibilityNode>(card).unwrap();
        assert_eq!(node.role(), Role::ListItem);
        assert_eq!(node.label(), Some("Probe"));
        assert_eq!(world.get::<TabIndex>(card), Some(&TabIndex(-1)));
    }
}
