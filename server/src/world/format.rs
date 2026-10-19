//! What is written to a world's folder and how: the atomic write every file goes through, and
//! the types of `world.ron`. The files of the chunks are in `chunk_files`, those of the
//! players in `save`.

use bevy::log::debug;
use bevy::math::IVec3;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use shared::world::{Chunk, Container, DroppedItem, MobId, ServerMob, WorldSeed};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Name of the file of a world that holds its metadata.
pub const WORLD_FILE: &str = "world.ron";

/// Version of the layout written by this code: `world.ron` is the metadata only and the chunks
/// are in the files of `chunks/`. The single-file formats that preceded it have no version.
/// Version 1 had the same layout, but its blocks had no level. Version 2 had no `containers/`:
/// it reads as a version 3 world without chests. Version 3 wrote the chunks cell by cell, where
/// version 4 writes the runs of equal cells: the reader of a chunk takes both.
///
/// Bump it when the files written to a world's folder change in a way an older reader would
/// misread (the layout, `WorldMeta`, the chunk encoding), and add a conversion from the
/// previous version on load. Adding a `BlockId` at the end of its enum needs no bump.
pub const FORMAT_VERSION: u32 = 4;

/// What `world.ron` holds: everything about a world but its chunks and players.
#[derive(Serialize, Deserialize)]
pub struct WorldMeta {
    pub format_version: u32,
    pub name: String,
    pub seed: WorldSeed,
    pub time: u64,
    pub mobs: HashMap<MobId, ServerMob>,
    /// Missing from the worlds saved before items could lie on the ground. The former feature
    /// of the same kind wrote `item_stacks`, which is ignored.
    #[serde(default)]
    pub dropped_items: Vec<DroppedItem>,
}

impl WorldMeta {
    pub fn new(name: String, seed: WorldSeed, time: u64, mobs: HashMap<MobId, ServerMob>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name,
            seed,
            time,
            mobs,
            dropped_items: Vec::new(),
        }
    }
}

/// A whole world as it is loaded: the metadata and every chunk. It is also the `world.ron` of
/// the single-file format that stored the chunks as sequences of cells.
#[derive(Serialize, Deserialize, Default)]
pub struct WorldData {
    pub map: HashMap<IVec3, Chunk>,
    /// The contents of the chests, by global position. The single-file formats had none.
    #[serde(skip)]
    pub containers: HashMap<IVec3, Container>,
    pub mobs: HashMap<MobId, ServerMob>,
    pub seed: WorldSeed,
    pub name: String,
    pub time: u64,
    #[serde(default)]
    pub dropped_items: Vec<DroppedItem>,
}

impl From<&WorldData> for WorldMeta {
    fn from(world: &WorldData) -> Self {
        Self {
            dropped_items: world.dropped_items.clone(),
            ..Self::new(
                world.name.clone(),
                world.seed,
                world.time,
                world.mobs.clone(),
            )
        }
    }
}

/// The world of `meta`, without its chunks.
impl From<WorldMeta> for WorldData {
    fn from(meta: WorldMeta) -> Self {
        Self {
            mobs: meta.mobs,
            seed: meta.seed,
            name: meta.name,
            time: meta.time,
            dropped_items: meta.dropped_items,
            ..Self::default()
        }
    }
}

/// Writes `world.ron` of the world stored in `world_dir`.
pub fn write_meta(world_dir: &Path, meta: &WorldMeta) -> Result<(), Box<dyn std::error::Error>> {
    let serialized = ron::ser::to_string_pretty(meta, PrettyConfig::new())?;
    write_atomically(&world_dir.join(WORLD_FILE), serialized.as_bytes())?;
    Ok(())
}

/// Writes `world.ron` of the world stored in `world_dir` as a file to commit.
pub fn write_meta_pending(
    world_dir: &Path,
    meta: &WorldMeta,
) -> Result<PendingFile, Box<dyn std::error::Error>> {
    let serialized = ron::ser::to_string_pretty(meta, PrettyConfig::new())?;
    Ok(write_pending(
        &world_dir.join(WORLD_FILE),
        serialized.as_bytes(),
    )?)
}

/// Suffix of the temporary file an atomic write goes through.
pub const TEMPORARY_SUFFIX: &str = ".tmp";

/// Writes `contents` to `<path>.tmp`, syncs it, then renames it over `path`, so a crash
/// leaves either the old file or the new one, never a truncated one.
pub fn write_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(TEMPORARY_SUFFIX);
    let tmp_path = Path::new(&tmp_name);

    let mut file = File::create(tmp_path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    std::fs::rename(tmp_path, path)
}

/// A file written to `<path>.tmp`, neither synced nor in place yet: `commit` makes a batch of
/// them durable, then puts each in place.
pub struct PendingFile {
    tmp: PathBuf,
    path: PathBuf,
}

