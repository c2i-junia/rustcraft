//! The maps of the server (`shared::world::cartography`): the paper maps, drawn from an empty
//! map, filled around their holders, put in frames and sent to who may see them; and the
//! waypoints of the players, the places they find by going near them included.
//!
//! The paper maps are kept in the folder `maps` of the world, one file each, written with the
//! world (`save::queue_save`). The frames are kept with the signs, in `places.ron`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use bevy::prelude::*;
use bevy_renet::RenetServer;
use shared::messages::{PlayerId, ServerToClientMessage};
use shared::players::Player;
use shared::world::cartography::{
    paper_map_centre, MapId, PaperMap, PlaceKind, WaypointsView, PAPER_MAP_REACH,
};
use shared::world::{BlockId, DroppedItem, ItemId, ItemStack, ItemType};
use shared::{CHUNK_SIZE, MAX_INVENTORY_SLOTS};

use super::data::ServerWorldMap;
use super::generation::dungeon::{self, DungeonKind};
use super::generation::village;
use super::home::respawn_position;
use super::simulation::LoadedGenerator;
use crate::init::{ServerLobby, ServerTime};
use crate::network::extensions::SendGameMessageExtension;

const MAPS_DIR: &str = "maps";

/// The ticks between two fillings of the held maps.
const FILL_EVERY: u64 = 5;
/// The ticks between two sendings of the maps that changed.
const SEND_EVERY: u64 = 10;
/// The ticks between two looks for the places near the players.
const FIND_EVERY: u64 = 20;
/// A player finds a village within this many blocks of its centre.
const VILLAGE_FOUND: f32 = 48.0;
/// A player finds a dungeon within this many blocks of its walls.
const DUNGEON_FOUND: i32 = 8;
/// A player finds the lair within this many blocks of its centre.
const LAIR_FOUND: f32 = 40.0;
/// The chunks above and below its holder a map looks for the surface in.
const CHUNKS_ABOVE: i32 = 6;
const CHUNKS_BELOW: i32 = 4;

/// The paper maps of the world, and what each player was sent of them and of its waypoints.
#[derive(Default, Debug)]
pub struct PaperMaps {
    /// Whether the maps of the folder of the world were read.
    loaded: bool,
    next_id: MapId,
    maps: HashMap<MapId, PaperMap>,
    /// Grows with every change of a map, so that it is sent again.
    revisions: HashMap<MapId, u64>,
    /// The maps changed since they were last written.
    unsaved: HashSet<MapId>,
    /// The revision of each map each player was sent.
    sent: HashMap<PlayerId, HashMap<MapId, u64>>,
    sent_waypoints: HashMap<PlayerId, WaypointsView>,
    /// The players who were sent the frames as they are.
    sent_frames: HashSet<PlayerId>,
}

impl PaperMaps {
    /// Reads the maps of the world in `dir`, once.
    fn load(&mut self, dir: Option<&Path>) {
        if std::mem::replace(&mut self.loaded, true) {
            return;
        }
        let Some(Ok(entries)) = dir.map(|dir| std::fs::read_dir(dir.join(MAPS_DIR))) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<MapId>().ok());
            let Some(id) = id.filter(|_| path.extension().is_some_and(|ext| ext == "map")) else {
                continue;
            };
            let map = std::fs::read(&path)
                .map_err(|err| err.to_string())
                .and_then(|bytes| {
                    shared::payload_to_game_message::<PaperMap>(&bytes).map_err(|e| e.to_string())
                });
            match map {
                Ok(map) => {
                    self.maps.insert(id, map);
                    self.next_id = self.next_id.max(id + 1);
                }
                Err(err) => error!("Could not read the map {}: {err}", path.display()),
            }
        }
    }

    /// A new map centred for `position`, filled around it in `world_map`: its id.
    fn draw(&mut self, map: PaperMap) -> MapId {
        let id = self.next_id;
        self.next_id += 1;
        self.maps.insert(id, map);
        self.changed(id);
        id
    }

    fn changed(&mut self, id: MapId) {
        *self.revisions.entry(id).or_default() += 1;
        self.unsaved.insert(id);
    }

    /// Writes the maps changed since the last time into the folder of the world `dir`.
    pub fn save(&mut self, dir: Option<&Path>) {
        let Some(dir) = dir else {
            return;
        };
        let folder = dir.join(MAPS_DIR);
        for id in std::mem::take(&mut self.unsaved) {
            let Some(map) = self.maps.get(&id) else {
                continue;
            };
            let part = folder.join(format!("{id}.map.part"));
            let written = std::fs::create_dir_all(&folder)
                .and_then(|()| std::fs::write(&part, shared::game_message_to_payload(map)))
                .and_then(|()| std::fs::rename(&part, folder.join(format!("{id}.map"))));
            if let Err(err) = written {
                error!("Could not save the map {id}: {err}");
                self.unsaved.insert(id);
            }
        }
    }
}

/// The id of the paper map of `stack`, if it is one.
fn map_id(stack: &ItemStack) -> Option<MapId> {
    match (stack.item_id, stack.item_type) {
        (ItemId::FilledMap, ItemType::Map { id }) => Some(id),
        _ => None,
    }
}

