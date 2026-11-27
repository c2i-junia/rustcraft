//! The announcement of a depth tier the local player reaches for the first time: its name in the
//! upper part of the screen, which fades in, stays a while and fades out.

use crate::i18n::tr;
use bevy::prelude::*;
use shared::world::depths::DepthTier;

use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::theme;

const FADE_SECONDS: f32 = 0.6;
const SHOWN_SECONDS: f32 = 4.0;

/// The tier the server says the local player reached for the first time, not announced yet: it
/// may come while the world loads.
#[derive(Resource, Debug, Default)]
pub struct DepthReached(pub Option<DepthTier>);

/// The name of a tier, shown for `age` seconds so far.
#[derive(Component)]
pub struct DepthBanner {
    age: f32,
}

/// How opaque the banner is `age` seconds after it appeared.
fn opacity(age: f32) -> f32 {
    (age / FADE_SECONDS)
        .min((SHOWN_SECONDS - age) / FADE_SECONDS)
        .clamp(0.0, 1.0)
}

/// Forgets the tier of an earlier session.
pub fn forget_depth(mut reached: ResMut<DepthReached>) {
    reached.0 = None;
}

/// Shows the tier just reached, in the place of the one shown before.
pub fn announce_depth(
    mut commands: Commands,
    mut reached: ResMut<DepthReached>,
    banners: Query<Entity, With<DepthBanner>>,
    assets: Res<AssetServer>,
) {
    let Some(tier) = reached.0.take() else {
        return;
    };
    for banner in &banners {
        commands.entity(banner).despawn();
    }
    commands.spawn((
        DepthBanner { age: 0.0 },
        Name::new(tr("hud.depth_reached")),
        Text::new(crate::i18n::depth_title(tier)),
        TextFont {
            font: assets.load(FONT_PATH).into(),
            font_size: px(theme::font_xl()).into(),
            ..default()
        },
        TextColor(theme::text().with_alpha(0.0)),
        TextLayout::justify(Justify::Center),
        TextShadow {
            offset: Vec2::splat(2.0),
            color: theme::bevel_lo().with_alpha(0.0),
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(0.),
            right: px(0.),
            top: percent(22.),
            ..default()
        },
        Pickable::IGNORE,
        DespawnOnExit(GameState::Game),
    ));
}

pub fn fade_depth_banner(
    mut commands: Commands,
    time: Res<Time>,
    mut banners: Query<(Entity, &mut DepthBanner, &mut TextColor, &mut TextShadow)>,
) {
    for (entity, mut banner, mut color, mut shadow) in &mut banners {
        banner.age += time.delta_secs();
        if banner.age >= SHOWN_SECONDS {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = opacity(banner.age);
        color.0.set_alpha(alpha);
        shadow.color.set_alpha(alpha);
    }
}