impl PendingFile {
    /// The file it replaces once committed.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Writes `contents` to `<path>.tmp`, to be committed with others.
pub fn write_pending(path: &Path, contents: &[u8]) -> io::Result<PendingFile> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(TEMPORARY_SUFFIX);
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, contents)?;
    Ok(PendingFile {
        tmp,
        path: path.to_owned(),
    })
}

/// Syncs the files of `pending`, all on the file system of `dir`, then renames each over the
/// file it replaces, in order: as with `write_atomically`, a crash leaves either the old file or
/// the new one, never a truncated one. On Linux it is one sync of the file system for the whole
/// batch, where syncing each file costs milliseconds. The result of each file, in order.
pub fn commit(dir: &Path, pending: Vec<PendingFile>) -> Vec<io::Result<()>> {
    let synced = sync_file_system(dir);
    if let Err(err) = &synced {
        debug!("Syncing the files one by one: {err}");
    }
    pending
        .into_iter()
        .map(|file| {
            if synced.is_err() {
                OpenOptions::new().write(true).open(&file.tmp)?.sync_all()?;
            }
            std::fs::rename(&file.tmp, &file.path)
        })
        .collect()
}

/// Writes to disk everything written to the file system of `dir`.
#[cfg(target_os = "linux")]
fn sync_file_system(dir: &Path) -> io::Result<()> {
    rustix::fs::syncfs(File::open(dir)?)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn sync_file_system(_: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomically_creates_then_replaces_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("data.ron");

        write_atomically(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        write_atomically(&path, b"second, longer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second, longer");
        assert!(!tmp.path().join("data.ron.tmp").exists());
    }

    #[test]
    fn pending_files_replace_theirs_only_once_committed() {
        let tmp = tempfile::tempdir().unwrap();
        let (first, second) = (tmp.path().join("a.bin"), tmp.path().join("b.bin"));
        write_atomically(&first, b"old").unwrap();

        let pending = vec![
            write_pending(&first, b"new").unwrap(),
            write_pending(&second, b"added").unwrap(),
        ];
        assert_eq!(std::fs::read(&first).unwrap(), b"old");
        assert!(!second.exists());

        assert!(commit(tmp.path(), pending).iter().all(Result::is_ok));
        assert_eq!(std::fs::read(&first).unwrap(), b"new");
        assert_eq!(std::fs::read(&second).unwrap(), b"added");
        let names: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        assert_eq!(names.len(), 2, "a temporary file is left: {names:?}");
    }

    #[test]
    fn failed_write_leaves_the_previous_file_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("data.ron");
        write_atomically(&path, b"precious").unwrap();

        // A directory squatting on the temporary name makes the new write fail early.
        std::fs::create_dir(tmp.path().join("data.ron.tmp")).unwrap();
        assert!(write_atomically(&path, b"lost").is_err());

        assert_eq!(std::fs::read(&path).unwrap(), b"precious");
    }

    #[test]
    fn the_dropped_items_are_saved_and_read_back_and_default_to_none() {
        use shared::world::{DroppedItem, ItemId, ItemStack};
        let dirt = ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb: 3,
        };
        let item = DroppedItem::new(
            dirt,
            bevy::math::Vec3::new(1.0, 64.5, 2.0),
            Default::default(),
        );
        let meta = WorldMeta {
            dropped_items: vec![item.clone()],
            ..WorldMeta::new("w".into(), WorldSeed(1), 0, HashMap::new())
        };
        let read: WorldMeta = ron::de::from_str(&ron::ser::to_string(&meta).unwrap()).unwrap();
        assert_eq!(read.dropped_items, vec![item]);

        let before: WorldMeta =
            ron::de::from_str(r#"(format_version: 2, name: "w", seed: (1), time: 0, mobs: {})"#)
                .unwrap();
        assert!(before.dropped_items.is_empty());
    }

    #[test]
    fn a_mob_saved_with_the_former_field_name_is_still_read() {
        use shared::world::{MobAction, MobKind, MobTarget};
        let fox = ServerMob {
            kind: MobKind::Fox,
            target: MobTarget::None,
            action: MobAction::Walk,
            position: Default::default(),
            rotation: Default::default(),
            height: 1.0,
            width: 1.0,
            on_ground: true,
            velocity: Default::default(),
            depth: 1.5,
        };
        let meta = WorldMeta::new("w".into(), WorldSeed(1), 0, HashMap::from([(7, fox)]));
        let saved = ron::ser::to_string(&meta).unwrap();
        assert!(saved.contains("depth:1.5"), "{saved}");

        let former = saved.replace("depth:", "deepth:");
        let read: WorldMeta = ron::de::from_str(&former).unwrap();
        assert_eq!(read.mobs[&7].depth, 1.5);
    }
}
