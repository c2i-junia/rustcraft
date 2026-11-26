//! What the local player knows of the mount it rides, over the hotbar: a horse's coat, its
//! health, the gallop it has left and the charge of its jump; and how to get off.

use bevy::prelude::*;
use shared::players::{riding::Riding, Player};
use shared::world::MobKind;

use crate::i18n::{mob_name, tr, trf};
use crate::mob::{Mob, MobEntities};
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::{BEVEL_LO, FONT_S, TEXT};

/// How far above the bottom of the screen the line is, over the bars of the player.
const BOTTOM: f32 = 112.0;

#[derive(Component)]
pub struct MountLine;

pub fn spawn_mount_line(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Name::new(tr("hud.mount")),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(BOTTOM),
            left: px(0.),
            right: px(0.),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Pickable::IGNORE,
        DespawnOnExit(GameState::Game),
        children![(
            MountLine,
            Text::new(""),
            TextFont {
                font: assets.load(FONT_PATH).into(),
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(TEXT),
            TextShadow {
                offset: Vec2::splat(2.0),
                color: BEVEL_LO,
            },
            Pickable::IGNORE,
        )],
    ));
}

/// The line of the mount `riding`, as `mob` is drawn, if it is: its name, its health, and a
/// horse's breath and jump.
pub fn mount_text(riding: &Riding, mob: Option<&Mob>) -> String {
    let mut parts = Vec::new();
    match (riding.kind, mob.and_then(|mob| mob.coat)) {
        (MobKind::Horse, Some(coat)) => {
            parts.push(trf("hud.mount.horse", &[("coat", &tr(coat.key()))]));
        }
        (kind, _) => parts.push(mob_name(kind).to_string()),
    }
    if let Some(mob) = mob.filter(|_| riding.kind == MobKind::Horse) {
        let health = mob.max_health.saturating_sub(mob.damage());
        parts.push(trf(
            "hud.mount.health",
            &[("health", &health), ("max", &mob.max_health)],
        ));
    }
    if riding.kind == MobKind::Horse {
        if riding.winded {
            parts.push(tr("hud.mount.winded").to_string());
        } else {
            let percent = (riding.breath() * 100.0).round() as u32;
            parts.push(trf("hud.mount.breath", &[("percent", &percent)]));
        }
        if riding.charge_ms > 0 {
            let percent = (riding.charge() * 100.0).round() as u32;
            parts.push(trf("hud.mount.jump", &[("percent", &percent)]));
        }
    }
    parts.push(tr("hud.mount.dismount").to_string());
    parts.join("  ·  ")
}

/// Writes the line while the local player rides, and empties it once it is off.
pub fn update_mount_line(
    player: Query<&Player, With<CurrentPlayerMarker>>,
    entities: Res<MobEntities>,
    mobs: Query<&Mob>,
    mut lines: Query<&mut Text, With<MountLine>>,
) {
    let Ok(mut line) = lines.single_mut() else {
        return;
    };
    let text = match player.single().ok().and_then(|player| player.riding) {
        Some(riding) => {
            let mob = entities.get(riding.mob).and_then(|e| mobs.get(e).ok());
            mount_text(&riding, mob)
        }
        None => String::new(),
    };
    if line.0 != text {
        line.0 = text;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::riding::Riding;

    #[test]
    fn a_boat_tells_only_how_to_get_off_and_a_horse_its_breath() {
        let boat = mount_text(&Riding::boat(1, 0, 0.0), None);
        assert!(boat.starts_with("Boat"), "{boat}");
        assert!(boat.contains("Sneak"), "{boat}");
        let traits = shared::world::HorseTraits {
            coat: shared::world::Coat::Dun,
            speed: 8.0,
            jump: 2.0,
            endurance: 4.0,
        };
        let mut riding = Riding::horse(1, &traits, 0.0);
        riding.charge_ms = 500;
        let horse = mount_text(&riding, None);
        assert!(horse.contains("Breath 100%"), "{horse}");
        assert!(horse.contains("Jump 50%"), "{horse}");
    }
}
