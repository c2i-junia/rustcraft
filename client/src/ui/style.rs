use bevy::prelude::*;

// Common styles for buttons
pub const NORMAL_BUTTON: Color = Color::srgb(0.3, 0.3, 0.3);
pub const HOVERED_BUTTON: Color = Color::srgb(0.4, 0.4, 0.4);
pub const PRESSED_BUTTON: Color = Color::srgb(0.2, 0.2, 0.2);

// Common background colors
pub const BACKGROUND_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);

// Common text colors
pub const TEXT_COLOR: Color = Color::WHITE;

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