fn map_stack(id: MapId) -> ItemStack {
    ItemStack {
        item_id: ItemId::FilledMap,
        item_type: ItemType::Map { id },
        nb: 1,
    }
}

/// The chunks of the columns a map held at `position` looks through, from the top.
fn heights(position: Vec3) -> (i32, i32) {
    let chunk = (position.y.floor() as i32).div_euclid(CHUNK_SIZE);
    (chunk + CHUNKS_ABOVE, chunk - CHUNKS_BELOW)
}

/// Gives `stack` to `player`, in `slot` if it is empty, else where it fits, else at its feet.
fn give(world_map: &mut ServerWorldMap, id: PlayerId, slot: u32, stack: ItemStack) {
    let Some(player) = world_map.players.get_mut(&id) else {
        return;
    };
    if !player.inventory.inner.contains_key(&slot) {
        player.inventory.inner.insert(slot, stack);
        return;
    }
    if player.inventory.add_item_to_inventory(stack) > 0 {
        world_map.dropped_items.drop_from_player(stack, player);
    }
}

/// What `player` is told of its waypoints.
fn view_of(world_map: &ServerWorldMap, player: &Player) -> WaypointsView {
    let home = respawn_position(world_map, player);
    let bedroll = world_map.places.bedrolls.contains_key(&player.name) && home != world_map.spawn;
    WaypointsView {
        home: home.floor().as_ivec3(),
        bedroll,
        waypoints: player.waypoints.clone(),
    }
}

/// The places near `player` it has not found yet.
fn places_near(generator: &LoadedGenerator, player: &Player) -> Vec<(PlaceKind, IVec3)> {
    let generator = &generator.0;
    let at = player.position;
    let column = at.floor().as_ivec3();
    let mut found = Vec::new();
    let region = village::region_of(column.x, column.z);
    if let Some(plan) = generator.village(region) {
        if (plan.centre.as_vec2() - at.xz()).length() <= VILLAGE_FOUND {
            found.push((PlaceKind::Village, plan.centre.extend(column.y).xzy()));
        }
    }
    if generator.has_dungeons() {
        let region = dungeon::region_of(column.x, column.z);
        for kind in DungeonKind::ALL {
            let Some(plan) = generator.dungeon(kind, region) else {
                continue;
            };
            let near = column.cmpge(plan.min - DUNGEON_FOUND).all()
                && column.cmple(plan.max + DUNGEON_FOUND).all();
            if near {
                let place = match kind {
                    DungeonKind::Ruin => PlaceKind::Ruin,
                    DungeonKind::Crypt => PlaceKind::Crypt,
                    DungeonKind::Sanctuary => PlaceKind::Sanctuary,
                };
                found.push((place, (plan.min + plan.max) / 2));
            }
        }
    }
    if let Some(lair) = generator.lair() {
        if lair.centre.as_vec3().distance(at) <= LAIR_FOUND {
            found.push((PlaceKind::Lair, lair.centre));
        }
    }
    found
}

