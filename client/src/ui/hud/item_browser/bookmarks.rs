//! The bookmarks of the item browser: the items the player keeps at hand, one ordered list per
//! world, saved by the client in `item_browser.ron` in the game folder. A world of this computer
//! is known by its name, a server by its address as the server list holds it.

use std::collections::BTreeMap;
use std::fs;

use bevy::prelude::*;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use shared::world::ItemId;
use shared::GameFolderPaths;

use crate::constants::ITEM_BROWSER_PATH;
use crate::network::session::SessionOrigin;
use crate::ui::hud::item_name;

/// Which list of `item_browser.ron` belongs to the world being played.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BookmarkKey {
    /// A world of this computer, by its name.
    World(String),
    /// A server, by its address as typed in the server list.
    Server(String),
}

impl BookmarkKey {
    /// The key of the session started from `origin`.
    pub fn of(origin: &SessionOrigin) -> Self {
        match origin {
            SessionOrigin::World { name } => BookmarkKey::World(name.clone()),
            SessionOrigin::Server { ip, .. } => BookmarkKey::Server(ip.clone()),
        }
    }
}

/// What `item_browser.ron` holds: the bookmarks of each world, and of each server, apart, so that
/// a world and a server never share a list.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct ItemBrowserFile {
    pub worlds: BTreeMap<String, Vec<ItemId>>,
    pub servers: BTreeMap<String, Vec<ItemId>>,
}

impl ItemBrowserFile {
    /// The lists `key` is one of.
    fn lists<'a>(
        &'a mut self,
        key: &'a BookmarkKey,
    ) -> (&'a mut BTreeMap<String, Vec<ItemId>>, &'a str) {
        match key {
            BookmarkKey::World(name) => (&mut self.worlds, name),
            BookmarkKey::Server(address) => (&mut self.servers, address),
        }
    }
}

/// The saved file, or an empty one when none was saved or it cannot be read.
pub fn read_item_browser_file(paths: &GameFolderPaths) -> ItemBrowserFile {
    let path = paths.game_folder_path.join(ITEM_BROWSER_PATH);
    fs::read_to_string(&path)
        .ok()
        .and_then(|content| ron::from_str(&content).ok())
        .unwrap_or_default()
}

/// The saved bookmarks of the world of `key`, in their order.
pub fn read_bookmarks(paths: &GameFolderPaths, key: &BookmarkKey) -> Vec<ItemId> {
    let mut file = read_item_browser_file(paths);
    let (lists, name) = file.lists(key);
    lists.remove(name).unwrap_or_default()
}

/// Saves `items` as the bookmarks of the world of `key`, keeping those of the other worlds. A
/// world without bookmark leaves the file.
pub fn write_bookmarks(paths: &GameFolderPaths, key: &BookmarkKey, items: &[ItemId]) {
    let mut file = read_item_browser_file(paths);
    let (lists, name) = file.lists(key);
    if items.is_empty() {
        lists.remove(name);
    } else {
        lists.insert(name.to_string(), items.to_vec());
    }
    let path = paths.game_folder_path.join(ITEM_BROWSER_PATH);
    let serialized = match ron::ser::to_string_pretty(&file, PrettyConfig::new()) {
        Ok(serialized) => serialized,
        Err(e) => {
            error!("Failed to serialize the bookmarks: {e}");
            return;
        }
    };
    if let Err(e) = fs::write(&path, serialized) {
        error!("Failed to save the bookmarks to {path:?}: {e}");
    }
}

/// The bookmarks of the world being played, and its key, `None` outside a session.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct Bookmarks {
    pub key: Option<BookmarkKey>,
    pub items: Vec<ItemId>,
}

impl Bookmarks {
    /// Whether `item` is bookmarked.
    pub fn contains(&self, item: ItemId) -> bool {
        self.items.contains(&item)
    }

    /// Bookmarks `item`, at the end of the list, or removes it when it is: whether it was added.
    pub fn toggle(&mut self, item: ItemId) -> bool {
        match self.items.iter().position(|&bookmarked| bookmarked == item) {
            Some(index) => {
                self.items.remove(index);
                false
            }
            None => {
                self.items.push(item);
                true
            }
        }
    }
}

/// Reads the bookmarks of the world being entered, known by the `SessionOrigin` of its session.
pub fn load_bookmarks(
    origin: Option<Res<SessionOrigin>>,
    paths: Res<GameFolderPaths>,
    mut bookmarks: ResMut<Bookmarks>,
) {
    let key = origin.map(|origin| BookmarkKey::of(&origin));
    let items = key
        .as_ref()
        .map_or_else(Vec::new, |key| read_bookmarks(&paths, key));
    *bookmarks = Bookmarks { key, items };
}

