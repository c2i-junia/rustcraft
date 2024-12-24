use bevy::prelude::*;
use bevy::ui::{AlignItems, FlexDirection, JustifyContent, Node, UiRect, Val};

// Common styles for buttons
pub const NORMAL_BUTTON: Color = Color::srgb(0.3, 0.3, 0.3);
pub const HOVERED_BUTTON: Color = Color::srgb(0.4, 0.4, 0.4);
// pub const HOVERED_PRESSED_BUTTON: Color = Color::srgb(0.25, 0.65, 0.25);
pub const PRESSED_BUTTON: Color = Color::srgb(0.2, 0.2, 0.2);

// Common background colors
pub const BACKGROUND_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);
// pub const BUTTON_BORDER_COLOR: Color = Color::BLACK;

// Common text color
pub const TEXT_COLOR: Color = Color::WHITE;

// Button styles
pub fn big_button_style() -> Node {
    Node {
        width: Val::Px(400.0),
        height: Val::Px(60.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..Default::default()
    }
}

// pub fn long_button_style() -> Node {
//     Node {
//         width: Val::Percent(80.0), // Wider than the standard button
//         height: Val::Px(65.0),
//         margin: UiRect::all(Val::Px(20.0)),
//         justify_content: JustifyContent::Center,
//         align_items: AlignItems::Center,
//         ..Default::default()
//     }
// }

// pub fn grid_button_style(grid_span: usize) -> Node {
//     let mut style = big_button_style();

//     // Convert `usize` to `u16` safely
//     style.grid_column = GridPlacement::span(
//         grid_span.try_into().expect("grid_span must fit into a u16"), // This ensures the value is safe for `u16`
//     );
//     style
// }

// Text styles
pub fn text_font(font: Handle<Font>, font_size: f32) -> TextFont {
    TextFont {
        font,
        font_size,
        ..Default::default()
    }
}

// Styles for Nodes or Containers
// pub fn container_style(flex_direction: FlexDirection) -> Node {
//     Node {
//         flex_direction,
//         align_items: AlignItems::Center,
//         justify_content: JustifyContent::Center,
//         width: Val::Percent(100.0),
//         height: Val::Percent(100.0),
//         ..Default::default()
//     }
// }

pub fn background_image_style() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        ..Default::default()
    }
}
