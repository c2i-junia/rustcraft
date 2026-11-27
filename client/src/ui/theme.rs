//! The design tokens of the UI: colours, sizes, font sizes and spacings. Their values come from
//! `ui/theme.ron` of the assets folder, read at start-up (`load`), or from the copy of that file
//! built into the game when it is missing. A dev build reads it again when it is saved
//! (`watch.rs`). Each token is read through its function, which returns the value in use.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

use bevy::prelude::*;
use serde::{de::Error as _, Deserialize, Deserializer};

#[cfg(feature = "dev")]
pub mod watch;

/// The values of the tokens, as `ui/theme.ron` holds them.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Theme {
    #[serde(deserialize_with = "hex")]
    panel: Color,
    #[serde(deserialize_with = "hex")]
    raise: Color,
    #[serde(deserialize_with = "hex")]
    raise_hover: Color,
    #[serde(deserialize_with = "hex")]
    bevel_hi: Color,
    #[serde(deserialize_with = "hex")]
    bevel_lo: Color,
    bevel_width: f32,
    #[serde(deserialize_with = "hex")]
    slot: Color,
    #[serde(deserialize_with = "hex")]
    accent: Color,
    #[serde(deserialize_with = "hex")]
    accent_hi: Color,
    #[serde(deserialize_with = "hex")]
    accent_lo: Color,
    #[serde(deserialize_with = "hex")]
    experience: Color,
    #[serde(deserialize_with = "hex")]
    experience_hi: Color,
    #[serde(deserialize_with = "hex")]
    experience_lo: Color,
    #[serde(deserialize_with = "hex")]
    ok_hover: Color,
    #[serde(deserialize_with = "hex")]
    ok: Color,
    #[serde(deserialize_with = "hex")]
    danger_hover: Color,
    #[serde(deserialize_with = "hex")]
    danger: Color,
    #[serde(deserialize_with = "hex")]
    text: Color,
    #[serde(deserialize_with = "hex")]
    muted: Color,
    #[serde(deserialize_with = "hex")]
    reticle: Color,
    #[serde(deserialize_with = "hex")]
    wireframe: Color,
    #[serde(deserialize_with = "hex")]
    dirt_tint: Color,
    #[serde(deserialize_with = "hexes")]
    thumbnail_colors: [Color; 6],
    focus_ring_width: f32,
    focus_ring_offset: f32,
    disabled_alpha: f32,
    font_s: f32,
    font_m: f32,
    font_l: f32,
    font_xl: f32,
    space_xs: f32,
    space_s: f32,
    space_m: f32,
    space_l: f32,
    space_xl: f32,
    space_xxl: f32,
}

/// Reads a colour written `"#RRGGBB"`.
fn hex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
    let text = String::deserialize(deserializer)?;
    Srgba::hex(&text)
        .map(Color::from)
        .map_err(|e| D::Error::custom(format!("{text:?} is not a colour: {e}")))
}

