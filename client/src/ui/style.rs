use bevy::prelude::*;
use bevy::ui::{AlignItems, FlexDirection, JustifyContent, Node, UiRect, Val};

// Common styles for buttons
pub const NORMAL_BUTTON: Color = Color::srgb(0.3, 0.3, 0.3);
pub const HOVERED_BUTTON: Color = Color::srgb(0.4, 0.4, 0.4);
pub const PRESSED_BUTTON: Color = Color::srgb(0.2, 0.2, 0.2);

// Common background colors
pub const BACKGROUND_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);

// Common text colors
pub const TEXT_COLOR: Color = Color::WHITE;
/// Text of the large buttons of the main and settings menus, slightly dimmed.
pub const BUTTON_TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

// Button styles
pub fn big_button_style() -> Node {
    Node {
        width: Val::Px(400.0),
        height: Val::Px(60.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

// Text styles
pub fn text_font(font: Handle<Font>, font_size: f32) -> TextFont {
    TextFont {
        font: font.into(),
        font_size: font_size.into(),
        ..default()
    }
}

/// An image that fills its whole node, border included, as `ImageNode` did before Bevy 0.19
/// (which now fits the image inside the content box, keeping its aspect ratio).
pub fn fill_image_node(image: Handle<Image>) -> ImageNode {
    ImageNode {
        image_mode: NodeImageMode::Stretch,
        visual_box: VisualBox::BorderBox,
        ..ImageNode::new(image)
    }
}

pub fn background_image_style() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        ..default()
    }
}
