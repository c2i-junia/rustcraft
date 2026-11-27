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
pub use files::ContentError;
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
    /// By `BlockId`, in the order of `BlockId::ALL`.
    pub blocks: Vec<BlockProps>,
    /// By `ItemId`, in the order of `ItemId::ALL`.
    pub items: Vec<ItemProps>,
}

impl Content {
    /// The properties of the block `id`.
    #[inline]
    pub fn block(&self, id: BlockId) -> &BlockProps {
        &self.blocks[id as usize]
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

/// Starts the process on the content files of `folder`, what a game does before anything reads
/// its content: those of the source tree if `folder` does not exist, as under `cargo run`
/// without the assets of a release. If the process already has content, the content of the
/// files is installed over it, unless it is the same.
pub fn start(folder: &Path) -> Result<&'static Content, ContentError> {
    let source = source_folder();
    let folder = match folder.is_dir() {
        true => folder,
        false if source.is_dir() => &source,
        false => {
            return Err(ContentError {
                file: folder.to_path_buf(),
                reason: "no such folder of content files".to_string(),
            })
        }
    };
    let loaded = Content::load(folder)?;
    if CONTENT.load(Ordering::Acquire).is_null() {
        return Ok(start_with(loaded));
    }
    Ok(match loaded.same_properties(content()) {
        true => content(),
        false => install(loaded),
    })
}

/// Makes `new` the content of the process, as the generation after the current one, and
/// returns it. The previous content is leaked: the references to it stay valid.
///
/// # Panics
///
/// If `new` does not have one entry per block and per item, which every read would trip on.
pub fn install(mut new: Content) -> &'static Content {
    // Lot 4 of `docs/design/content-registry.md` numbers the blocks at load: the table then has
    // as many entries as the content defines, and this check goes
    assert_eq!(new.blocks.len(), BlockId::ALL.len(), "one entry per block");
    assert_eq!(new.items.len(), ItemId::ALL.len(), "one entry per item");
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
        assert_eq!(loaded.blocks.len(), BlockId::ALL.len());
        assert_eq!(loaded.items.len(), ItemId::ALL.len());
    }

    #[test]
    fn the_process_starts_on_the_content_files_of_the_source_tree() {
        assert!(content().same_properties(&Content::load(&source_folder()).unwrap()));
        // started already: the same files change nothing
        let started = content();
        assert!(start(&source_folder()).unwrap().same_properties(started));
        assert!(
            start(Path::new("/no/such/folder")).is_ok(),
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
    #[should_panic(expected = "one entry per block")]
    fn an_install_of_a_table_of_the_wrong_size_fails() {
        let mut short = content().clone();
        short.blocks.pop();
        install(short);
    }
}
