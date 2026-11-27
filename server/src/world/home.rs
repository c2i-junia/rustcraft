//! The signs and the bedrolls: what the signs say, where the players sleep, and the night they
//! sleep through. Both are kept in `places.ron`, in the folder of the world.

use bevy::prelude::*;
use bevy_renet::RenetServer;
use serde::{Deserialize, Serialize};
use shared::{
    messages::{
        FullChatMessage, PlayerId, ServerToClientMessage, SIGN_LINES, SIGN_LINE_LENGTH,
    },
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
const MAX_SIGN_LENGTH: usize = SIGN_LINES * SIGN_LINE_LENGTH;

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Places {
    pub signs: HashMap<IVec3, String>,
    /// The bedroll of each player, by name.
    pub bedrolls: HashMap<String, IVec3>,
    /// The paper map each frame of a map holds (`cartography`).
    #[serde(default)]
    pub frames: HashMap<IVec3, shared::world::cartography::MapId>,
    /// What each item frame and flower pot shows (`decor`).
    #[serde(default)]
    pub displays: HashMap<IVec3, shared::world::Shown>,
    /// The patterns of each banner that has some (`decor`).
    #[serde(default)]
    pub banners: HashMap<IVec3, shared::world::BannerPatterns>,
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

    pub fn save(&self, dir: Option<&Path>) {
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
    /// The frames of maps clicked, and the hotbar slots of the empty maps used
    /// (`cartography`).
    pub framed: Vec<(PlayerId, IVec3)>,
    pub drawn: Vec<(PlayerId, u32)>,
    /// The item frames and flower pots clicked (`decor`).
    pub displayed: Vec<(PlayerId, IVec3)>,
    /// Whether a frame, a pot or a banner changed since the decorations were sent.
    pub decorations_changed: bool,
    /// The players who were sent the decorations as they are.
    pub sent_decorations: HashSet<PlayerId>,
    sleeping: HashSet<PlayerId>,
}

/// Where `player` comes back to: its bedroll, while it is there, else the spawn of the world.
pub fn respawn_position(world_map: &ServerWorldMap, player: &Player) -> Vec3 {
    let Some(&pos) = world_map.places.bedrolls.get(&player.name) else {
        return world_map.spawn;
    };
    let chunk = shared::world::global_block_to_chunk_pos(&pos);
    let gone = world_map.chunks.is_chunk_loaded(chunk)
        && !world_map
            .chunks
            .get_block(pos)
            .is_some_and(|b| b.id.is_bedroll());
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

/// Writes `lines` on the sign at `at`, which the player `id` wrote in the editor: it must still be
/// a sign, within its reach. Control characters are left out, and each line cut to
/// `SIGN_LINE_LENGTH`; a sign without a word is empty again.
pub fn write_sign_lines(
    world_map: &mut ServerWorldMap,
    id: PlayerId,
    at: IVec3,
    lines: [String; SIGN_LINES],
) {
    let Some(player) = world_map.players.get(&id) else {
        return;
    };
    let reach = shared::players::blocks::INTERACTION_DISTANCE + 2.0;
    let is_sign = world_map.chunks.get_block(at).map(|b| b.id) == Some(BlockId::Sign);
    if !is_sign || (at.as_vec3() + Vec3::splat(0.5)).distance(player.position) > reach {
        return;
    }
    let text = sign_text(&lines);
    if text.is_empty() {
        world_map.places.signs.remove(&at);
    } else {
        world_map.places.signs.insert(at, text);
    }
    world_map.places.save(world_map.saved.as_deref());
}

/// The text kept for `lines`: each cleaned and cut, joined by line breaks, without the empty
/// lines at the end.
fn sign_text(lines: &[String; SIGN_LINES]) -> String {
    let lines: Vec<String> = lines
        .iter()
        .map(|line| {
            let kept: String = line
                .chars()
                .filter(|c| !c.is_control())
                .take(SIGN_LINE_LENGTH)
                .collect();
            kept.trim_end().to_string()
        })
        .collect();
    lines.join("\n").trim_end_matches('\n').to_string()
}

/// The lines of the text of a sign, as the editor shows them: a text written by `/sign` on one
/// line is wrapped between its words.
pub fn sign_lines(text: &str) -> [String; SIGN_LINES] {
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let length = line.chars().count();
            if length > 0 && length + 1 + word.chars().count() > SIGN_LINE_LENGTH {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            // a word longer than a line is cut
            while line.chars().count() > SIGN_LINE_LENGTH {
                let rest: String = line.chars().skip(SIGN_LINE_LENGTH).collect();
                lines.push(line.chars().take(SIGN_LINE_LENGTH).collect());
                line = rest;
            }
        }
        lines.push(line);
    }
    std::array::from_fn(|index| lines.get(index).cloned().unwrap_or_default())
}

pub fn home_system(
    mut world_map: ResMut<ServerWorldMap>,
    mut time: ResMut<ServerTime>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
) {
    let world_map = world_map.as_mut();
    let mut notices = Vec::new();
    // A sign placed or clicked opens its editor, which shows what it says
    for (id, at) in std::mem::take(&mut world_map.home.read) {
        let text = world_map.places.signs.get(&at).cloned().unwrap_or_default();
        let lines = sign_lines(&text);
        server.send_game_message(id, ServerToClientMessage::EditSign { at, lines });
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
        // A night slept away is not lived through
        for player in world_map.players.values_mut() {
            player.progress.night = None;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_of_a_sign_are_kept_cut_and_given_back() {
        let lines = [
            "Home".to_string(),
            "of the\tbuilder".to_string(),
            "x".repeat(40),
            String::new(),
        ];
        let text = sign_text(&lines);
        assert_eq!(text, format!("Home\nof thebuilder\n{}", "x".repeat(30)));
        assert_eq!(
            sign_lines(&text),
            [
                "Home".to_string(),
                "of thebuilder".to_string(),
                "x".repeat(30),
                String::new()
            ]
        );
        assert_eq!(sign_text(&Default::default()), "");
        // a text of `/sign`, on one line, is wrapped between its words
        let long = "The way to the mine goes down past the old well and the lake";
        let wrapped = sign_lines(long);
        assert_eq!(wrapped[0], "The way to the mine goes down");
        assert_eq!(wrapped[1], "past the old well and the lake");
        assert!(wrapped.iter().all(|line| line.chars().count() <= SIGN_LINE_LENGTH));
    }
    use crate::init::LobbyPlayer;
    use bevy::ecs::system::RunSystemOnce;
    use bevy_renet::renet::ConnectionConfig;
    use shared::players::progress::Achievement;
    use shared::players::Player;

    #[test]
    fn a_night_slept_away_is_skipped_and_is_not_one_lived_through() {
        let mut world_map = ServerWorldMap::default();
        let mut player = Player::new(1, "sleeper".into(), Vec3::ZERO, Transform::default());
        player.watch_the_night(0);
        world_map.players.insert(1, player);
        world_map.home.slept.push((1, IVec3::ZERO));
        let mut lobby = ServerLobby::default();
        lobby.players.insert(1, LobbyPlayer::new("sleeper".into()));
        let mut world = World::new();
        world.insert_resource(world_map);
        world.insert_resource(lobby);
        world.insert_resource(ServerTime(100));
        world.insert_resource(RenetServer::new(ConnectionConfig::default()));
        world.run_system_once(home_system).unwrap();

        let dawn = world.resource::<ServerTime>().0;
        assert_eq!(dawn, DAY_DURATION_IN_TICKS / 2);
        let mut world_map = world.resource_mut::<ServerWorldMap>();
        let player = world_map.players.get_mut(&1).unwrap();
        player.watch_the_night(dawn);
        assert!(!player.progress.has(Achievement::Night));
    }
}
