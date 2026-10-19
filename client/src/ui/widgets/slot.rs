//! The inventory slot: a sunken square holding an icon, a count and, in the hotbar, its number.
//!
//! The hit test of a slot is its `Hovered`; callers observe the pointer events on the entity.
//! A slot takes the focus, and shows the focus ring, like the other widgets.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode, input_focus::tab_navigation::TabIndex, picking::hover::Hovered,
    prelude::*,
};

use super::focus::FocusRing;
use crate::ui::theme::{
    ACCENT, BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, DANGER, FONT_S, MUTED, OK, RAISE_HOVER, SLOT, TEXT,
};
use shared::world::{ItemStack, ItemType};

/// The side of a slot, in logical px.
pub const SLOT_SIZE: f32 = 50.0;

/// Twice the 16 px item textures: a whole multiple of them at every UiScale step.
pub const ICON_SIZE: f32 = 32.0;

/// An inventory slot built by `slot`, which keeps its look in step with the cursor and selection.
#[derive(Component)]
#[require(SlotSelected, Hovered)]
pub struct Slot;

/// Whether the slot is the selected one: it then takes the accent colour.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotSelected(pub bool);

/// The icon of the item in a slot, hidden while the slot is empty.
#[derive(Component)]
pub struct SlotIcon;

/// The count of the items in a slot.
#[derive(Component)]
pub struct SlotCount;

/// The bar under the icon of a worn tool, as long as what is left of the tool.
#[derive(Component)]
pub struct SlotDurability;

/// Height of the durability bar, in logical px.
const DURABILITY_BAR_HEIGHT: f32 = 3.0;

