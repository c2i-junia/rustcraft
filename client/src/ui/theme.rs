//! The design tokens of the UI: colours, sizes, font sizes and spacings.

use bevy::prelude::*;

/// Background of the panels, opaque: a translucent panel shows the world through, as the
/// colours are blended in linear space.
pub const PANEL: Color = Color::srgb_u8(0x11, 0x13, 0x18);
/// Surface of the raised controls.
pub const RAISE: Color = Color::srgb_u8(0x25, 0x2A, 0x33);
/// Surface of the raised controls under the cursor.
pub const RAISE_HOVER: Color = Color::srgb_u8(0x2E, 0x34, 0x40);
/// Light edge of a bevel.
pub const BEVEL_HI: Color = Color::srgb_u8(0x48, 0x50, 0x5F);
/// Dark edge of a bevel.
pub const BEVEL_LO: Color = Color::srgb_u8(0x07, 0x08, 0x0A);
pub const BEVEL_WIDTH: f32 = 2.0;
/// Background of the item slots and the text fields.
pub const SLOT: Color = Color::srgb_u8(0x0C, 0x0E, 0x12);
pub const ACCENT: Color = Color::srgb_u8(0xF0, 0xB5, 0x4A);
/// `OK` under the cursor.
pub const OK_HOVER: Color = Color::srgb_u8(0x8F, 0xD8, 0x77);
pub const OK: Color = Color::srgb_u8(0x78, 0xC7, 0x5F);
/// `DANGER` under the cursor.
pub const DANGER_HOVER: Color = Color::srgb_u8(0xEA, 0x72, 0x5C);
pub const DANGER: Color = Color::srgb_u8(0xE3, 0x59, 0x3F);
pub const TEXT: Color = Color::srgb_u8(0xEF, 0xE9, 0xDA);
pub const MUTED: Color = Color::srgb_u8(0x9A, 0xA2, 0xAE);
/// The bars of the reticle, over their `BEVEL_LO` outline.
pub const RETICLE: Color = Color::WHITE;
/// The wireframes of the debug overlay, drawn over the world.
pub const WIREFRAME: Color = Color::WHITE;
/// The tint of the dirt texture behind the menus: about 45 % of its brightness on screen, so
/// that the text over it reads. The tint multiplies in linear space, hence more than 0.45 in sRGB.
pub const DIRT_TINT: Color = Color::srgb(0.6, 0.6, 0.6);
/// The earthy colours a world's thumbnail takes: grass, dirt, sand, stone, clay and pine.
pub const THUMBNAIL_COLORS: [Color; 6] = [
    Color::srgb_u8(0x5D, 0x8A, 0x3A),
    Color::srgb_u8(0x86, 0x60, 0x3A),
    Color::srgb_u8(0xC9, 0xB2, 0x7A),
    Color::srgb_u8(0x7D, 0x7F, 0x82),
    Color::srgb_u8(0xA0, 0x58, 0x3A),
    Color::srgb_u8(0x2F, 0x5D, 0x50),
];

pub const FOCUS_RING_WIDTH: f32 = 3.0;
pub const FOCUS_RING_OFFSET: f32 = 2.0;
/// Opacity of a disabled control.
pub const DISABLED_ALPHA: f32 = 0.4;

pub const FONT_S: f32 = 16.0;
pub const FONT_M: f32 = 20.0;
pub const FONT_L: f32 = 24.0;
pub const FONT_XL: f32 = 32.0;

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_S: f32 = 8.0;
pub const SPACE_M: f32 = 12.0;
pub const SPACE_L: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;
pub const SPACE_XXL: f32 = 32.0;
