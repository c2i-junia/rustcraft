//! The numbers a world saves its blocks as (`docs/design/content-registry.md`, 3.2): `world.ron`
//! lists the name of each, by number, and the chunk files hold those numbers. A chunk read is
//! turned to the numbers of the process, and one written back to those of the world, a kind of
//! cell of its palette at a time; nothing is done while the two are the same, as they are for
//! the blocks of the engine. The list only grows: a block of the process the world does not
//! know yet takes the next number when a chunk holding it is saved.

use std::collections::HashMap;
use std::sync::{PoisonError, RwLock};

use bevy::log::warn;
use shared::content::{content, install};
use shared::world::{BlockId, Chunk, LEGACY_BLOCK_NAMES};

/// The numbers a world saves its blocks as, shared by the thread of the tick, which reads its
/// chunks, and the writer, which saves them.
#[derive(Debug)]
pub struct SavedBlocks(RwLock<Numbering>);

#[derive(Debug, Clone)]
struct Numbering {
    /// The name of each block, by the number the world saves it as.
    names: Vec<String>,
    /// The block of the process each saved number stands for.
    to_process: Vec<BlockId>,
    /// The saved number of each block of the process the world knows.
    to_saved: HashMap<BlockId, u16>,
}

impl Default for SavedBlocks {
    /// The numbers of a world saved before it listed its blocks, and of a new world.
    fn default() -> Self {
        Self::new(
            LEGACY_BLOCK_NAMES
                .iter()
                .map(|name| name.to_string())
                .collect(),
        )
    }
}

impl SavedBlocks {
    /// The numbers of a world that lists `names`, or of one that lists none (`None`): a world
    /// saved before it listed its blocks, or a new one. The names the content of the process
    /// lacks, blocks of content that is gone, are given placeholders (`BlockProps::placeholder`)
    /// by an install of the content, so that the world saves them back under their names.
    pub fn open(names: Option<Vec<String>>) -> Self {
        let Some(names) = names else {
            return Self::default();
        };
        let missing: Vec<&String> = names
            .iter()
            .filter(|name| content().block_named(name).is_none())
            .collect();
        if !missing.is_empty() {
            warn!(
                "The content defines no block named {}: they are kept as placeholders",
                missing
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let mut held = content().clone();
            held.hold_places(missing.into_iter().cloned());
            install(held);
        }
        Self::new(names)
    }

    /// The numbers of a world that lists `names`, each of which the content of the process
    /// defines, or not: a saved number of an unknown name stands for `BlockId::Debug`.
    fn new(names: Vec<String>) -> Self {
        let content = content();
        let to_process: Vec<BlockId> = names
            .iter()
            .map(|name| content.block_named(name).unwrap_or(BlockId::Debug))
            .collect();
        let mut to_saved = HashMap::new();
        for (saved, &id) in to_process.iter().enumerate() {
            if content.block_names.get(id.index()) == Some(&names[saved]) {
                to_saved.entry(id).or_insert(saved as u16);
            }
        }
        Self(RwLock::new(Numbering {
            names,
            to_process,
            to_saved,
        }))
    }

    /// The name of each block, by the number the world saves it as: what `world.ron` lists.
    pub fn names(&self) -> Vec<String> {
        self.numbering().names.clone()
    }

    fn numbering(&self) -> std::sync::RwLockReadGuard<'_, Numbering> {
        self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// `chunk`, read from a file of the world, in the numbers of the process.
    pub fn read(&self, chunk: Chunk) -> Chunk {
        let numbering = self.numbering();
        let to_process = |id: BlockId| {
            numbering
                .to_process
                .get(id.index())
                .copied()
                .unwrap_or(BlockId::Debug)
        };
        if !chunk.may_hold(|cell| cell.is_some_and(|block| to_process(block.id) != block.id)) {
            return chunk;
        }
        chunk.map_blocks(to_process)
    }

    /// Gives a number to each block of `chunks` the world does not know yet, before they are
    /// written (`write`).
    pub fn number_new_blocks<'a>(&self, chunks: impl IntoIterator<Item = &'a Chunk>) {
        let mut new = Vec::new();
        {
            let numbering = self.numbering();
            for chunk in chunks {
                chunk.may_hold(|cell| {
                    if let Some(block) = cell {
                        if !numbering.to_saved.contains_key(&block.id) && !new.contains(&block.id) {
                            new.push(block.id);
                        }
                    }
                    false
                });
            }
        }
        if new.is_empty() {
            return;
        }
        let mut numbering = self.0.write().unwrap_or_else(PoisonError::into_inner);
        for id in new {
            // a number the content does not define is saved as the debug block it reads as
            let (id, name) = match id.name() {
                Some(name) => (id, name),
                None => (BlockId::Debug, "debug"),
            };
            if numbering.to_saved.contains_key(&id) {
                continue;
            }
            let saved = numbering.names.len() as u16;
            numbering.names.push(name.to_string());
            numbering.to_process.push(id);
            numbering.to_saved.insert(id, saved);
        }
    }

    /// `chunk`, to be written to a file of the world, in the numbers of the world: the blocks
    /// the world does not know yet are numbered first (`number_new_blocks`), which a save does
    /// for all its chunks before it writes the list. A number the content does not define is
    /// saved as the debug block.
    pub fn write(&self, chunk: &Chunk) -> Chunk {
        self.number_new_blocks([chunk]);
        let numbering = self.numbering();
        let to_saved = |id: BlockId| {
            let saved = numbering
                .to_saved
                .get(&id)
                .or_else(|| numbering.to_saved.get(&BlockId::Debug));
            BlockId::from_index(*saved.expect("numbered by number_new_blocks"))
        };
        if !chunk.may_hold(|cell| cell.is_some_and(|block| to_saved(block.id) != block.id)) {
            return chunk.clone();
        }
        chunk.map_blocks(to_saved)
    }
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
}
