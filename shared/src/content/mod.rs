//! The properties of the blocks and the items, held in one table that every reader shares: the
//! getters of `BlockId` and `ItemId` read it, on any thread, with nothing passed to them. The
//! table is read from the content files (`files`), `data/content/rustcraft/*.ron`.
//!
//! The table of the process is a snapshot that is never changed in place: `install` puts a new
//! one in its stead, and the old one is leaked, so that a reference taken from `content()` stays
//! valid for the whole process. A reader that reads several properties in a loop takes
//! `content()` once, so that it never reads some from one install and some from the next.

mod blocks;
mod families;
mod files;
mod items;
mod look;
mod recipes;

pub(crate) use blocks::tool_bit;
pub use blocks::BlockProps;
pub use families::Replaced;
pub use files::{ContentError, NewName};
pub use items::{DisplayName, ItemProps};
pub use look::{Behaviour, BlockFlags, Look, Shape, Sway, Tint};
pub use recipes::{Crafting, Recipe, Tag};

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
    /// The name of each item, by `ItemId::index`, as the files and the commands name it:
    /// `oak_log`. What a world saves its items as.
    pub item_names: Vec<String>,
    /// The Rust name of each item, by `ItemId::index`, which names its texture, its key in the
    /// language files and which RON writes it as: `OakLog`.
    pub item_idents: Vec<String>,
    /// By `ItemId::index`: the items of `ItemId::ENGINE` first, in their order.
    pub items: Vec<ItemProps>,
    /// By `ItemId::index`, the name each item the files name is shown under: its own, or that
    /// of its block (`ItemId::display_name`).
    pub item_display: std::collections::BTreeMap<u16, DisplayName>,
    /// The blocks by the other names the files give them (`aliases`), their former ones.
    pub block_aliases: std::collections::BTreeMap<String, BlockId>,
    /// The items by the other names the files give them, and the block of their name gives them.
    pub item_aliases: std::collections::BTreeMap<String, ItemId>,
    /// The recipes, what a furnace smelts and burns, and the families of items they take.
    pub crafting: Crafting,
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

    /// The block named `name`, as the files name it, or by one of its aliases.
    pub fn block_named(&self, name: &str) -> Option<BlockId> {
        self.block_names
            .iter()
            .position(|known| known == name)
            .map(|index| BlockId::from_index(index as u16))
            .or_else(|| self.block_aliases.get(name).copied())
    }

    /// What makes this content unfit to install, `None` when it is whole: a block or an item of
    /// the engine it lacks, a block or an item without its name or Rust name.
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
        if self.items.len() < ItemId::ENGINE.len() {
            return Some("not one entry per item of the engine".to_string());
        }
        (self.item_names.len() != self.items.len() || self.item_idents.len() != self.items.len())
            .then(|| {
                format!(
                    "not one name per item: {} items, {} names, {} Rust names",
                    self.items.len(),
                    self.item_names.len(),
                    self.item_idents.len()
                )
            })
    }

    /// Why a game cannot play with this content, a server's: it is not whole (`flaw`), or the
    /// blocks and the items its engine names (`BlockId::ENGINE`, `ItemId::ENGINE`) are not the
    /// first of the content, by the same names. `None` when it can, whatever the content holds
    /// after them.
    pub fn engine_mismatch(&self) -> Option<String> {
        if let Some(flaw) = self.flaw() {
            return Some(format!("The content of the server is broken: {flaw}"));
        }
        // whole: it names every block and every item of the engine
        let first_other = |kind: &str, ours: Vec<String>, theirs: &[String]| {
            let (number, (ours, theirs)) = ours
                .iter()
                .zip(theirs)
                .enumerate()
                .find(|(_, (ours, theirs))| ours != theirs)?;
            Some(format!(
                "The server and this game do not have the same {kind}s: {kind} {number} is \
                 \"{theirs}\" on the server and \"{ours}\" in this game"
            ))
        };
        let blocks = BlockId::ENGINE
            .into_iter()
            .map(files::engine_name)
            .collect();
        let items = ItemId::ENGINE
            .into_iter()
            .map(files::engine_item_name)
            .collect();
        first_other("block", blocks, &self.block_names)
            .or_else(|| first_other("item", items, &self.item_names))
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

    /// Gives each of `names` the content does not define an item of its own, after the others:
    /// a placeholder (`ItemProps::PLACEHOLDER`), as for an item a world holds that the content
    /// no longer has.
    pub fn hold_item_places(&mut self, names: impl IntoIterator<Item = String>) {
        for name in names {
            if self.item_named(&name).is_none() {
                self.item_idents.push(crate::utils::pascal_case(&name));
                self.item_names.push(name);
                self.items.push(ItemProps::PLACEHOLDER);
            }
        }
    }

    /// The properties of the item `id`, those of a placeholder for a number the content does
    /// not define, as one a server of other content may send.
    #[inline]
    pub fn item(&self, id: ItemId) -> &ItemProps {
        self.items
            .get(id.index())
            .unwrap_or(&ItemProps::PLACEHOLDER)
    }

    /// The item named `name`, as the files name it, or by one of its aliases.
    pub fn item_named(&self, name: &str) -> Option<ItemId> {
        self.item_names
            .iter()
            .position(|known| known == name)
            .map(|index| ItemId::from_index(index as u16))
            .or_else(|| self.item_aliases.get(name).copied())
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
    let (loaded, new, replaced) = Content::load_telling(&folder, started.then(content))?;
    tell_new_names(&new);
    tell_replaced(&replaced);
    if !started {
        start_with(loaded);
    } else if !loaded.same_properties(content()) {
        install(loaded);
    }
    Ok(folder)
}

/// Logs each of `new`: a warning for one that may be a typo.
pub fn tell_new_names(new: &[NewName]) {
    for block in new {
        if block.near.is_some() {
            bevy_log::warn!("{block}");
        } else {
            bevy_log::info!("{block}");
        }
    }
}

/// Logs each of `replaced`, as the family's definition a file's own replaces is silent.
pub fn tell_replaced(replaced: &[Replaced]) {
    for member in replaced {
        bevy_log::info!("{member}");
    }
}

/// Makes `new` the content of the process, as the generation after the current one, and
/// returns it. The previous content is leaked: the references to it stay valid.
///
/// # Panics
///
/// If `new` is not whole (`Content::flaw`), which every read would trip on.
pub fn install(new: Content) -> &'static Content {
    if let Some(flaw) = new.flaw() {
        panic!("{flaw}");
    }
    let _installing = INSTALLING.lock().unwrap_or_else(PoisonError::into_inner);
    store(new)
}