/// The node of a slot. Callers override the grid placement with `..slot_node()`.
pub fn slot_node() -> Node {
    Node {
        width: px(SLOT_SIZE),
        height: px(SLOT_SIZE),
        border: UiRect::all(px(BEVEL_WIDTH)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// The icon of a slot, hidden until an item shows in it.
pub fn slot_icon(icon: ImageNode) -> impl Bundle {
    (
        SlotIcon,
        icon,
        Node {
            width: px(ICON_SIZE),
            height: px(ICON_SIZE),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
    )
}

/// The count of a slot, in its bottom-right corner. It is empty until set.
pub fn slot_count(font: Handle<Font>) -> impl Bundle {
    (
        SlotCount,
        Text::new(""),
        TextFont {
            font: font.into(),
            font_size: px(FONT_S).into(),
            ..default()
        },
        TextColor(TEXT),
        TextShadow {
            offset: Vec2::splat(2.0),
            color: BEVEL_LO,
        },
        Node {
            position_type: PositionType::Absolute,
            right: px(BEVEL_WIDTH),
            bottom: px(BEVEL_WIDTH),
            ..default()
        },
        Pickable::IGNORE,
    )
}

/// The durability bar of a slot, under its icon, hidden until a worn tool shows in it.
pub fn slot_durability() -> impl Bundle {
    (
        SlotDurability,
        Node {
            position_type: PositionType::Absolute,
            left: px((SLOT_SIZE - 2.0 * BEVEL_WIDTH - ICON_SIZE) / 2.0),
            bottom: px(BEVEL_WIDTH),
            width: px(ICON_SIZE),
            height: px(DURABILITY_BAR_HEIGHT),
            ..default()
        },
        BackgroundColor(OK),
        Visibility::Hidden,
        Pickable::IGNORE,
    )
}

/// The wear and the durability of a tool stack, `None` for any other item.
pub fn tool_wear(stack: &ItemStack) -> Option<(u32, u32)> {
    let tool = stack.item_id.tool()?;
    let ItemType::Tool { wear } = stack.item_type else {
        return None;
    };
    Some((wear, tool.durability))
}

/// The share of its durability a worn tool has left, from 0 to 1, and the colour of its bar:
/// `None` for an unworn tool or any other item, which show no bar, like in Minecraft.
pub fn durability_bar(stack: &ItemStack) -> Option<(f32, Color)> {
    let (wear, durability) = tool_wear(stack).filter(|&(wear, _)| wear > 0)?;
    let left = durability.saturating_sub(wear) as f32 / durability as f32;
    let color = if left > 0.5 {
        OK
    } else if left > 0.25 {
        ACCENT
    } else {
        DANGER
    };
    Some((left, color))
}

/// The text of a count: nothing for a single item.
pub fn count_label(nb: u32) -> String {
    if nb < 2 {
        String::new()
    } else {
        nb.to_string()
    }
}

/// A slot named `label`, with its number in the top-left corner when it has one.
pub fn slot(label: &str, number: Option<u32>, icon: ImageNode, font: Handle<Font>) -> impl Bundle {
    let (background, border) = slot_colors(false, false);
    let number = number.map(|number| {
        (
            Text::new(number.to_string()),
            TextFont {
                font: font.clone().into(),
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(MUTED),
            // Read over any icon, as the count is
            TextShadow {
                offset: Vec2::splat(2.0),
                color: BEVEL_LO,
            },
            Node {
                position_type: PositionType::Absolute,
                left: px(BEVEL_WIDTH),
                top: px(BEVEL_WIDTH),
                ..default()
            },
            Pickable::IGNORE,
        )
    });
    (
        Slot,
        // the role before the label: the label's default node must not win
        AccessibilityNode(accesskit::Node::new(Role::GridCell)),
        AccessibleLabel::new(label),
        TabIndex(0),
        FocusRing,
        slot_node(),
        BackgroundColor(background),
        border,
        Children::spawn((
            Spawn(slot_icon(icon)),
            Spawn(slot_durability()),
            Spawn(slot_count(font)),
            SpawnIter(number.into_iter()),
        )),
    )
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, style_slots);
}

/// The background and the border of a slot in a given state.
fn slot_colors(hovered: bool, selected: bool) -> (Color, BorderColor) {
    let background = if hovered { RAISE_HOVER } else { SLOT };
    let border = if selected {
        BorderColor::all(ACCENT)
    } else {
        BorderColor {
            top: BEVEL_LO,
            left: BEVEL_LO,
            bottom: BEVEL_HI,
            right: BEVEL_HI,
        }
    };
    (background, border)
}

/// Applies the look of a slot to the slots whose state changed.
pub(crate) fn style_slots(
    mut slots: Query<
        (
            &Hovered,
            &SlotSelected,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (With<Slot>, Or<(Changed<Hovered>, Changed<SlotSelected>)>),
    >,
) {
    for (hovered, selected, mut background, mut border) in &mut slots {
        let (new_background, new_border) = slot_colors(hovered.0, selected.0);
        background.0 = new_background;
        *border = new_border;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_shows_from_two_items() {
        assert_eq!(count_label(0), "");
        assert_eq!(count_label(1), "");
        assert_eq!(count_label(2), "2");
        assert_eq!(count_label(64), "64");
    }

    fn tool(wear: u32) -> ItemStack {
        ItemStack {
            item_id: shared::world::ItemId::WoodenPickaxe,
            item_type: ItemType::Tool { wear },
            nb: 1,
        }
    }

    #[test]
    fn only_a_worn_tool_shows_a_durability_bar() {
        let dirt = ItemStack {
            item_id: shared::world::ItemId::Dirt,
            item_type: shared::world::ItemId::Dirt.get_default_type(),
            nb: 3,
        };
        assert_eq!(durability_bar(&dirt), None);
        assert_eq!(durability_bar(&tool(0)), None);
        let durability = tool(0).item_id.tool().unwrap().durability;
        let (left, _) = durability_bar(&tool(durability / 2)).unwrap();
        assert!((left - 0.5).abs() < 0.02, "{left}");
    }

    #[test]
    fn the_durability_bar_turns_from_green_to_red() {
        let durability = tool(0).item_id.tool().unwrap().durability;
        let color = |wear| durability_bar(&tool(wear)).unwrap().1;
        assert_eq!(color(1), OK);
        assert_eq!(color(durability * 2 / 3), ACCENT);
        assert_eq!(color(durability - 1), DANGER);
    }

    #[test]
    fn a_slot_is_sunken_lightens_on_hover_and_takes_the_accent_when_selected() {
        let (background, border) = slot_colors(false, false);
        assert_eq!(background, SLOT);
        assert_eq!(
            [border.top, border.left, border.bottom, border.right],
            [BEVEL_LO, BEVEL_LO, BEVEL_HI, BEVEL_HI]
        );
        assert_eq!(slot_colors(true, false).0, RAISE_HOVER);
        assert_eq!(slot_colors(false, true).1, BorderColor::all(ACCENT));
    }

    #[test]
    fn icons_are_whole_multiples_of_their_texels_at_every_scale() {
        for scale in [1.0, 1.5, 2.0, 3.0] {
            assert_eq!((ICON_SIZE * scale) % 16.0, 0.0, "scale {scale}");
        }
        // the icon is centred on whole pixels
        assert_eq!((SLOT_SIZE - ICON_SIZE - 2.0 * BEVEL_WIDTH) % 2.0, 0.0);
    }

    #[test]
    fn a_slot_has_the_grid_cell_role_and_its_label() {
        let mut world = World::new();
        let id = world
            .spawn(slot(
                "Hotbar slot 1",
                Some(1),
                ImageNode::default(),
                Handle::default(),
            ))
            .id();
        let node = world.get::<AccessibilityNode>(id).unwrap();
        assert_eq!(node.role(), Role::GridCell);
        assert_eq!(node.label(), Some("Hotbar slot 1"));
    }
}
