//! The status effects of the local player (`shared::players::effects`), in a column at the top
//! left of the screen: each one its icon, its level when above the first, and the time it has
//! left. The icons are Pixel Perfection's `mob_effect/`.

use bevy::prelude::*;
use shared::players::effects::{Effect, EffectKind};
use shared::players::Player;

use crate::i18n::{effect_kind_name, tr, trf};
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::{ACCENT, BEVEL_LO, FONT_S, OK, PANEL, SPACE_S, SPACE_XS, TEXT};

/// The side of an icon on screen, in logical px: twice its texture.
const ICON_SIZE: f32 = 36.0;

/// The column of the effects.
#[derive(Component)]
pub struct EffectsColumn;

/// What the column shows, to rebuild it only when it changes: each effect, and its seconds left.
#[derive(Default)]
pub struct Shown(Vec<(EffectKind, u8, u32)>);

/// The texture of the icon of `kind`.
pub fn icon_path(kind: EffectKind) -> String {
    format!("./graphics/hud/effect_{kind:?}.png").to_lowercase()
}

/// The time an effect has left, as `m:ss`.
pub fn time_left(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The name of an effect at its level: `Poison II`.
pub fn effect_name(kind: EffectKind, level: u8) -> String {
    let name = effect_kind_name(kind).to_string();
    match level {
        0 | 1 => name,
        2 => format!("{name} II"),
        3 => format!("{name} III"),
        _ => format!("{name} IV"),
    }
}

/// An effect as the accessibility tree and the tests read it.
pub fn effect_label(effect: &Effect) -> String {
    trf(
        "hud.effect.label",
        &[
            ("name", &effect_name(effect.kind, effect.level)),
            ("time", &time_left(effect.seconds_left())),
        ],
    )
}

pub fn spawn_effects_column(mut commands: Commands) {
    commands.spawn((
        EffectsColumn,
        Name::new(tr("hud.effects")),
        DespawnOnExit(GameState::Game),
        Node {
            position_type: PositionType::Absolute,
            left: px(SPACE_S),
            top: px(SPACE_S),
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_XS),
            ..default()
        },
        Pickable::IGNORE,
    ));
}

/// Shows the effects of the local player, rebuilt when one comes, goes, or counts a second.
pub fn update_effects_column(
    mut commands: Commands,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    column: Query<Entity, With<EffectsColumn>>,
    assets: Res<AssetServer>,
    mut shown: Local<Shown>,
) {
    let (Ok(player), Ok(column)) = (player.single(), column.single()) else {
        return;
    };
    let effects = &player.effects.0;
    let wanted: Vec<_> = effects
        .iter()
        .map(|effect| (effect.kind, effect.level, effect.seconds_left()))
        .collect();
    if shown.0 == wanted {
        return;
    }
    shown.0 = wanted;
    commands.entity(column).despawn_related::<Children>();
    let font = TextFont {
        font: assets.load(FONT_PATH).into(),
        font_size: px(FONT_S).into(),
        ..default()
    };
    for effect in effects {
        let frame = if effect.kind.is_beneficial() { OK } else { ACCENT };
        let row = commands
            .spawn((
                Name::new(effect_name(effect.kind, effect.level)),
                AccessibleLabel::new(effect_label(effect)),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(SPACE_XS),
                    padding: UiRect::all(px(2.0)),
                    border: UiRect::all(px(2.0)),
                    ..default()
                },
                BorderColor::all(frame),
                BackgroundColor(PANEL.with_alpha(0.75)),
                Pickable::IGNORE,
            ))
            .with_children(|row| {
                row.spawn((
                    ImageNode::new(assets.load(icon_path(effect.kind))),
                    Node {
                        width: px(ICON_SIZE),
                        height: px(ICON_SIZE),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                let level = match effect.level {
                    0 | 1 => String::new(),
                    2 => "II ".into(),
                    3 => "III ".into(),
                    _ => "IV ".into(),
                };
                row.spawn((
                    Text::new(format!("{level}{}", time_left(effect.seconds_left()))),
                    font.clone(),
                    TextColor(TEXT),
                    TextShadow {
                        offset: Vec2::splat(2.0),
                        color: BEVEL_LO,
                    },
                    Pickable::IGNORE,
                ));
            })
            .id();
        commands.entity(column).add_child(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_left_reads_in_minutes_and_seconds() {
        assert_eq!(time_left(5), "0:05");
        assert_eq!(time_left(75), "1:15");
    }

    #[test]
    fn every_effect_has_its_icon() {
        for kind in EffectKind::ALL {
            let path = icon_path(kind);
            let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../data")
                .join(path.trim_start_matches("./"));
            assert!(file.exists(), "{}", file.display());
        }
    }
}
