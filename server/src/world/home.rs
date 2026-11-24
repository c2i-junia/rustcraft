//! The signs and the bedrolls: what the signs say, where the players sleep, and the night they
//! sleep through. Both are kept in `places.ron`, in the folder of the world.

use bevy::prelude::*;
use bevy_renet::RenetServer;
use serde::{Deserialize, Serialize};
use shared::{
    messages::{FullChatMessage, PlayerId, ServerToClientMessage},
    players::{blocks::aim, Player},
    world::BlockId,
    DAY_DURATION_IN_TICKS,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::data::ServerWorldMap;
use crate::init::{ServerLobby, ServerTime};
use crate::network::{commands::SERVER_AUTHOR, extensions::SendGameMessageExtension};

const PLACES_FILE: &str = "places.ron";
const MAX_SIGN_LENGTH: usize = 120;

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Places {
    pub signs: HashMap<IVec3, String>,
    /// The bedroll of each player, by name.
    pub bedrolls: HashMap<String, IVec3>,
}

impl Places {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(PLACES_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        ron::de::from_str(&text).unwrap_or_else(|err| {
            // Kept aside: the next change would write over it
            error!("Could not read {}: {err}", path.display());
            let _ = std::fs::rename(&path, path.with_extension("ron.corrupt"));
            Self::default()
        })
    }

    fn save(&self, dir: Option<&Path>) {
        let Some(dir) = dir else {
            return;
        };
        let written = ron::ser::to_string(self)
            .map_err(|err| err.to_string())
            .and_then(|text| {
                let part = dir.join("places.ron.part");
                std::fs::create_dir_all(dir)
                    .and_then(|()| std::fs::write(&part, text))
                    .and_then(|()| std::fs::rename(&part, dir.join(PLACES_FILE)))
                    .map_err(|err| err.to_string())
            });
        if let Err(err) = written {
            error!("Could not save the signs and the bedrolls: {err}");
        }
    }
}

/// What the players did with signs and bedrolls since the last tick, and who sleeps.
#[derive(Default, Debug)]
pub struct HomeEvents {
    pub read: Vec<(PlayerId, IVec3)>,
    pub slept: Vec<(PlayerId, IVec3)>,
    sleeping: HashSet<PlayerId>,
}

/// Where `player` comes back to: its bedroll, while it is there, else the spawn of the world.
pub fn respawn_position(world_map: &ServerWorldMap, player: &Player) -> Vec3 {
    let Some(&pos) = world_map.places.bedrolls.get(&player.name) else {
        return world_map.spawn;
    };
    let chunk = shared::world::global_block_to_chunk_pos(&pos);
    let gone = world_map.chunks.is_chunk_loaded(chunk)
        && world_map.chunks.get_block(pos).map(|b| b.id) != Some(BlockId::Bedroll);
    if gone {
        return world_map.spawn;
    }
    pos.as_vec3() + Vec3::new(0.5, 0.25 + player.height / 2.0, 0.5)
}

/// Writes `text` on the sign `player` looks at, and returns the answer for it.
pub fn write_sign(world_map: &mut ServerWorldMap, id: PlayerId, text: &str) -> String {
    let Some(player) = world_map.players.get(&id) else {
        return String::new();
    };
    let target = aim(
        &world_map.chunks,
        &player.camera_transform,
        &player.position,
        Default::default(),
        None,
    );
    let Some(target) = target.filter(|target| {
        target.block.id == BlockId::Sign
            && target.position.as_vec3().distance(player.position)
                <= shared::players::blocks::INTERACTION_DISTANCE + 1.0
    }) else {
        return "Look at a sign to write on it".to_string();
    };
    let text: String = text.trim().chars().take(MAX_SIGN_LENGTH).collect();
    world_map.places.signs.insert(target.position, text);
    world_map.places.save(world_map.saved.as_deref());
    "Written on the sign".to_string()
}

pub fn home_system(
    mut world_map: ResMut<ServerWorldMap>,
    mut time: ResMut<ServerTime>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
) {
    let world_map = world_map.as_mut();
    let mut notices = Vec::new();
    for (id, pos) in std::mem::take(&mut world_map.home.read) {
        let text = world_map.places.signs.get(&pos).cloned();
        notices.push((
            id,
            text.unwrap_or_else(|| "An empty sign: /sign <text> writes on it".into()),
        ));
    }
    let night = is_night(time.0);
    for (id, pos) in std::mem::take(&mut world_map.home.slept) {
        let Some(player) = world_map.players.get(&id) else {
            continue;
        };
        let name = player.name.clone();
        if world_map.places.bedrolls.insert(name, pos) != Some(pos) {
            world_map.places.save(world_map.saved.as_deref());
        }
        if night {
            world_map.home.sleeping.insert(id);
            notices.push((id, "You will come back here. Sleeping...".into()));
        } else {
            notices.push((id, "You will come back here".into()));
        }
    }
    // A sign broken since, and another put in its place, starts empty
    let chunks = &world_map.chunks;
    for pos in chunks.changed_blocks() {
        if chunks.get_block(pos).map(|b| b.id) != Some(BlockId::Sign) {
            world_map.places.signs.remove(&pos);
        }
    }
    if !night {
        world_map.home.sleeping.clear();
    } else if !world_map.home.sleeping.is_empty()
        && lobby
            .players
            .keys()
            .all(|id| world_map.home.sleeping.contains(id))
    {
        // The night is the first half of a day
        time.0 = time.0 - time.0 % DAY_DURATION_IN_TICKS + DAY_DURATION_IN_TICKS / 2;
        world_map.home.sleeping.clear();
        for &id in lobby.players.keys() {
            server.send_game_message(id, ServerToClientMessage::TimeSet(time.0));
        }
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64);
    for (id, content) in notices {
        let message = FullChatMessage {
            author: SERVER_AUTHOR.to_string(),
            content,
            timestamp,
        };
        server.send_game_message(id, ServerToClientMessage::ChatMessage(message));
    }
}

fn is_night(tick: u64) -> bool {
    tick % DAY_DURATION_IN_TICKS < DAY_DURATION_IN_TICKS / 2
}
