//! RustCraft Accents (`data/fonts/RustCraftAccents.ttf`, drawn by
//! `scripts/generate_accent_font.py`): the letters French needs that the pixel font lacks, given
//! to the text engine as the fallback of every Latin text, glyph by glyph. A text in the pixel
//! font then shows é or Œ in the same pixels instead of a missing glyph.

use bevy::prelude::*;
use bevy::text::FontCx;
use fontique::Script;

pub const ACCENTS_FONT_PATH: &str = "fonts/RustCraftAccents.ttf";
const ACCENTS_FAMILY: &str = "RustCraft Accents";

/// The characters of RustCraft Accents.
pub const ACCENTS_FONT_CHARS: &str = "\u{a0}°ÀÂÈÉÊËÎÏÔÙÛèéïŒœŸ‘’“”…\u{202f}";

/// The accents font, kept loaded, and whether the text engine falls back on it yet.
#[derive(Resource)]
struct AccentsFont {
    _handle: Handle<Font>,
    registered: bool,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load_accents).add_systems(
        Update,
        register_accents
            .run_if(|font: Option<Res<AccentsFont>>| font.is_some_and(|f| !f.registered)),
    );
}

fn load_accents(mut commands: Commands, assets: Option<Res<AssetServer>>) {
    if let Some(assets) = assets {
        commands.insert_resource(AccentsFont {
            _handle: assets.load(ACCENTS_FONT_PATH),
            registered: false,
        });
    }
}

/// Once the font is in the collection of the text engine, makes it the fallback of the Latin
/// texts, and of the punctuation and the spaces, whose script is that of no language.
fn register_accents(mut font: ResMut<AccentsFont>, font_cx: Option<ResMut<FontCx>>) {
    let Some(mut font_cx) = font_cx else {
        return;
    };
    let Some(family) = font_cx.collection.family_id(ACCENTS_FAMILY) else {
        return;
    };
    for script in [*b"Latn", *b"Zyyy", *b"Zinh"] {
        font_cx
            .collection
            .append_fallbacks(Script::from_bytes(script), std::iter::once(family));
    }
    font.registered = true;
}
