//! The properties of the blocks and the items, held in one table that every reader shares: the
//! getters of `BlockId` and `ItemId` read it, on any thread, with nothing passed to them. The
//! table is read from the content files (`files`), `data/content/rustcraft/*.ron`.
//!
//! The table of the process is a snapshot that is never changed in place: `install` puts a new
//! one in its stead, and the old one is leaked, so that a reference taken from `content()` stays
//! valid for the whole process. A reader that reads several properties in a loop takes
//! `content()` once, so that it never reads some from one install and some from the next.

mod blocks;
mod files;
mod items;
mod look;

pub(crate) use blocks::tool_bit;
pub use blocks::BlockProps;
pub use files::{ContentError, NewBlock};
pub use items::ItemProps;
pub use look::{Behaviour, BlockFlags, Look, Shape, Sway, Tint};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::world::{BlockId, ItemId};

/// The properties of every block and every item, by their number. The server sends it whole to
/// its clients (`ServerToClientMessage::Content`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Content {
    /// Which install of the process this is: 0 for the content the process starts with, one
    /// more at each `install`. What the caches built from the content watch.
    pub generation: u32,
    /// The name of each block, by `BlockId::index`, as the files and the commands name it:
    /// `oak_log`. What a world saves its blocks as (`docs/design/content-registry.md`, 3).
    pub block_names: Vec<String>,
    /// The Rust name of each block, by `BlockId::index`, which names its textures and which RON
    /// writes it as: `OakLog`.
    pub block_idents: Vec<String>,
    /// By `BlockId::index`: the blocks of `BlockId::ENGINE` first, in their order.
    pub blocks: Vec<BlockProps>,
    /// By `ItemId`, in the order of `ItemId::ALL`.
    pub items: Vec<ItemProps>,
}

impl Content {
    /// The properties of the block `id`, those of `BlockId::Debug` for a number the content
    /// does not define, as one a server of other content may send.
    #[inline]
    pub fn block(&self, id: BlockId) -> &BlockProps {
        self.blocks
            .get(id.index())
            .unwrap_or(&self.blocks[BlockId::Debug.index()])
    }

    /// The block named `name`, as the files name it.
    pub fn block_named(&self, name: &str) -> Option<BlockId> {
        self.block_names
            .iter()
            .position(|known| known == name)
            .map(|index| BlockId::from_index(index as u16))
    }

    /// What makes this content unfit to install, `None` when it is whole: a block of the
    /// engine it lacks, a block without its name or Rust name, an item missing or too many.
    /// What a client checks of the content a server sends (`engine_mismatch`).
    pub fn flaw(&self) -> Option<String> {
        if self.blocks.len() < BlockId::ENGINE.len() {
            return Some("not one entry per block of the engine".to_string());
        }
        if self.block_names.len() != self.blocks.len()
            || self.block_idents.len() != self.blocks.len()
        {
            return Some(format!(
                "not one name per block: {} blocks, {} names, {} Rust names",
                self.blocks.len(),
                self.block_names.len(),
                self.block_idents.len()
            ));
        }
        (self.items.len() != ItemId::ALL.len()).then(|| "not one entry per item".to_string())
    }

    /// Why a game cannot play with this content, a server's: it is not whole (`flaw`), or the
    /// blocks its engine names (`BlockId::ENGINE`) are not the first of the content, by the
    /// same names. `None` when it can, whatever the content holds after them.
    pub fn engine_mismatch(&self) -> Option<String> {
        if let Some(flaw) = self.flaw() {
            return Some(format!("The content of the server is broken: {flaw}"));
        }
        // whole: it names every block of the engine
        let ours = BlockId::ENGINE.into_iter().map(files::engine_name);
        let (number, theirs, ours) = ours
            .zip(&self.block_names)
            .enumerate()
            .find(|(_, (ours, theirs))| ours != *theirs)
            .map(|(number, (ours, theirs))| (number, theirs, ours))?;
        Some(format!(
            "The server and this game do not have the same blocks: block {number} is \
             \"{theirs}\" on the server and \"{ours}\" in this game"
        ))
    }

