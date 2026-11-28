//! What the players achieve: what is told by where they are, what they hold and the hour is
//! looked at here, the rest is earned where it happens (`Player::earn`). Each player is told of
//! its own progress.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_renet::RenetServer;
use shared::messages::{PlayerId, ServerToClientMessage};
use shared::players::progress::{Achievement, Progress};
use shared::world::depths::DepthTier;

use super::data::ServerWorldMap;
use super::generation::dungeon::{region_of, DungeonKind};
use super::simulation::LoadedGenerator;
use crate::init::{ServerLobby, ServerTime};
use crate::network::extensions::SendGameMessageExtension;

/// Ticks between two looks at the players.
const LOOK_INTERVAL_TICKS: u64 = 10;
/// How near a creature is met, in blocks.
const SEEN_RADIUS: f32 = 24.0;
/// Ticks between two messages when only the statistics changed.
const STATS_INTERVAL_TICKS: u64 = 20;

/// Looks at what the players reached, then tells each of its progress, when it changed. Runs once
/// the inputs of the tick were applied.
pub fn progress_system(
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
    generator: Option<Res<LoadedGenerator>>,
    lobby: Res<ServerLobby>,
    mut server: ResMut<RenetServer>,
    mut told: Local<HashMap<PlayerId, Progress>>,
) {
    for player in world_map.players.values_mut() {
        player.count_tick();
    }
    told.retain(|id, _| lobby.players.contains_key(id));
    if time.0.is_multiple_of(LOOK_INTERVAL_TICKS) {
        let ServerWorldMap {
            chunks,
            players,
            boss,
            mobs,
            ..
        } = &mut *world_map;
        // What a player had when it joined is told first: the rest is news
        for player in players.values_mut().filter(|p| told.contains_key(&p.id)) {
            player.watch_the_night(time.0);
            if player.is_dead() {
                continue;
            }
            for mob in mobs.values() {
                if mob.position.distance_squared(player.position) <= SEEN_RADIUS * SEEN_RADIUS {
                    player.see(mob.kind);
                }
            }
            player.earn_possessions();
            if let Some(tier) = DepthTier::of_player(chunks, player) {
                player.earn(Achievement::of_tier(tier));
            }
            if boss.holds(player.position) {
                player.earn(Achievement::Lair);
            }
            let Some(generator) = &generator else {
                continue;
            };
            if !player.progress.has(Achievement::Dungeon) {
                let at = player.position.floor().as_ivec3();
                let inside = DungeonKind::ALL.into_iter().any(|kind| {
                    generator
                        .0
                        .dungeon(kind, region_of(at.x, at.z))
                        .is_some_and(|plan| plan.touches(at, at))
                });
                if inside {
                    player.earn(Achievement::Dungeon);
                }
            }
        }
    }
    for (&id, player) in &world_map.players {
        if !lobby.players.contains_key(&id) {
            continue;
        }
        let progress = &player.progress;
        let fresh = told.get(&id).is_none_or(|told| {
            let stats_only = Progress {
                stats: progress.stats,
                ..told.clone()
            } == *progress;
            !stats_only || (told != progress && time.0.is_multiple_of(STATS_INTERVAL_TICKS))
        });
        if fresh {
            // Those it had when it joined are not news
            let known = told
                .get(&id)
                .map_or(progress.achievements.len(), |told| told.achievements.len());
            let earned = progress.achievements.get(known..).unwrap_or_default();
            let earned = earned.to_vec();
            let progress = progress.clone();
            told.insert(id, progress.clone());
            server.send_game_message(id, ServerToClientMessage::Progress { progress, earned });
        }
    }
}
