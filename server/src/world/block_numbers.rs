//! The numbers a world saves its blocks and its items as (`docs/design/content-registry.md`,
//! 3.2): `world.ron` lists the name of each, by number, and the chunk files hold those numbers
//! of the blocks, the files of the chests and the furnaces those of the items and of the blocks
//! they place. What is read is turned to the numbers of the process, and what is written back to
//! those of the world: a kind of cell of a chunk's palette at a time, a stack at a time; nothing
//! is done to a chunk while the two are the same, as they are for the blocks of the engine. Each
//! list only grows: a block or an item of the process the world does not know yet takes the
//! next number when something holding it is saved.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{PoisonError, RwLock};

use bevy::log::{info, warn};
use shared::content::{content, install_changed, Content};
use shared::world::{
    BlockId, Chunk, Container, Furnace, ItemId, ItemStack, ItemType, LEGACY_BLOCK_NAMES,
    LEGACY_ITEM_NAMES,
};

use super::container_files::{ChunkContainers, ChunkContents};

/// What a world saves by number and lists by name: a block or an item.
pub trait Listed: Copy + Eq + Hash {
    /// What it is called in the log.
    const KIND: &'static str;
    /// The names of the worlds saved before they listed their own, by number.
    const LEGACY: &'static [&'static str];

    fn index(self) -> usize;
    fn from_index(index: u16) -> Self;
    /// The one named `name` in `content`.
    fn named(content: &Content, name: &str) -> Option<Self>;
    /// Its name in `content`, `None` for a number `content` does not define.
    fn name_in(self, content: &Content) -> Option<&str>;
    /// Gives each of `names` a placeholder in `content`.
    fn hold_places(content: &mut Content, names: Vec<String>);
    /// What stands for a number no name is known for: the debug block. `None` for an item,
    /// whose stack is then dropped.
    fn unknown() -> Option<Self>;
}

impl Listed for BlockId {
    const KIND: &'static str = "block";
    const LEGACY: &'static [&'static str] = &LEGACY_BLOCK_NAMES;

    fn index(self) -> usize {
        BlockId::index(self)
    }

    fn from_index(index: u16) -> Self {
        BlockId::from_index(index)
    }

    fn named(content: &Content, name: &str) -> Option<Self> {
        content.block_named(name)
    }

    fn name_in(self, content: &Content) -> Option<&str> {
        content.block_names.get(self.index()).map(String::as_str)
    }

    fn hold_places(content: &mut Content, names: Vec<String>) {
        content.hold_places(names);
    }

    fn unknown() -> Option<Self> {
        Some(BlockId::Debug)
    }
}

impl Listed for ItemId {
    const KIND: &'static str = "item";
    const LEGACY: &'static [&'static str] = &LEGACY_ITEM_NAMES;

    fn index(self) -> usize {
        ItemId::index(self)
    }

    fn from_index(index: u16) -> Self {
        ItemId::from_index(index)
    }

    fn named(content: &Content, name: &str) -> Option<Self> {
        content.item_named(name)
    }

    fn name_in(self, content: &Content) -> Option<&str> {
        content.item_names.get(self.index()).map(String::as_str)
    }

    fn hold_places(content: &mut Content, names: Vec<String>) {
        content.hold_item_places(names);
    }

    fn unknown() -> Option<Self> {
        None
    }
}

/// The numbers a world saves one kind of thing as, shared by the thread of the tick, which reads
/// its files, and the writer, which saves them.
#[derive(Debug)]
pub struct SavedNames<Id>(RwLock<Numbering<Id>>);

/// The numbers a world saves its blocks as.
pub type SavedBlocks = SavedNames<BlockId>;

/// The numbers a world saves its items as.
pub type SavedItems = SavedNames<ItemId>;

#[derive(Debug, Clone)]
struct Numbering<Id> {
    /// The name of each, by the number the world saves it as.
    names: Vec<String>,
    /// The one of the process each saved number stands for, `Listed::unknown` for a name the
    /// content lacks.
    to_process: Vec<Option<Id>>,
    /// The saved number of each one of the process the world knows.
    to_saved: HashMap<Id, u16>,
}