/// Reads a list of colours written `"#RRGGBB"`.
fn hexes<'de, D: Deserializer<'de>, const N: usize>(
    deserializer: D,
) -> Result<[Color; N], D::Error> {
    let texts = Vec::<String>::deserialize(deserializer)?;
    let count = texts.len();
    let colors = texts
        .iter()
        .map(|text| {
            Srgba::hex(text)
                .map(Color::from)
                .map_err(|e| D::Error::custom(format!("{text:?} is not a colour: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    colors
        .try_into()
        .map_err(|_| D::Error::custom(format!("{N} colours are expected, not {count}")))
}

/// The file of the theme in the assets folder.
pub fn theme_file(assets_folder: &Path) -> PathBuf {
    assets_folder.join("ui").join("theme.ron")
}

/// Reads the theme from `path`, or says why it cannot.
fn read(path: &Path) -> Result<Theme, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("{} cannot be read: {e}", path.display()))?;
    ron::from_str(&source).map_err(|e| format!("{} cannot be read: {e}", path.display()))
}

/// The theme built into the game: `ui/theme.ron` of the source tree.
static BUILT_IN: LazyLock<Theme> = LazyLock::new(|| {
    ron::from_str(include_str!("../../../data/ui/theme.ron"))
        .unwrap_or_else(|e| panic!("the built-in theme cannot be read: {e}"))
});

/// The theme in use, installed by `load` or `install`; the built-in one until then.
static THEME: RwLock<Option<Theme>> = RwLock::new(None);

#[cfg(test)]
thread_local! {
    /// The theme a test installed, seen by its own thread only: the tests beside it, the
    /// goldens among them, keep the built-in one.
    static INSTALLED_IN_TEST: std::cell::Cell<Option<Theme>> = const { std::cell::Cell::new(None) };
}

/// Puts `theme` in use: the UI spawned from now on takes its values.
fn install(theme: Theme) {
    #[cfg(test)]
    INSTALLED_IN_TEST.set(Some(theme));
    #[cfg(not(test))]
    {
        *THEME.write().unwrap_or_else(|poison| poison.into_inner()) = Some(theme);
    }
}

/// Reads the theme of the assets folder and puts it in use. Without the file the built-in theme
/// stays; a wrong file is logged and the built-in theme stays too.
///
/// The tests, whose assets folder is the source tree, leave the built-in theme, which is the same.
pub fn load(assets_folder: &Path) {
    if cfg!(test) {
        return;
    }
    let path = theme_file(assets_folder);
    if !path.exists() {
        info!("No {}: the built-in theme is used", path.display());
        return;
    }
    match read(&path) {
        Ok(theme) => install(theme),
        Err(err) => error!("{err}: the built-in theme is used"),
    }
}

/// The value of a token in the theme in use.
fn token<T>(value: impl FnOnce(&Theme) -> T) -> T {
    #[cfg(test)]
    if let Some(theme) = INSTALLED_IN_TEST.get() {
        return value(&theme);
    }
    match &*THEME.read().unwrap_or_else(|poison| poison.into_inner()) {
        Some(theme) => value(theme),
        None => value(&BUILT_IN),
    }
}

/// Declares the function that returns each token.
macro_rules! tokens {
    ($($(#[doc = $doc:literal])* $name:ident: $ty:ty;)*) => {
        $(
            $(#[doc = $doc])*
            pub fn $name() -> $ty {
                token(|theme| theme.$name)
            }
        )*
    };
}

tokens! {
    /// Background of the panels, opaque: a translucent panel shows the world through, as the
    /// colours are blended in linear space.
    panel: Color;
    /// Surface of the raised controls.
    raise: Color;
    /// Surface of the raised controls under the cursor.
    raise_hover: Color;
    /// Light edge of a bevel.
    bevel_hi: Color;
    /// Dark edge of a bevel.
    bevel_lo: Color;
    bevel_width: f32;
    /// Background of the item slots and the text fields.
    slot: Color;
    accent: Color;
    /// The light edge of the bevel of a control in the `accent` colour.
    accent_hi: Color;
    /// The dark edge of the bevel of a control in the `accent` colour.
    accent_lo: Color;
    /// The experience: its bar and its orbs.
    experience: Color;
    /// The light edge of the fill of the experience bar.
    experience_hi: Color;
    /// The dark edge of the fill of the experience bar.
    experience_lo: Color;
    /// `ok` under the cursor.
    ok_hover: Color;
    ok: Color;
    /// `danger` under the cursor.
    danger_hover: Color;
    danger: Color;
    text: Color;
    muted: Color;
    /// The bars of the reticle, over their `bevel_lo` outline.
    reticle: Color;
    /// The wireframes of the debug overlay, drawn over the world.
    wireframe: Color;
    /// The tint of the dirt texture behind the menus: about 45 % of its brightness on screen, so
    /// that the text over it reads. The tint multiplies in linear space, hence 0.6 in sRGB.
    dirt_tint: Color;
    /// The earthy colours a world's thumbnail takes: grass, dirt, sand, stone, clay and pine.
    thumbnail_colors: [Color; 6];
    focus_ring_width: f32;
    focus_ring_offset: f32;
    /// Opacity of a disabled control.
    disabled_alpha: f32;
    font_s: f32;
    font_m: f32;
    font_l: f32;
    font_xl: f32;
    space_xs: f32;
    space_s: f32;
    space_m: f32;
    space_l: f32;
    space_xl: f32;
    space_xxl: f32;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The theme as the constants of the code held it before it moved to `ui/theme.ron`.
    fn former_constants() -> Theme {
        Theme {
            panel: Color::srgb_u8(0x11, 0x13, 0x18),
            raise: Color::srgb_u8(0x25, 0x2A, 0x33),
            raise_hover: Color::srgb_u8(0x2E, 0x34, 0x40),
            bevel_hi: Color::srgb_u8(0x48, 0x50, 0x5F),
            bevel_lo: Color::srgb_u8(0x07, 0x08, 0x0A),
            bevel_width: 2.0,
            slot: Color::srgb_u8(0x0C, 0x0E, 0x12),
            accent: Color::srgb_u8(0xF0, 0xB5, 0x4A),
            accent_hi: Color::srgb_u8(0xF6, 0xCF, 0x86),
            accent_lo: Color::srgb_u8(0x8A, 0x65, 0x24),
            experience: Color::srgb_u8(0x3F, 0xC4, 0xB4),
            experience_hi: Color::srgb_u8(0x9C, 0xF2, 0xE4),
            experience_lo: Color::srgb_u8(0x1D, 0x7A, 0x78),
            ok_hover: Color::srgb_u8(0x8F, 0xD8, 0x77),
            ok: Color::srgb_u8(0x78, 0xC7, 0x5F),
            danger_hover: Color::srgb_u8(0xEA, 0x72, 0x5C),
            danger: Color::srgb_u8(0xE3, 0x59, 0x3F),
            text: Color::srgb_u8(0xEF, 0xE9, 0xDA),
            muted: Color::srgb_u8(0x9A, 0xA2, 0xAE),
            reticle: Color::WHITE,
            wireframe: Color::WHITE,
            dirt_tint: Color::srgb(0.6, 0.6, 0.6),
            thumbnail_colors: [
                Color::srgb_u8(0x5D, 0x8A, 0x3A),
                Color::srgb_u8(0x86, 0x60, 0x3A),
                Color::srgb_u8(0xC9, 0xB2, 0x7A),
                Color::srgb_u8(0x7D, 0x7F, 0x82),
                Color::srgb_u8(0xA0, 0x58, 0x3A),
                Color::srgb_u8(0x2F, 0x5D, 0x50),
            ],
            focus_ring_width: 3.0,
            focus_ring_offset: 2.0,
            disabled_alpha: 0.4,
            font_s: 16.0,
            font_m: 20.0,
            font_l: 24.0,
            font_xl: 32.0,
            space_xs: 4.0,
            space_s: 8.0,
            space_m: 12.0,
            space_l: 16.0,
            space_xl: 24.0,
            space_xxl: 32.0,
        }
    }

    #[test]
    fn the_theme_file_holds_the_values_of_the_former_constants_to_the_bit() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        let file = read(&theme_file(&data)).unwrap();
        let mut former = former_constants();
        // `Color::WHITE` is linear, and "#FFFFFF" sRGB: the renderer takes both as the same
        // linear white, to the bit
        for (white, read) in [
            (former.reticle, file.reticle),
            (former.wireframe, file.wireframe),
        ] {
            assert_eq!(
                format!("{:?}", white.to_linear()),
                format!("{:?}", read.to_linear())
            );
        }
        (former.reticle, former.wireframe) = (file.reticle, file.wireframe);
        // the shortest decimal `Debug` writes tells every two `f32`s apart, -0.0 from 0.0 too
        assert_eq!(format!("{file:?}"), format!("{former:?}"));
        assert_eq!(*BUILT_IN, file, "the built-in theme is the file");
    }

    #[test]
    fn a_wrong_colour_is_named() {
        let err = ron::from_str::<Theme>(
            &include_str!("../../../data/ui/theme.ron").replace("\"#111318\"", "\"#11131\""),
        )
        .unwrap_err();
        assert!(err.to_string().contains("#11131"), "{err}");
    }
}