/// Draws the empty maps used, puts the maps on the frames clicked or takes them back, fills
/// the held maps, finds the places near the players, and sends each player what changed of its
/// maps, of the frames and of its waypoints.
pub fn cartography_system(
    mut world_map: ResMut<ServerWorldMap>,
    time: Res<ServerTime>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
    generator: Option<Res<LoadedGenerator>>,
) {
    let world_map = world_map.as_mut();
    world_map.paper_maps.load(world_map.saved.as_deref());

    // The empty maps used are drawn where their user stands
    for (id, slot) in std::mem::take(&mut world_map.home.drawn) {
        let Some(player) = world_map.players.get_mut(&id) else {
            continue;
        };
        let empty = player.inventory.inner.get(&slot).map(|stack| stack.item_id);
        if empty != Some(ItemId::EmptyMap) {
            continue;
        }
        player.inventory.remove_item_from_stack(slot, 1);
        let mut map = PaperMap::new(paper_map_centre(player.position));
        map.fill(
            &world_map.chunks,
            player.position,
            PAPER_MAP_REACH,
            heights(player.position),
        );
        let map_id = world_map.paper_maps.draw(map);
        give(world_map, id, slot, map_stack(map_id));
    }

    // A frame clicked takes the map held, or gives back the map it holds
    let mut frames_changed = false;
    for (id, at) in std::mem::take(&mut world_map.home.framed) {
        let is_frame = world_map.chunks.get_block(at).map(|b| b.id) == Some(BlockId::MapFrame);
        let Some(slot) = world_map
            .players
            .get(&id)
            .filter(|_| is_frame)
            .map(|p| p.hotbar_slot)
        else {
            continue;
        };
        if let Some(map) = world_map.places.frames.remove(&at) {
            give(world_map, id, slot, map_stack(map));
            frames_changed = true;
            continue;
        }
        let Some(player) = world_map.players.get_mut(&id) else {
            continue;
        };
        if let Some(map) = player.inventory.inner.get(&slot).and_then(map_id) {
            player.inventory.inner.remove(&slot);
            world_map.places.frames.insert(at, map);
            frames_changed = true;
        }
    }
    // A frame broken drops its map
    let broken: Vec<IVec3> = world_map
        .places
        .frames
        .keys()
        .copied()
        .filter(|&at| {
            world_map
                .chunks
                .is_chunk_loaded(shared::world::global_block_to_chunk_pos(&at))
                && world_map.chunks.get_block(at).map(|b| b.id) != Some(BlockId::MapFrame)
        })
        .collect();
    for at in broken {
        if let Some(map) = world_map.places.frames.remove(&at) {
            let centre = at.as_vec3() + Vec3::splat(0.5);
            world_map
                .dropped_items
                .spawn(DroppedItem::new(map_stack(map), centre, Vec3::Y * 2.0));
            frames_changed = true;
        }
    }
    if frames_changed {
        world_map.places.save(world_map.saved.as_deref());
        world_map.paper_maps.sent_frames.clear();
    }

    // The map in the hand fills around its holder
    if time.0 % FILL_EVERY == 0 {
        for player in world_map.players.values() {
            if player.is_dead() {
                continue;
            }
            let held = player.inventory.inner.get(&player.hotbar_slot);
            let Some(id) = held.and_then(map_id) else {
                continue;
            };
            let Some(map) = world_map.paper_maps.maps.get_mut(&id) else {
                continue;
            };
            let filled = map.fill(
                &world_map.chunks,
                player.position,
                PAPER_MAP_REACH,
                heights(player.position),
            );
            if filled {
                world_map.paper_maps.changed(id);
            }
        }
    }

    // The places near the players
    if let Some(generator) = generator.filter(|_| time.0 % FIND_EVERY == 0) {
        for player in world_map.players.values_mut() {
            if player.is_dead() {
                continue;
            }
            for (kind, at) in places_near(&generator, player) {
                player.waypoints.find(kind, at);
            }
        }
    }

    // What each player may see
    let maps = &mut world_map.paper_maps;
    maps.sent.retain(|id, _| lobby.players.contains_key(id));
    maps.sent_waypoints
        .retain(|id, _| lobby.players.contains_key(id));
    maps.sent_frames.retain(|id| lobby.players.contains_key(id));
    let framed: Vec<(IVec3, MapId)> = world_map
        .places
        .frames
        .iter()
        .map(|(&at, &id)| (at, id))
        .collect();
    for &id in lobby.players.keys() {
        let Some(player) = world_map.players.get(&id) else {
            continue;
        };
        if world_map.paper_maps.sent_frames.insert(id) {
            server.send_game_message(id, ServerToClientMessage::FramedMaps(framed.clone()));
        }
        let view = view_of(world_map, player);
        let maps = &mut world_map.paper_maps;
        if maps.sent_waypoints.get(&id) != Some(&view) {
            maps.sent_waypoints.insert(id, view.clone());
            server.send_game_message(id, ServerToClientMessage::Waypoints(view));
        }
        if time.0 % SEND_EVERY != 0 {
            continue;
        }
        let held = (0..MAX_INVENTORY_SLOTS)
            .filter_map(|slot| player.inventory.inner.get(&slot))
            .chain(player.inventory.floating.as_ref())
            .filter_map(map_id);
        let seen: HashSet<MapId> = held.chain(framed.iter().map(|&(_, id)| id)).collect();
        for map_id in seen {
            let (Some(map), Some(&revision)) =
                (maps.maps.get(&map_id), maps.revisions.get(&map_id))
            else {
                // a map read from the folder and not changed since: sent once
                if let Some(map) = maps.maps.get(&map_id) {
                    let sent = maps.sent.entry(id).or_default();
                    if !sent.contains_key(&map_id) {
                        sent.insert(map_id, 0);
                        let map = map.clone();
                        server.send_game_message(
                            id,
                            ServerToClientMessage::PaperMap { id: map_id, map },
                        );
                    }
                }
                continue;
            };
            let sent = maps.sent.entry(id).or_default();
            if sent.get(&map_id) != Some(&revision) {
                sent.insert(map_id, revision);
                let map = map.clone();
                server.send_game_message(id, ServerToClientMessage::PaperMap { id: map_id, map });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maps_are_saved_and_read_back_with_their_ids() {
        let dir = tempfile::tempdir().unwrap();
        let mut maps = PaperMaps {
            loaded: true,
            ..Default::default()
        };
        let first = maps.draw(PaperMap::new(IVec2::ZERO));
        let second = maps.draw(PaperMap::new(IVec2::new(128, 0)));
        assert_eq!((first, second), (0, 1));
        maps.save(Some(dir.path()));
        assert!(maps.unsaved.is_empty());

        let mut read = PaperMaps::default();
        read.load(Some(dir.path()));
        assert_eq!(
            read.maps.get(&second).map(|map| map.centre),
            Some(IVec2::new(128, 0))
        );
        assert_eq!(read.draw(PaperMap::new(IVec2::ZERO)), 2);
    }
}
