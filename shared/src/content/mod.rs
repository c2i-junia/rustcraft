//! The properties of the blocks and the items, held in one table that every reader shares: the
//! getters of `BlockId` and `ItemId` read it, on any thread, with nothing passed to them.
//!
//! The table of the process is a snapshot that is never changed in place: `install` puts a new
//! one in its stead, and the old one is leaked, so that a reference taken from `content()` stays
//! valid for the whole process. A reader that reads several properties in a loop takes
//! `content()` once, so that it never reads some from one install and some from the next.

mod blocks;
#[cfg(test)]
mod dump;
mod files;
mod items;

pub(crate) use blocks::tool_bit;
pub use blocks::BlockProps;
pub use files::ContentError;
pub use items::ItemProps;

use serde::{Deserialize, Serialize};
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::world::{BlockId, ItemId};

/// The properties of every block and every item, by their number. The server sends it whole to
/// its clients (`ServerToClientMessage::Content`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Content {
    /// Which install of the process this is: 0 for the built-in content the process starts
    /// with, one more at each `install`. What the caches built from the content watch.
    pub generation: u32,
    /// By `BlockId`, in the order of `BlockId::ALL`.
    pub blocks: Vec<BlockProps>,
    /// By `ItemId`, in the order of `ItemId::ALL`.
    pub items: Vec<ItemProps>,
}

impl Content {
    /// The content written in the code: what the `builtin_*` functions of `BlockId` and
    /// `ItemId` say of each block and item.
    pub fn builtin() -> Self {
        Self {
            generation: 0,
            blocks: BlockId::ALL.into_iter().map(BlockProps::builtin).collect(),
            items: ItemId::ALL.into_iter().map(ItemProps::builtin).collect(),
        }
    }

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

/// The content of the process: null until the first read installs the built-in content, then
/// always a pointer leaked by `Box::leak` or `Box::into_raw`, never freed.
static CONTENT: AtomicPtr<Content> = AtomicPtr::new(ptr::null_mut());

/// Held by `install`, so that two installs at once still count their generations one by one.
static INSTALLING: Mutex<()> = Mutex::new(());

/// The content of the process: the built-in content until an `install`. On x86-64 a read is a
/// plain load of the pointer, then of the entry.
#[inline]
pub fn content() -> &'static Content {
    let current = CONTENT.load(Ordering::Acquire);
    if current.is_null() {
        return install_builtin();
    }
    // SAFETY: `CONTENT` only ever holds null or a leaked pointer, which nothing frees.
    unsafe { &*current }
}

/// Installs the built-in content, on the first read of the process, unless another thread did
/// so in the meantime.
#[cold]
#[inline(never)]
fn install_builtin() -> &'static Content {
    let builtin = Box::into_raw(Box::new(Content::builtin()));
    match CONTENT.compare_exchange(
        ptr::null_mut(),
        builtin,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        // SAFETY: `builtin` comes from `Box::into_raw`, and is now never freed.
        Ok(_) => unsafe { &*builtin },
        Err(current) => {
            // SAFETY: `builtin` was never shared: it is freed once, here.
            drop(unsafe { Box::from_raw(builtin) });
            // SAFETY: as in `content`.
            unsafe { &*current }
        }
    }
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
    fn the_builtin_content_has_one_entry_per_block_and_per_item_in_their_order() {
        let builtin = Content::builtin();
        assert_eq!(builtin.generation, 0);
        assert_eq!(builtin.blocks.len(), BlockId::ALL.len());
        assert_eq!(builtin.items.len(), ItemId::ALL.len());
        for id in BlockId::ALL {
            assert_eq!(builtin.block(id), &BlockProps::builtin(id), "{id:?}");
        }
        for id in ItemId::ALL {
            assert_eq!(builtin.item(id), &ItemProps::builtin(id), "{id:?}");
        }
    }

    /// The only test that installs: it installs content equal to the built-in one, so that the
    /// tests running beside it read the same properties before and after.
    #[test]
    fn an_install_is_the_next_generation_and_what_the_getters_read() {
        let before = content();
        let installed = install(Content::builtin());
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
        let mut short = Content::builtin();
        short.blocks.pop();
        install(short);
    }
}
