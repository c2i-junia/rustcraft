//! The bar of the boss near the local player: its name and its health, at the top of the screen.
//! Its music plays meanwhile.

use crate::i18n::tr;
use bevy::prelude::*;
use shared::messages::BossBar;

use crate::audio::music::{MusicMood, MusicMoodOverride};
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::{BEVEL_LO, FONT_L, SLOT, SPACE_L, SPACE_XS, TEXT};

const WIDTH: f32 = 360.0;
const HEIGHT: f32 = 14.0;
const EMBER: Color = Color::srgb(0.93, 0.36, 0.08);
const EMBER_HI: Color = Color::srgb(1.0, 0.72, 0.25);
const EMBER_LO: Color = Color::srgb(0.55, 0.14, 0.03);

/// What the server last said of the boss near the local player.
#[derive(Resource, Debug, Default, PartialEq)]
pub struct NearBoss(pub Option<BossBar>);

/// The bar, its name and its health.
#[derive(Component)]
pub struct BossBarRoot;

#[derive(Component)]
pub struct BossBarFill;

/// The music of the boss plays while its bar shows.
pub fn boss_music(near: Res<NearBoss>, mut mood: ResMut<MusicMoodOverride>) {
    if near.is_changed() {
        mood.0 = near.0.map(|_| MusicMood::Boss);
    }
}

/// Forgets the boss of a session that ends.
pub fn forget_boss(mut near: ResMut<NearBoss>, mut mood: ResMut<MusicMoodOverride>) {
    near.0 = None;
    mood.0 = None;
}

/// Shows the bar while a boss is near, filled with its health.
pub fn show_boss_bar(
    near: Res<NearBoss>,
    roots: Query<Entity, With<BossBarRoot>>,
    mut fills: Query<&mut Node, With<BossBarFill>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    if !near.is_changed() {
        return;
    }
    let Some(bar) = near.0 else {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    };
    let width = percent(100.0 * bar.health as f32 / bar.max_health.max(1) as f32);
    if let Ok(mut fill) = fills.single_mut() {
        fill.width = width;
        return;
    }
    commands.spawn((
        BossBarRoot,
        Name::new(tr("hud.boss_bar")),
        Node {
            position_type: PositionType::Absolute,
            top: px(SPACE_L),
            left: px(0.),
            right: px(0.),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(SPACE_XS),
            ..default()
        },
        Pickable::IGNORE,
        DespawnOnExit(GameState::Game),
        children![
            (
                Text::new(crate::i18n::mob_name(bar.kind)),
                TextFont {
                    font: assets.load(FONT_PATH).into(),
                    font_size: px(FONT_L).into(),
                    ..default()
                },
                TextColor(TEXT),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: BEVEL_LO,
                },
                Pickable::IGNORE,
            ),
            (
                Node {
                    width: px(WIDTH),
                    max_width: percent(50),
                    height: px(HEIGHT),
                    border: UiRect::all(px(2.)),
                    ..default()
                },
                BackgroundColor(SLOT),
                BorderColor::all(BEVEL_LO),
                Pickable::IGNORE,
                children![(
                    BossBarFill,
                    Node {
                        width,
                        height: percent(100),
                        border: UiRect::vertical(px(3.)),
                        ..default()
                    },
                    BackgroundColor(EMBER),
                    BorderColor {
                        top: EMBER_HI,
                        bottom: EMBER_LO,
                        ..BorderColor::all(EMBER)
                    },
                    Pickable::IGNORE,
                )],
            )
        ],
    ));
}