    /// Gives each of `names` the content does not define a block of its own, after the others:
    /// a placeholder (`BlockProps::placeholder`), as for a block a world holds that the
    /// content no longer has.
    pub fn hold_places(&mut self, names: impl IntoIterator<Item = String>) {
        let placeholder = BlockProps::placeholder(self.block(BlockId::Debug));
        for name in names {
            if self.block_named(&name).is_none() {
                self.block_idents.push(crate::utils::pascal_case(&name));
                self.block_names.push(name);
                self.blocks.push(placeholder.clone());
            }
        }
    }

    /// The properties of the item `id`.
    #[inline]
    pub fn item(&self, id: ItemId) -> &ItemProps {
        &self.items[id as usize]
    }
}

/// The content of the process: null until it starts (`start`, or the first read), then always a
/// pointer leaked by `Box::leak` or `Box::into_raw`, never freed.
static CONTENT: AtomicPtr<Content> = AtomicPtr::new(ptr::null_mut());

/// Held by `install`, so that two installs at once still count their generations one by one.
static INSTALLING: Mutex<()> = Mutex::new(());

/// The folder of the content files of the base game in the assets folder `assets`.
pub fn folder(assets: &Path) -> PathBuf {
    assets.join("content").join("rustcraft")
}

/// The content files of the source tree: those a process reads that was not started on others,
/// such as the tests.
fn source_folder() -> PathBuf {
    folder(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../data"))
}

/// The content of the process: the content it started with until an `install`. On x86-64 a
/// read is a plain load of the pointer, then of the entry.
#[inline]
pub fn content() -> &'static Content {
    let current = CONTENT.load(Ordering::Acquire);
    if current.is_null() {
        return start_from_source();
    }
    // SAFETY: `CONTENT` only ever holds null or a leaked pointer, which nothing frees.
    unsafe { &*current }
}

/// Starts the process on the content files of the source tree, on its first read, as a process
/// that `start` was not called in.
///
/// # Panics
///
/// If they cannot be read: nothing can go on without content.
#[cold]
#[inline(never)]
fn start_from_source() -> &'static Content {
    let folder = source_folder();
    let loaded = Content::load(&folder).unwrap_or_else(|err| {
        panic!("the game has no content: {err} (start it with --assets-folder-path)")
    });
    start_with(loaded)
}

/// Makes `start` the content the process starts with, unless another thread started it in the
/// meantime. Returns the content of the process.
fn start_with(start: Content) -> &'static Content {
    let start = Box::into_raw(Box::new(start));
    match CONTENT.compare_exchange(ptr::null_mut(), start, Ordering::AcqRel, Ordering::Acquire) {
        // SAFETY: `start` comes from `Box::into_raw`, and is now never freed.
        Ok(_) => unsafe { &*start },
        Err(current) => {
            // SAFETY: `start` was never shared: it is freed once, here.
            drop(unsafe { Box::from_raw(start) });
            // SAFETY: as in `content`.
            unsafe { &*current }
        }
    }
}

/// The folder of content files a game whose assets hold `folder` reads: `folder`, or the
/// source tree's if it does not exist, as under `cargo run` without the assets of a release.
/// `None` if neither exists.
pub fn files_folder(folder: &Path) -> Option<PathBuf> {
    [folder.to_path_buf(), source_folder()]
        .into_iter()
        .find(|folder| folder.is_dir())
}

/// Starts the process on the content files of `files_folder(folder)`, what a game does before
/// anything reads its content, and returns that folder. If the process already has content,
/// the content of the files is installed over it, numbered as it, unless it is the same.
pub fn start(folder: &Path) -> Result<PathBuf, ContentError> {
    let folder = files_folder(folder).ok_or_else(|| ContentError {
        file: folder.to_path_buf(),
        reason: "no such folder of content files".to_string(),
    })?;
    let started = !CONTENT.load(Ordering::Acquire).is_null();
    let (loaded, new) = Content::load_telling(&folder, started.then(content))?;
    tell_new_blocks(&new);
    if !started {
        start_with(loaded);
    } else if !loaded.same_properties(content()) {
        install(loaded);
    }
    Ok(folder)
}