/// Makes `new` the content of the process, as `install` does, under its lock.
fn store(mut new: Content) -> &'static Content {
    new.generation = content().generation + 1;
    let new = Box::leak(Box::new(new));
    CONTENT.store(new, Ordering::Release);
    new
}

thread_local! {
    /// Whether a name the content lacks, read on this thread, is given a placeholder
    /// (`HoldingPlaces`).
    static HOLDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While it lives, a block or an item that a file read on this thread names but the content
/// lacks is given a placeholder (`hold_block_place`): what the server reads the files of a world
/// under, `world.ron` and the files of its players, which may hold content that is gone.
/// Elsewhere such a name is an error, as in a file of the client.
pub struct HoldingPlaces(bool);

impl HoldingPlaces {
    pub fn begin() -> Self {
        Self(HOLDING.replace(true))
    }
}

impl Drop for HoldingPlaces {
    fn drop(&mut self) {
        HOLDING.set(self.0);
    }
}

/// The block a file reads by the Rust name `ident` that the content of the process does not
/// define, as a world or a player file holds one of content that is gone, while this thread is
/// `HoldingPlaces`: a placeholder is installed in its place (`Content::hold_places`), so that
/// the file reads and the block is saved back under its name. `None` when `ident` cannot name a
/// block, or when the thread holds no places.
pub fn hold_block_place(ident: &str) -> Option<BlockId> {
    hold_place("block", ident, Content::block_named, |content, name| {
        content.hold_places([name]);
    })
}

/// The item a file reads by the Rust name `ident` that the content of the process does not
/// define, given a placeholder as `hold_block_place` gives a block one.
pub fn hold_item_place(ident: &str) -> Option<ItemId> {
    hold_place("item", ident, Content::item_named, |content, name| {
        content.hold_item_places([name]);
    })
}

/// `hold_block_place` or `hold_item_place`: the `kind` of `ident` that `find` finds in the
/// content, once `hold` gave it a place if it had none.
fn hold_place<Id>(
    kind: &str,
    ident: &str,
    find: impl Fn(&Content, &str) -> Option<Id>,
    hold: impl Fn(&mut Content, String),
) -> Option<Id> {
    let name = crate::utils::snake_case(ident);
    if !HOLDING.get() || !files::is_content_name(&name) || crate::utils::pascal_case(&name) != ident
    {
        return None;
    }
    let _installing = INSTALLING.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(id) = find(content(), &name) {
        return Some(id);
    }
    bevy_log::warn!("The content defines no {kind} named {name}: it is kept as a placeholder");
    let mut held = content().clone();
    hold(&mut held, name.clone());
    find(store(held), &name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_content_has_one_entry_per_block_and_per_item() {
        let loaded = Content::load(&source_folder()).unwrap();
        assert_eq!(loaded.generation, 0);
        assert_eq!(loaded.blocks.len(), BlockId::ENGINE.len());
        assert_eq!(loaded.items.len(), ItemId::ENGINE.len());
        assert_eq!(loaded.item_names.len(), ItemId::ENGINE.len());
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

        let mut renamed = content().clone();
        renamed.item_names[18] = "twig".to_string();
        let reason = renamed.engine_mismatch().unwrap();
        assert!(
            reason.contains("item 18 is \"twig\" on the server and \"stick\" in this game"),
            "{reason}"
        );
        let mut broken = content().clone();
        broken.item_idents.pop();
        let reason = broken.engine_mismatch().unwrap();
        assert!(reason.contains("not one name per item"), "{reason}");
    }

    /// What the files of a world read by a Rust name: a known one installs nothing, and a word
    /// no block or item could be named is not given a place. A placeholder installed is the
    /// test `server/tests/removed_pack.rs`, in a process of its own.
    #[test]
    fn a_name_read_from_a_file_is_given_a_place_only_if_it_could_name_one() {
        let before = content();
        assert_eq!(
            hold_item_place("NotAnItem"),
            None,
            "not while reading a world"
        );
        let _holding = HoldingPlaces::begin();
        assert_eq!(hold_item_place("Stick"), Some(ItemId::Stick));
        assert_eq!(hold_block_place("Stone"), Some(BlockId::Stone));
        for ident in ["glow_moss", "Glow_Moss", "", "glowMoss"] {
            assert_eq!(hold_item_place(ident), None, "{ident}");
            assert_eq!(hold_block_place(ident), None, "{ident}");
        }
        assert!(std::ptr::eq(content(), before), "nothing was installed");
    }

    #[test]
    fn an_item_a_world_holds_that_the_content_lacks_holds_its_place() {
        let mut held = content().clone();
        held.hold_item_places(["ruby".to_string(), "stick".to_string()]);
        let ruby = ItemId::from_index(content().items.len() as u16);
        assert_eq!(
            held.items.len(),
            content().items.len() + 1,
            "the stick is known"
        );
        assert_eq!(held.item_named("ruby"), Some(ruby));
        assert_eq!(held.item_idents[ruby.index()], "Ruby");
        assert_eq!(*held.item(ruby), ItemProps::PLACEHOLDER);
        // marked so, which no item of the files is (the item browser leaves it out)
        assert!(held.item(ruby).placeholder);
        assert!(held
            .items
            .iter()
            .take(content().items.len())
            .all(|item| !item.placeholder));
        assert_eq!(held.flaw(), None);
        // a number no content defines reads as a placeholder
        assert_eq!(
            *content().item(ItemId::from_index(u16::MAX)),
            ItemProps::PLACEHOLDER
        );
    }

    #[test]
    #[should_panic(expected = "not one entry per block of the engine")]
    fn an_install_of_a_table_of_the_wrong_size_fails() {
        let mut short = content().clone();
        short.blocks.truncate(BlockId::ENGINE.len() - 1);
        install(short);
    }
}
