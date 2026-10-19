use bevy::prelude::*;

// Text styles
pub fn text_font(font: Handle<Font>, font_size: f32) -> TextFont {
    TextFont {
        font: font.into(),
        font_size: font_size.into(),
        ..default()
    }
}