/// The toast saying that `item` was bookmarked, or removed from the bookmarks.
pub fn bookmark_message(item: ItemId, added: bool) -> String {
    let name = item_name(item);
    if added {
        format!("Bookmarked {name}")
    } else {
        format!("Removed {name} from bookmarks")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> GameFolderPaths {
        let dir = std::env::temp_dir().join(format!("rustcraft-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        }
    }

    fn world(name: &str) -> BookmarkKey {
        BookmarkKey::World(name.to_string())
    }

    #[test]
    fn the_bookmarks_of_a_world_are_saved_and_read_back() {
        let paths = temp_paths("bookmarks-roundtrip");
        let items = [ItemId::WoodenPickaxe, ItemId::Chest, ItemId::Dirt];
        write_bookmarks(&paths, &world("Survie"), &items);
        let read = read_bookmarks(&paths, &world("Survie"));
        let file = read_item_browser_file(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(read, items);
        assert_eq!(file.worlds.len(), 1);
        assert!(file.servers.is_empty());
    }

    #[test]
    fn each_world_and_each_server_has_its_own_bookmarks() {
        let paths = temp_paths("bookmarks-keys");
        let server = BookmarkKey::Server("Survie".to_string());
        write_bookmarks(&paths, &world("Survie"), &[ItemId::Chest]);
        write_bookmarks(&paths, &world("Creative"), &[ItemId::Sand, ItemId::Glass]);
        write_bookmarks(&paths, &server, &[ItemId::Stick]);
        let read = |key: &BookmarkKey| read_bookmarks(&paths, key);
        assert_eq!(read(&world("Survie")), [ItemId::Chest]);
        assert_eq!(read(&world("Creative")), [ItemId::Sand, ItemId::Glass]);
        assert_eq!(read(&server), [ItemId::Stick]);
        assert_eq!(read(&world("Other")), []);

        // An emptied list leaves the file, the others stay
        write_bookmarks(&paths, &world("Survie"), &[]);
        let file = read_item_browser_file(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(file.worlds.keys().collect::<Vec<_>>(), ["Creative"]);
        assert_eq!(file.servers["Survie"], [ItemId::Stick]);
    }

    #[test]
    fn a_missing_or_broken_file_has_no_bookmark() {
        let paths = temp_paths("bookmarks-broken");
        let file = paths.game_folder_path.join(ITEM_BROWSER_PATH);
        assert_eq!(read_bookmarks(&paths, &world("A")), []);
        for (content, expected) in [
            ("garbage", vec![]),
            ("()", vec![]),
            ("(worlds: {\"A\": [NotAnItem]})", vec![]),
            // a file without servers
            ("(worlds: {\"A\": [Chest]})", vec![ItemId::Chest]),
        ] {
            fs::write(&file, content).unwrap();
            assert_eq!(read_bookmarks(&paths, &world("A")), expected, "{content}");
        }
        // The next change overwrites a broken file
        fs::write(&file, "garbage").unwrap();
        write_bookmarks(&paths, &world("A"), &[ItemId::Dirt]);
        let read = read_bookmarks(&paths, &world("A"));
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(read, [ItemId::Dirt]);
    }

    #[test]
    fn toggling_adds_at_the_end_and_removes() {
        let mut bookmarks = Bookmarks::default();
        assert!(bookmarks.toggle(ItemId::Chest));
        assert!(bookmarks.toggle(ItemId::Dirt));
        assert!(bookmarks.contains(ItemId::Chest));
        assert_eq!(bookmarks.items, [ItemId::Chest, ItemId::Dirt]);
        assert!(!bookmarks.toggle(ItemId::Chest));
        assert!(!bookmarks.contains(ItemId::Chest));
        assert_eq!(bookmarks.items, [ItemId::Dirt]);
        assert_eq!(bookmark_message(ItemId::Chest, true), "Bookmarked Chest");
        assert_eq!(
            bookmark_message(ItemId::Chest, false),
            "Removed Chest from bookmarks"
        );
    }

    #[test]
    fn the_key_of_a_session() {
        let solo = SessionOrigin::World {
            name: "Survie".into(),
        };
        let server = SessionOrigin::Server {
            name: "Friends".into(),
            ip: "play.example.org:8000".into(),
            address: "127.0.0.1:8000".parse().unwrap(),
        };
        assert_eq!(BookmarkKey::of(&solo), world("Survie"));
        assert_eq!(
            BookmarkKey::of(&server),
            BookmarkKey::Server("play.example.org:8000".into())
        );
    }
}