impl<Id: Listed> Default for SavedNames<Id> {
    /// The numbers of a world saved before it listed them, and of a new world.
    fn default() -> Self {
        Self::new(Id::LEGACY.iter().map(|name| name.to_string()).collect())
    }
}

impl<Id: Listed> SavedNames<Id> {
    /// The numbers of a world that lists `names`, or of one that lists none (`None`): a world
    /// saved before it listed them, or a new one. The names the content of the process lacks,
    /// of content that is gone, are given placeholders by an install of the content, so that
    /// the world saves them back under their names.
    pub fn open(names: Option<Vec<String>>) -> Self {
        let Some(names) = names else {
            return Self::default();
        };
        let missing: Vec<String> = names
            .iter()
            .filter(|name| Id::named(content(), name).is_none())
            .cloned()
            .collect();
        if !missing.is_empty() {
            warn!(
                "The content defines no {} named {}: they are kept as placeholders",
                Id::KIND,
                missing.join(", ")
            );
            install_changed(|held| Id::hold_places(held, missing));
        }
        Self::new(names)
    }

    /// The numbers of a world that lists `names`, each of which the content of the process
    /// defines, or not: a saved number of an unknown name stands for `Listed::unknown`. A name
    /// the content knows as an alias, a former name, is listed under the name it has now, which
    /// the next save writes, unless the world lists that one too.
    pub(super) fn new(mut names: Vec<String>) -> Self {
        let content = content();
        let to_process: Vec<Option<Id>> = names
            .iter()
            .map(|name| Id::named(content, name).or_else(Id::unknown))
            .collect();
        for saved in 0..names.len() {
            let Some(now) = Id::named(content, &names[saved]).and_then(|id| id.name_in(content))
            else {
                continue;
            };
            if now != names[saved] && !names.iter().any(|name| name == now) {
                info!(
                    "The {} {} of this world is now named {now}",
                    Id::KIND,
                    names[saved]
                );
                names[saved] = now.to_string();
            }
        }
        let mut to_saved = HashMap::new();
        for (saved, id) in to_process.iter().enumerate() {
            if let Some(id) = *id {
                if id.name_in(content) == Some(names[saved].as_str()) {
                    to_saved.entry(id).or_insert(saved as u16);
                }
            }
        }
        Self(RwLock::new(Numbering {
            names,
            to_process,
            to_saved,
        }))
    }

    /// The name of each, by the number the world saves it as: what `world.ron` lists.
    pub fn names(&self) -> Vec<String> {
        self.numbering().names.clone()
    }

    fn numbering(&self) -> std::sync::RwLockReadGuard<'_, Numbering<Id>> {
        self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// The one of the process the saved number of `saved` stands for: `Listed::unknown` for a
    /// number out of the list.
    fn to_process(numbering: &Numbering<Id>, saved: Id) -> Option<Id> {
        numbering
            .to_process
            .get(saved.index())
            .copied()
            .unwrap_or_else(Id::unknown)
    }

    /// The saved number of `id`, which `number_new` numbered: that of `Listed::unknown` for one
    /// the content does not define.
    fn to_saved(numbering: &Numbering<Id>, id: Id) -> Option<Id> {
        numbering
            .to_saved
            .get(&id)
            .or_else(|| numbering.to_saved.get(&Id::unknown()?))
            .map(|saved| Id::from_index(*saved))
    }

    /// Gives a number to each of `ids` the world does not know yet, before something holding
    /// them is written. One the content does not define is saved as `Listed::unknown`.
    pub fn number_new(&self, ids: impl IntoIterator<Item = Id>) {
        let new: Vec<Id> = {
            let numbering = self.numbering();
            let mut new = Vec::new();
            for id in ids {
                if !numbering.to_saved.contains_key(&id) && !new.contains(&id) {
                    new.push(id);
                }
            }
            new
        };
        if new.is_empty() {
            return;
        }
        let content = content();
        let mut numbering = self.0.write().unwrap_or_else(PoisonError::into_inner);
        for id in new {
            let named = match id.name_in(content) {
                Some(name) => Some((id, name)),
                None => Id::unknown().and_then(|id| Some((id, id.name_in(content)?))),
            };
            let Some((id, name)) = named else {
                continue;
            };
            if numbering.to_saved.contains_key(&id) {
                continue;
            }
            let saved = numbering.names.len() as u16;
            numbering.names.push(name.to_string());
            numbering.to_process.push(Some(id));
            numbering.to_saved.insert(id, saved);
        }
    }
}

