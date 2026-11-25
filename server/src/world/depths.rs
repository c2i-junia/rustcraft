//! Tells each player that reaches a depth tier for the first time.

use bevy::prelude::*;
use bevy_renet::RenetServer;
use shared::{messages::ServerToClientMessage, world::depths::DepthTier};

use super::data::ServerWorldMap;
use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;

/// Ticks between two looks at how deep the players are.
const LOOK_INTERVAL_TICKS: u64 = 10;

/// Remembers the tiers the living players are in, saved with them, and announces each the first
/// time. Runs once the inputs of the tick moved the players.
pub fn announce_depths_system(
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
    mut server: ResMut<RenetServer>,
) {
    if !time.0.is_multiple_of(LOOK_INTERVAL_TICKS) {
        return;
    }
    let ServerWorldMap {
        chunks, players, ..
    } = &mut *world_map;
    for player in players.values_mut().filter(|player| !player.is_dead()) {
        let Some(tier) = DepthTier::of_player(chunks, player) else {
            continue;
        };
        if !player.depths_reached.contains(&tier) {
            player.depths_reached.push(tier);
            server.send_game_message(player.id, ServerToClientMessage::DepthReached(tier));
        }
    }
}
