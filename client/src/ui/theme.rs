//! The design tokens of the UI: colours, sizes, font sizes and spacings.

// The tokens are used as the widgets and screens are rebuilt on them
#![allow(dead_code)]

use bevy::prelude::*;

/// Background of the panels, 93 % opaque.
pub const PANEL: Color = Color::srgba_u8(0x11, 0x13, 0x18, 237);
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
pub const OK: Color = Color::srgb_u8(0x78, 0xC7, 0x5F);
pub const DANGER: Color = Color::srgb_u8(0xE3, 0x59, 0x3F);
pub const TEXT: Color = Color::srgb_u8(0xEF, 0xE9, 0xDA);
pub const MUTED: Color = Color::srgb_u8(0x9A, 0xA2, 0xAE);

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