/// Logs each of `new`: a warning for one that may be a typo.
pub fn tell_new_blocks(new: &[NewBlock]) {
    for block in new {
        if block.near.is_some() {
            bevy_log::warn!("{block}");
        } else {
            bevy_log::info!("{block}");
        }
    }
}

/// Makes `new` the content of the process, as the generation after the current one, and
/// returns it. The previous content is leaked: the references to it stay valid.
///
/// # Panics
///
/// If `new` is not whole (`Content::flaw`), which every read would trip on.
pub fn install(mut new: Content) -> &'static Content {
    if let Some(flaw) = new.flaw() {
        panic!("{flaw}");
    }
    let _installing = INSTALLING.lock().unwrap_or_else(PoisonError::into_inner);
    new.generation = content().generation + 1;
    let new = Box::leak(Box::new(new));
    CONTENT.store(new, Ordering::Release);
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_content_has_one_entry_per_block_and_per_item() {
        let loaded = Content::load(&source_folder()).unwrap();
        assert_eq!(loaded.generation, 0);
        assert_eq!(loaded.blocks.len(), BlockId::ENGINE.len());
        assert_eq!(loaded.items.len(), ItemId::ALL.len());
    }

    #[test]
    fn the_process_starts_on_the_content_files_of_the_source_tree() {
        assert!(content().same_properties(&Content::load(&source_folder()).unwrap()));
        // started already: the same files change nothing
        let started = content();
        assert_eq!(start(&source_folder()), Ok(source_folder()));
        assert!(content().same_properties(started));
        assert_eq!(
            start(Path::new("/no/such/folder")),
            Ok(source_folder()),
            "the source tree's instead"
        );
    }

    /// The only test that installs: it installs content equal to that of the process, so that
    /// the tests running beside it read the same properties before and after.
    #[test]
    fn an_install_is_the_next_generation_and_what_the_getters_read() {
        let before = content();
        let installed = install(content().clone());
        assert!(installed.generation > before.generation);
        assert!(content().generation >= installed.generation);
        assert_eq!(content().blocks, before.blocks);
        assert_eq!(content().items, before.items);
        // the old snapshot is still readable
        assert_eq!(before.block(BlockId::Torch).light_emission, 14);
    }

    #[test]
    #[should_panic(expected = "not one name per block")]
    fn an_install_of_a_table_without_a_name_per_block_fails() {
        let mut nameless = content().clone();
        nameless.block_idents.pop();
        install(nameless);
    }

    #[test]
    fn a_game_plays_with_content_whose_engine_blocks_are_its_own() {
        let mut server = content().clone();
        server.hold_places(["glow_moss".to_string()]);
        assert_eq!(server.engine_mismatch(), None, "blocks after the engine's");

        let mut renamed = content().clone();
        renamed.block_names[3] = "granite".to_string();
        let reason = renamed.engine_mismatch().unwrap();
        assert!(
            reason.contains("block 3 is \"granite\" on the server and \"stone\" in this game"),
            "{reason}"
        );

        let mut older = content().clone();
        older.block_names.truncate(10);
        older.block_idents.truncate(10);
        older.blocks.truncate(10);
        let reason = older.engine_mismatch().unwrap();
        assert!(
            reason.contains("not one entry per block of the engine"),
            "{reason}"
        );

        let mut broken = content().clone();
        broken.block_names.push("extra".to_string());
        let reason = broken.engine_mismatch().unwrap();
        assert!(reason.contains("not one name per block"), "{reason}");
    }

    #[test]
    #[should_panic(expected = "not one entry per block of the engine")]
    fn an_install_of_a_table_of_the_wrong_size_fails() {
        let mut short = content().clone();
        short.blocks.truncate(BlockId::ENGINE.len() - 1);
        install(short);
    }
}