impl SavedBlocks {
    /// The block of the process the saved block `saved` stands for: the debug block for a
    /// number out of the list.
    fn block_to_process(numbering: &Numbering<BlockId>, saved: BlockId) -> BlockId {
        Self::to_process(numbering, saved).unwrap_or(BlockId::Debug)
    }

    /// The saved number of `id`, which `number_new` numbered.
    fn block_to_saved(numbering: &Numbering<BlockId>, id: BlockId) -> BlockId {
        Self::to_saved(numbering, id).expect("numbered by number_new")
    }

    /// `chunk`, read from a file of the world, in the numbers of the process.
    pub fn read(&self, chunk: Chunk) -> Chunk {
        let numbering = self.numbering();
        let to_process = |id: BlockId| Self::block_to_process(&numbering, id);
        if !chunk.may_hold(|cell| cell.is_some_and(|block| to_process(block.id) != block.id)) {
            return chunk;
        }
        chunk.map_blocks(to_process)
    }

    /// The block of the process the block `saved`, read from a file of the world, stands for.
    pub fn read_block(&self, saved: BlockId) -> BlockId {
        Self::block_to_process(&self.numbering(), saved)
    }

    /// The number the world saves `id` as, numbered first if the world does not know it yet.
    pub fn write_block(&self, id: BlockId) -> BlockId {
        self.number_new([id]);
        Self::block_to_saved(&self.numbering(), id)
    }

    /// Gives a number to each block of `chunks` the world does not know yet, before they are
    /// written (`write`).
    pub fn number_new_blocks<'a>(&self, chunks: impl IntoIterator<Item = &'a Chunk>) {
        let mut held = Vec::new();
        for chunk in chunks {
            chunk.may_hold(|cell| {
                if let Some(block) = cell {
                    if !held.contains(&block.id) {
                        held.push(block.id);
                    }
                }
                false
            });
        }
        self.number_new(held);
    }

    /// `chunk`, to be written to a file of the world, in the numbers of the world: the blocks
    /// the world does not know yet are numbered first (`number_new_blocks`), which a save does
    /// for all its chunks before it writes the list. A number the content does not define is
    /// saved as the debug block.
    pub fn write(&self, chunk: &Chunk) -> Chunk {
        self.number_new_blocks([chunk]);
        let numbering = self.numbering();
        let to_saved = |id: BlockId| Self::block_to_saved(&numbering, id);
        if !chunk.may_hold(|cell| cell.is_some_and(|block| to_saved(block.id) != block.id)) {
            return chunk.clone();
        }
        chunk.map_blocks(to_saved)
    }
}

/// The numbers a world saves its blocks and its items as.
#[derive(Debug, Default)]
pub struct SavedNumbers {
    pub blocks: SavedBlocks,
    pub items: SavedItems,
}

impl SavedNumbers {
    /// The numbers of a world that lists `blocks` and `items`, each list `None` in a world
    /// saved before it had it (`SavedNames::open`).
    pub fn open(blocks: Option<Vec<String>>, items: Option<Vec<String>>) -> Self {
        Self {
            blocks: SavedBlocks::open(blocks),
            items: SavedItems::open(items),
        }
    }

    /// `stack` in the numbers `item` and `block` turn the numbers of its item and of the block
    /// it places into, or `None` when its item has no number there.
    fn map_stack(
        stack: ItemStack,
        item: impl Fn(ItemId) -> Option<ItemId>,
        block: impl Fn(BlockId) -> BlockId,
    ) -> Option<ItemStack> {
        let item_type = match stack.item_type {
            ItemType::Block(id) => ItemType::Block(block(id)),
            other => other,
        };
        Some(ItemStack {
            item_id: item(stack.item_id)?,
            item_type,
            ..stack
        })
    }

