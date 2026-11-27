//! The key hints: a bar at the bottom of a screen listing what each key does, drawn as key caps.

use bevy::prelude::*;

use crate::i18n::tr_as;

use crate::ui::theme;

/// A key (or keys) and what it does.
pub struct Hint {
    pub keys: &'static str,
    pub action: &'static str,
}

/// The bar of key hints, at the bottom of the screen.
#[derive(Component)]
pub struct HintBar;

/// The height of the bar of key hints: screens keep their content clear of it.
pub const HINT_BAR_HEIGHT: f32 = 44.0;
/// The room above and below the label of a key cap, less than `theme::space_xs()` so that the caps
/// stay within the bar.
const KEY_CAP_PADDING_Y: f32 = 2.0;

/// Whether `keys` has an arrow (U+2190 to U+2193), which the sans font lacks: the key cap then
/// uses the glyph font.
fn uses_glyph_font(keys: &str) -> bool {
    keys.chars().any(|c| ('\u{2190}'..='\u{2193}').contains(&c))
}

/// A bar along the bottom of the screen showing each hint as a key cap and its action.
/// `glyph_font` draws the arrows.
pub fn hint_bar(hints: &[Hint], font: Handle<Font>, glyph_font: Handle<Font>) -> impl Bundle {
    let hints: Vec<_> = hints
        .iter()
        .map(|hint| {
            (
                Node {
                    column_gap: px(theme::space_xs()),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
                children![
                    key_cap(hint.keys, font.clone(), glyph_font.clone()),
                    (
                        Text::new(tr_as("hint", hint.action)),
                        TextFont {
                            font: font.clone().into(),
                            font_size: px(theme::font_s()).into(),
                            ..default()
                        },
                        TextColor(theme::muted()),
                        Pickable::IGNORE,
                    ),
                ],
            )
        })
        .collect();
    (
        HintBar,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(0),
            left: px(0),
            right: px(0),
            height: px(HINT_BAR_HEIGHT),
            column_gap: px(theme::space_l()),
            padding: UiRect::axes(px(theme::space_l()), px(theme::space_s())),
            ..default()
        },
        BackgroundColor(theme::panel()),
        Pickable::IGNORE,
        Children::spawn(SpawnIter(hints.into_iter())),
    )
}

/// A key cap: `keys` on a sunken square. `glyph_font` replaces `font` when `keys` has an arrow.
pub fn key_cap(keys: &str, font: Handle<Font>, glyph_font: Handle<Font>) -> impl Bundle {
    let keys = tr_as("keycap", keys);
    let font = if uses_glyph_font(keys) {
        glyph_font
    } else {
        font
    };
    (
        Node {
            border: UiRect::all(px(theme::bevel_width())),
            padding: UiRect::axes(px(theme::space_xs()), px(KEY_CAP_PADDING_Y)),
            ..default()
        },
        BackgroundColor(theme::slot()),
        BorderColor {
            top: theme::bevel_lo(),
            left: theme::bevel_lo(),
            bottom: theme::bevel_hi(),
            right: theme::bevel_hi(),
        },
        Pickable::IGNORE,
        children![(
            Text::new(keys),
            TextFont {
                font: font.into(),
                font_size: px(theme::font_s()).into(),
                ..default()
            },
            TextColor(theme::text()),
            Pickable::IGNORE,
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_keys_use_the_glyph_font() {
        assert!(uses_glyph_font("←→"));
        assert!(uses_glyph_font("↑"));
        assert!(!uses_glyph_font("Enter"));
        assert!(!uses_glyph_font("Ctrl+Enter"));
    }

    #[test]
    fn a_hint_bar_lists_its_hints() {
        let mut world = World::new();
        let hints = [
            Hint {
                keys: "Enter",
                action: "Select",
            },
            Hint {
                keys: "Esc",
                action: "Quit",
            },
        ];
        let bar = world
            .spawn(hint_bar(&hints, Handle::default(), Handle::default()))
            .id();
        let mut texts = Vec::new();
        let mut stack = vec![bar];
        while let Some(entity) = stack.pop() {
            if let Some(text) = world.get::<Text>(entity) {
                texts.push(text.0.clone());
            }
            if let Some(children) = world.get::<Children>(entity) {
                stack.extend(children.iter().rev());
            }
        }
        assert_eq!(texts, ["Enter", "Select", "Esc", "Quit"]);
        assert!(world.get::<HintBar>(bar).is_some());
    }
}
