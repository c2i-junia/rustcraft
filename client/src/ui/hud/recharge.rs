//! The gauge of the recharge of the blows, under the reticle: it fills as the weapon held
//! recharges (`melee::Recharge`), counted from the inputs as the server counts them, and is gone
//! once the blow is full.

use bevy::prelude::*;
use shared::players::{blocks::held_item, melee::Recharge, Player};

use super::reticle::Reticle;
use crate::i18n::{tr, trf};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::theme;

/// The size of the gauge, in logical px, and how far under the middle of the screen it is.
const GAUGE_WIDTH: f32 = 24.0;
const GAUGE_HEIGHT: f32 = 3.0;
const GAUGE_BELOW: f32 = 16.0;

/// The recharge of the local player's blows, as the server counts it.
#[derive(Resource, Default)]
pub struct LocalRecharge(pub Recharge);

/// The gauge, and its fill.
#[derive(Component)]
pub struct RechargeGauge;
#[derive(Component)]
pub struct RechargeFill;

/// The gauge as the accessibility tree and the tests read it.
pub fn recharge_label(charge: f32) -> String {
    let percent = (charge * 100.0).round() as u32;
    trf("hud.recharge.label", &[("percent", &percent)])
}

pub fn spawn_recharge_gauge(mut commands: Commands) {
    commands.spawn((
        RechargeGauge,
        Name::new(tr("hud.recharge")),
        AccessibleLabel::new(recharge_label(1.0)),
        DespawnOnExit(GameState::Game),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50.),
            top: percent(50.),
            width: px(GAUGE_WIDTH + 2.0),
            height: px(GAUGE_HEIGHT + 2.0),
            margin: UiRect {
                left: px(-(GAUGE_WIDTH + 2.0) / 2.0),
                top: px(GAUGE_BELOW),
                ..default()
            },
            padding: UiRect::all(px(1.0)),
            ..default()
        },
        BackgroundColor(theme::bevel_lo()),
        Visibility::Hidden,
        Pickable::IGNORE,
        children![(
            RechargeFill,
            Node {
                width: percent(100.),
                height: percent(100.),
                ..default()
            },
            BackgroundColor(theme::reticle()),
            Pickable::IGNORE,
        )],
    ));
}

/// Counts the frame's input toward the recharge, as the server will.
pub fn track_recharge(
    frame: Res<CurrentFrameInputs>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut recharge: ResMut<LocalRecharge>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let input = &frame.0;
    let pressed = input.is_pressed(shared::messages::NetworkAction::LeftClick);
    recharge
        .0
        .step(held_item(player, input), pressed, input.delta_ms);
}

/// Fills the gauge with the recharge of the item held, shown while it is not full and the
/// reticle shows.
pub fn update_recharge_gauge(
    frame: Res<CurrentFrameInputs>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    recharge: Res<LocalRecharge>,
    reticle: Query<&Visibility, (With<Reticle>, Without<RechargeGauge>)>,
    mut gauge: Query<(Entity, &mut Visibility), With<RechargeGauge>>,
    mut fill: Query<&mut Node, With<RechargeFill>>,
    mut shown: Local<Option<(Entity, u32)>>,
    mut commands: Commands,
) {
    let (Ok(player), Ok((entity, mut visibility))) = (player.single(), gauge.single_mut()) else {
        return;
    };
    let charge = recharge.0.charge(held_item(player, &frame.0));
    let reticle_shown = reticle.iter().all(|v| *v != Visibility::Hidden);
    let wanted = if charge < 1.0 && reticle_shown && !player.is_dead() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    visibility.set_if_neq(wanted);
    let percent_full = (charge * 100.0).round() as u32;
    // The gauge of another session, or of a HUD spawned again, starts afresh
    if *shown == Some((entity, percent_full)) {
        return;
    }
    *shown = Some((entity, percent_full));
    if let Ok(mut node) = fill.single_mut() {
        node.width = percent(percent_full as f32);
    }
    commands
        .entity(entity)
        .insert(AccessibleLabel::new(recharge_label(charge)));
}