    /// What the blocks of a chunk hold, read from the files of the world, in the numbers of the
    /// process. A stack of an item the world never named is dropped.
    pub fn read_contents(&self, contents: ChunkContents) -> ChunkContents {
        let (items, blocks) = (self.items.numbering(), self.blocks.numbering());
        let map = |stack: ItemStack| {
            Self::map_stack(
                stack,
                |id| SavedItems::to_process(&items, id),
                |id| SavedBlocks::block_to_process(&blocks, id),
            )
        };
        map_contents(contents, map)
    }

    /// Gives a number to each item, and each block an item places, that `contents` holds and
    /// the world does not know yet, before they are written (`write_contents`).
    pub fn number_new_in<'a>(&self, contents: impl IntoIterator<Item = &'a ChunkContents>) {
        let mut items = Vec::new();
        let mut blocks = Vec::new();
        for contents in contents {
            for stack in stacks_of(contents) {
                items.push(stack.item_id);
                if let ItemType::Block(id) = stack.item_type {
                    blocks.push(id);
                }
            }
        }
        self.items.number_new(items);
        self.blocks.number_new(blocks);
    }

    /// What the blocks of a chunk hold, to be written to the files of the world, in the numbers
    /// of the world: what it does not know yet is numbered first (`number_new_in`), which a
    /// save does for all its chunks before it writes the lists.
    pub fn write_contents(&self, contents: &ChunkContents) -> ChunkContents {
        self.number_new_in([contents]);
        let (items, blocks) = (self.items.numbering(), self.blocks.numbering());
        let map = |stack: ItemStack| {
            Self::map_stack(
                stack,
                |id| SavedItems::to_saved(&items, id),
                |id| SavedBlocks::block_to_saved(&blocks, id),
            )
        };
        map_contents(contents.clone(), map)
    }
}

/// Every stack `contents` holds.
fn stacks_of(contents: &ChunkContents) -> impl Iterator<Item = &ItemStack> {
    let chests = contents
        .chests
        .iter()
        .flat_map(|(_, chest)| chest.slots.iter());
    let furnaces = contents
        .furnaces
        .iter()
        .flat_map(|(_, furnace)| [&furnace.input, &furnace.fuel, &furnace.output]);
    chests.chain(furnaces).flatten()
}

/// `contents` with each stack turned into what `map` makes of it.
fn map_contents(
    contents: ChunkContents,
    map: impl Fn(ItemStack) -> Option<ItemStack>,
) -> ChunkContents {
    let map_slot = |slot: &mut Option<ItemStack>| *slot = slot.and_then(&map);
    let mut chests: ChunkContainers<Container> = contents.chests;
    for (_, chest) in &mut chests {
        chest.slots.iter_mut().for_each(map_slot);
    }
    let mut furnaces: ChunkContainers<Furnace> = contents.furnaces;
    for (_, furnace) in &mut furnaces {
        [&mut furnace.input, &mut furnace.fuel, &mut furnace.output]
            .into_iter()
            .for_each(map_slot);
    }
    ChunkContents { chests, furnaces }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};
    use shared::world::{BlockData, BlockDirection};
    use shared::CHUNK_SIZE;

    /// A chunk of `kinds` blocks drawn from `blocks` at random places.
    fn random_chunk(rng: &mut StdRng, blocks: &[BlockId], kinds: usize) -> Chunk {
        let kinds: Vec<BlockId> = blocks.choose_multiple(rng, kinds).copied().collect();
        let mut chunk = Chunk::new();
        for _ in 0..rng.gen_range(1..2000) {
            let pos = bevy::math::IVec3::new(
                rng.gen_range(0..CHUNK_SIZE),
                rng.gen_range(0..CHUNK_SIZE),
                rng.gen_range(0..CHUNK_SIZE),
            );
            let id = *kinds.choose(rng).unwrap();
            chunk.set(pos, BlockData::new(id, BlockDirection::Back));
        }
        chunk
    }

    #[test]
    fn the_legacy_numbers_are_those_of_the_process() {
        let saved = SavedBlocks::default();
        let chunk = random_chunk(&mut StdRng::seed_from_u64(1), &BlockId::ENGINE, 40);
        assert_eq!(saved.write(&chunk), chunk);
        assert_eq!(saved.read(chunk.clone()), chunk);
        assert_eq!(saved.names(), LEGACY_BLOCK_NAMES);
    }

    /// Random chunks, of a few kinds of cells to more than a palette holds, written and read
    /// back through random lists of names.
    #[test]
    fn a_chunk_reads_back_as_it_was_written_whatever_the_numbers_of_the_world() {
        let mut rng = StdRng::seed_from_u64(7);
        for case in 0..40 {
            let mut names: Vec<String> = LEGACY_BLOCK_NAMES.iter().map(|n| n.to_string()).collect();
            names.shuffle(&mut rng);
            // a world that does not know some blocks yet
            names.truncate(rng.gen_range(100..=names.len()));
            let saved = SavedBlocks::new(names.clone());
            let kinds = [1, 3, 30, 300][case % 4];
            let chunk = random_chunk(&mut rng, &BlockId::ENGINE, kinds);

            saved.number_new_blocks([&chunk]);
            let written = saved.write(&chunk);
            let listed = saved.names();
            assert_eq!(listed[..names.len()], names, "the list only grows");
            for (pos, block) in written.iter() {
                let id = chunk.get(pos).unwrap().id;
                assert_eq!(listed[block.id.index()], id.name().unwrap(), "case {case}");
            }
            // read by the next process, from the list on disk
            assert_eq!(SavedBlocks::new(listed).read(written), chunk, "case {case}");
        }
    }

    #[test]
    fn a_saved_number_out_of_the_list_reads_as_the_debug_block() {
        let saved = SavedBlocks::new(vec!["stone".into(), "dirt".into()]);
        let mut chunk = Chunk::new();
        chunk.set(
            bevy::math::IVec3::ZERO,
            BlockData::new(BlockId::from_index(0), BlockDirection::Front),
        );
        chunk.set(
            bevy::math::IVec3::X,
            BlockData::new(BlockId::from_index(9), BlockDirection::Front),
        );
        let read = saved.read(chunk);
        assert_eq!(
            read.get(bevy::math::IVec3::ZERO).unwrap().id,
            BlockId::Stone
        );
        assert_eq!(read.get(bevy::math::IVec3::X).unwrap().id, BlockId::Debug);
    }

    /// What a chunk holds: a chest and a furnace of random stacks of `items`, some placing
    /// random blocks of `blocks`.
    fn random_contents(rng: &mut StdRng, items: &[ItemId], blocks: &[BlockId]) -> ChunkContents {
        let stack = |rng: &mut StdRng| {
            rng.gen_bool(0.7).then(|| ItemStack {
                item_id: *items.choose(rng).unwrap(),
                item_type: if rng.gen_bool(0.5) {
                    ItemType::Block(*blocks.choose(rng).unwrap())
                } else {
                    ItemType::Generic
                },
                nb: rng.gen_range(1..64),
            })
        };
        let mut chest = Container::default();
        for slot in &mut chest.slots {
            *slot = stack(rng);
        }
        let furnace = Furnace {
            input: stack(rng),
            fuel: stack(rng),
            output: stack(rng),
            ..Furnace::default()
        };
        ChunkContents {
            chests: vec![(bevy::math::IVec3::ZERO, chest)],
            furnaces: vec![(bevy::math::IVec3::X, furnace)],
        }
    }

    /// `names`, shuffled and cut short, as a world may list them.
    fn random_list(rng: &mut StdRng, names: &[&str], least: usize) -> Vec<String> {
        let mut names: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        names.shuffle(rng);
        names.truncate(rng.gen_range(least..=names.len()));
        names
    }

    #[test]
    fn the_legacy_numbers_of_the_items_are_those_of_the_process() {
        let saved = SavedNumbers::default();
        let contents = random_contents(
            &mut StdRng::seed_from_u64(2),
            &ItemId::ENGINE,
            &BlockId::ENGINE,
        );
        assert_eq!(saved.write_contents(&contents), contents);
        assert_eq!(saved.read_contents(contents.clone()), contents);
        assert_eq!(saved.items.names(), LEGACY_ITEM_NAMES);
    }

    /// Random chests and furnaces written and read back through random lists of names.
    #[test]
    fn what_a_chunk_holds_reads_back_as_it_was_written_whatever_the_numbers_of_the_world() {
        let mut rng = StdRng::seed_from_u64(11);
        for case in 0..40 {
            let (blocks, items) = (
                random_list(&mut rng, &LEGACY_BLOCK_NAMES, 100),
                random_list(&mut rng, &LEGACY_ITEM_NAMES, 100),
            );
            let saved = SavedNumbers {
                blocks: SavedBlocks::new(blocks.clone()),
                items: SavedItems::new(items.clone()),
            };
            let contents = random_contents(&mut rng, &ItemId::ENGINE, &BlockId::ENGINE);

            let written = saved.write_contents(&contents);
            let (listed_blocks, listed_items) = (saved.blocks.names(), saved.items.names());
            assert_eq!(listed_items[..items.len()], items, "the list only grows");
            assert_eq!(listed_blocks[..blocks.len()], blocks, "the list only grows");
            for (stack, written) in stacks_of(&contents).zip(stacks_of(&written)) {
                assert_eq!(
                    listed_items[written.item_id.index()],
                    stack.item_id.name(),
                    "case {case}"
                );
                if let (ItemType::Block(id), ItemType::Block(saved)) =
                    (stack.item_type, written.item_type)
                {
                    assert_eq!(listed_blocks[saved.index()], id.name().unwrap());
                }
            }
            // read by the next process, from the lists on disk
            let next = SavedNumbers {
                blocks: SavedBlocks::new(listed_blocks),
                items: SavedItems::new(listed_items),
            };
            assert_eq!(next.read_contents(written), contents, "case {case}");
        }
    }

    #[test]
    fn a_stack_of_an_item_out_of_the_list_is_dropped() {
        let saved = SavedNumbers {
            blocks: SavedBlocks::default(),
            items: SavedItems::new(vec!["stick".into()]),
        };
        let mut chest = Container::default();
        for (slot, item) in [ItemId::from_index(0), ItemId::from_index(7)]
            .into_iter()
            .enumerate()
        {
            chest.slots[slot] = Some(ItemStack {
                item_id: item,
                item_type: ItemType::Generic,
                nb: 3,
            });
        }
        let read = saved.read_contents(ChunkContents {
            chests: vec![(bevy::math::IVec3::ZERO, chest)],
            ..ChunkContents::default()
        });
        let slots = &read.chests[0].1.slots;
        assert_eq!(slots[0].map(|stack| stack.item_id), Some(ItemId::Stick));
        assert_eq!(slots[1], None);
    }

    #[test]
    fn a_block_and_an_item_listed_under_a_former_name_are_what_it_names_and_listed_anew() {
        // the oak slab was the wooden slab
        let blocks = SavedBlocks::new(vec!["dirt".to_string(), "wooden_slab".to_string()]);
        let saved = BlockId::from_index(1);
        assert_eq!(blocks.read_block(saved), BlockId::OakSlab);
        assert_eq!(blocks.names(), ["dirt", "oak_slab"]);
        assert_eq!(
            blocks.write_block(BlockId::OakSlab),
            saved,
            "no number is added"
        );
        let items = SavedItems::new(vec!["wooden_stairs".to_string()]);
        assert_eq!(items.names(), ["oak_stairs"]);
        // a world that lists both keeps both, each read as the slab
        let both = SavedBlocks::new(vec!["oak_slab".to_string(), "wooden_slab".to_string()]);
        assert_eq!(both.names(), ["oak_slab", "wooden_slab"]);
        assert_eq!(both.read_block(BlockId::from_index(1)), BlockId::OakSlab);
        assert_eq!(both.write_block(BlockId::OakSlab), BlockId::from_index(0));
    }
}
