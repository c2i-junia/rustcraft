//! What the blocks of a chunk hold, a file per kind of block and per chunk in the folder of a
//! world: the contents of the chests in `containers/<x>_<y>_<z>.bin`, the furnaces in
//! `furnaces/<x>_<y>_<z>.bin`, named like the file of the chunk and encoded the same way:
//! bincode, then lz4. Only a chunk that holds such a block has its file.

#[cfg(test)]
use std::collections::HashMap;
use std::io;
use std::path::Path;

use bevy::{log::warn, math::IVec3};
use serde::{de::DeserializeOwned, Serialize};
use shared::{
    game_message_to_payload, payload_to_game_message_limited,
    world::{to_global_pos, BlockId, Chunk, ChunkMap, Container, Furnace, CHEST_SLOTS},
    CHUNK_SIZE,
};

#[cfg(test)]
use super::chunk_files::read_files_named_by_chunk;
use super::chunk_files::{chunk_file_name, read_file_of_chunk};
#[cfg(test)]
use super::format::write_atomically;
use super::format::{write_pending, PendingFile};

/// Name of the folder of a world that holds the contents of its chests.
pub const CONTAINERS_DIR: &str = "containers";

/// Name of the folder of a world that holds its furnaces.
pub const FURNACES_DIR: &str = "furnaces";

/// What a kind of block holds, kept in a file per chunk.
pub trait Stored: Serialize + DeserializeOwned + Default {
    /// The folder of a world that holds the files.
    const DIR: &'static str;
    /// More than the encoded size of one entry: its position, three varints, and what the
    /// block holds.
    const MAX_ENCODED_ENTRY_BYTES: u64;
    /// What a damaged file means, for the log.
    const LOST: &'static str;
    /// The name of the block, for the log.
    const NAME: &'static str;

    /// Whether a block `id` holds one.
    fn held_by(id: BlockId) -> bool;

    /// Puts `value` into `chunks`, at `pos`, where a block that holds one stands.
    fn insert_into(chunks: &mut ChunkMap, pos: IVec3, value: Self);
}

impl Stored for Container {
    const DIR: &'static str = CONTAINERS_DIR;
    // 27 slots of an option tag, an item, its type and a count
    const MAX_ENCODED_ENTRY_BYTES: u64 = 16 + CHEST_SLOTS as u64 * 16;
    const LOST: &'static str = "its chests will be found empty";
    const NAME: &'static str = "chest";

    fn held_by(id: BlockId) -> bool {
        id == BlockId::Chest
    }

    fn insert_into(chunks: &mut ChunkMap, pos: IVec3, value: Self) {
        chunks.insert_container(pos, value);
    }
}

impl Stored for Furnace {
    const DIR: &'static str = FURNACES_DIR;
    // Three stacks as a chest's slots, then three varints
    const MAX_ENCODED_ENTRY_BYTES: u64 = 16 + 3 * 16 + 16;
    const LOST: &'static str = "its furnaces will be found empty";
    const NAME: &'static str = "furnace";

    fn held_by(id: BlockId) -> bool {
        id.is_furnace()
    }

    fn insert_into(chunks: &mut ChunkMap, pos: IVec3, value: Self) {
        chunks.insert_furnace(pos, value);
    }
}

/// The blocks of one kind of a chunk: their local position and what they hold, sorted by
/// position.
pub type ChunkContainers<T = Container> = Vec<(IVec3, T)>;

/// What the blocks of a chunk hold, by kind.
#[derive(Default, Debug, PartialEq)]
pub struct ChunkContents {
    pub chests: ChunkContainers,
    pub furnaces: ChunkContainers<Furnace>,
}

/// Largest decompressed size of a valid file: one in every cell of the chunk.
fn max_decompressed_size<T: Stored>() -> u64 {
    (CHUNK_SIZE as u64).pow(3) * T::MAX_ENCODED_ENTRY_BYTES + 16
}

/// Largest size of a valid file: what lz4 makes of the largest decompressed size, plus its
/// size prefix.
fn max_file_size<T: Stored>() -> u64 {
    let decompressed = max_decompressed_size::<T>();
    decompressed + decompressed / 255 + 16 + 4
}

pub fn encode_containers<T: Stored>(containers: &ChunkContainers<T>) -> Vec<u8> {
    game_message_to_payload(containers)
}

/// Decodes the content of a file. A file is not trusted: one that does not hold exactly the
/// blocks of one chunk, at positions inside it, is an error.
pub fn decode_containers<T: Stored>(bytes: &[u8]) -> Result<ChunkContainers<T>, String> {
    let containers: ChunkContainers<T> =
        payload_to_game_message_limited(bytes, max_decompressed_size::<T>() as usize)
            .map_err(|err| err.to_string())?;
    match containers
        .iter()
        .find(|(local, _)| !Chunk::contains(*local))
    {
        Some((local, _)) => Err(format!("{local} is outside a chunk")),
        None => Ok(containers),
    }
}

/// Writes what the blocks of the chunk at `chunk_pos` hold into `containers_dir`, atomically,
/// or removes its file when it holds none, so that a broken chest does not come back.
#[cfg(test)]
pub fn write_containers<T: Stored>(
    containers_dir: &Path,
    chunk_pos: IVec3,
    containers: &ChunkContainers<T>,
) -> io::Result<()> {
    let path = containers_dir.join(chunk_file_name(chunk_pos));
    if !containers.is_empty() {
        return write_atomically(&path, &encode_containers(containers));
    }
    remove_containers(containers_dir, chunk_pos)
}

/// Writes what the blocks of the chunk at `chunk_pos`, which holds some, hold into
/// `containers_dir`, to be committed.
pub fn write_containers_pending<T: Stored>(
    containers_dir: &Path,
    chunk_pos: IVec3,
    containers: &ChunkContainers<T>,
) -> io::Result<PendingFile> {
    write_pending(
        &containers_dir.join(chunk_file_name(chunk_pos)),
        &encode_containers(containers),
    )
}

/// Removes the file of the chunk at `chunk_pos` from `containers_dir`, if there is one.
pub fn remove_containers(containers_dir: &Path, chunk_pos: IVec3) -> io::Result<()> {
    match std::fs::remove_file(containers_dir.join(chunk_file_name(chunk_pos))) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// Reads every file of `containers_dir`, as `read_chunks` reads the chunks: a damaged file is
/// logged and skipped, its blocks are found empty. Returns what they hold by global position.
#[cfg(test)]
pub fn read_containers<T: Stored>(containers_dir: &Path) -> io::Result<HashMap<IVec3, T>> {
    let by_chunk = read_files_named_by_chunk(
        containers_dir,
        max_file_size::<T>(),
        decode_containers::<T>,
        T::LOST,
    )?;
    Ok(by_chunk
        .into_iter()
        .flat_map(|(chunk_pos, containers)| {
            containers
                .into_iter()
                .map(move |(local, container)| (to_global_pos(&chunk_pos, &local), container))
        })
        .collect())
}

/// Loads `chunk` at `chunk_pos` into `chunks` with what its blocks hold, `contents`. A chest or
/// a furnace found without a saved state is empty, and a state found where no such block
/// stands is dropped.
pub fn insert_with_contents(
    chunks: &mut ChunkMap,
    chunk_pos: IVec3,
    chunk: Chunk,
    contents: ChunkContents,
) {
    let chests = held_in::<Container>(&chunk);
    let furnaces = held_in::<Furnace>(&chunk);
    chunks.insert_chunk(chunk_pos, chunk);
    insert_held(chunks, chunk_pos, chests, contents.chests);
    insert_held(chunks, chunk_pos, furnaces, contents.furnaces);
}

/// The local positions of the blocks of `chunk` that hold a `T`.
fn held_in<T: Stored>(chunk: &Chunk) -> Vec<IVec3> {
    chunk
        .iter()
        .filter(|(_, block)| T::held_by(block.id))
        .map(|(local, _)| local)
        .collect()
}

/// Puts into `chunks` what each block at `found` in the chunk at `chunk_pos` holds: its value
/// among `values`, else an empty one. The values left are dropped.
fn insert_held<T: Stored>(
    chunks: &mut ChunkMap,
    chunk_pos: IVec3,
    found: Vec<IVec3>,
    mut values: ChunkContainers<T>,
) {
    for local in found {
        let value = match values.iter().position(|(at, _)| *at == local) {
            Some(index) => values.swap_remove(index).1,
            None => T::default(),
        };
        T::insert_into(chunks, to_global_pos(&chunk_pos, &local), value);
    }
    for (local, _) in values {
        warn!(
            "Dropping the state of a {} at {}, where none stands",
            T::NAME,
            to_global_pos(&chunk_pos, &local)
        );
    }
}

/// What the blocks of the chunk at `chunk_pos` hold, read from their files in the folder of
/// the world `world_dir`: none of a kind without its file, and none, logged, when it is
/// damaged.
pub fn read_chunk_contents(world_dir: &Path, chunk_pos: IVec3) -> ChunkContents {
    ChunkContents {
        chests: read_chunk_containers(world_dir, chunk_pos),
        furnaces: read_chunk_containers(world_dir, chunk_pos),
    }
}

/// The blocks of one kind of the chunk at `chunk_pos`, read from their file in the folder of
/// the world `world_dir`.
fn read_chunk_containers<T: Stored>(world_dir: &Path, chunk_pos: IVec3) -> ChunkContainers<T> {
    read_file_of_chunk(
        &world_dir.join(T::DIR),
        chunk_pos,
        max_file_size::<T>(),
        decode_containers::<T>,
        T::LOST,
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{ItemId, ItemStack};

    #[test]
    fn loaded_chests_get_their_contents_or_empty_ones_and_stray_contents_are_dropped() {
        use shared::world::{BlockData, BlockDirection};

        let mut chunk = Chunk::new();
        for local in [IVec3::new(1, 0, 0), IVec3::new(2, 0, 0)] {
            chunk.set(local, BlockData::new(BlockId::Chest, BlockDirection::Front));
        }
        let held = holding(ItemId::Stone, 2);
        let chests = vec![
            (IVec3::new(1, 0, 0), held.clone()),
            (IVec3::new(3, 0, 0), held.clone()),
        ];

        let mut chunks = ChunkMap::default();
        let contents = ChunkContents {
            chests,
            ..Default::default()
        };
        insert_with_contents(&mut chunks, IVec3::X, chunk, contents);

        assert_eq!(chunks.container(IVec3::new(17, 0, 0)), Some(&held));
        assert_eq!(
            chunks.container(IVec3::new(18, 0, 0)),
            Some(&Container::default())
        );
        assert_eq!(chunks.container(IVec3::new(19, 0, 0)), None);
        assert!(chunks.take_changed_containers().is_empty());
        assert!(chunks.take_changed_blocks().is_empty());
    }

    #[test]
    fn loaded_furnaces_get_their_state_or_empty_ones_and_stray_states_are_dropped() {
        use shared::world::{BlockData, BlockDirection};

        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::new(1, 0, 0),
            BlockData::new(BlockId::Furnace, BlockDirection::Front),
        );
        chunk.set(
            IVec3::new(2, 0, 0),
            BlockData::new(BlockId::LitFurnace, BlockDirection::Left),
        );
        chunk.set(
            IVec3::new(3, 0, 0),
            BlockData::new(BlockId::Chest, BlockDirection::Front),
        );
        let lit = burning(ItemId::Sand, 5);
        let contents = ChunkContents {
            chests: Vec::new(),
            furnaces: vec![
                (IVec3::new(2, 0, 0), lit.clone()),
                // A furnace's state where a chest stands
                (IVec3::new(3, 0, 0), lit.clone()),
            ],
        };

        let mut chunks = ChunkMap::default();
        insert_with_contents(&mut chunks, IVec3::X, chunk, contents);

        assert_eq!(
            chunks.furnace(IVec3::new(17, 0, 0)),
            Some(&Furnace::default())
        );
        assert_eq!(chunks.furnace(IVec3::new(18, 0, 0)), Some(&lit));
        assert_eq!(chunks.furnace(IVec3::new(19, 0, 0)), None);
        assert_eq!(
            chunks.container(IVec3::new(19, 0, 0)),
            Some(&Container::default())
        );
        assert!(chunks.take_changed_furnaces().is_empty());
    }

    fn burning(item_id: ItemId, nb: u32) -> Furnace {
        Furnace {
            input: Some(ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb,
            }),
            burn_left: 1200,
            burn_total: 1600,
            cook: 40,
            ..Default::default()
        }
    }

    #[test]
    fn the_furnaces_of_a_chunk_are_written_and_read_back_by_global_position() {
        let tmp = tempfile::tempdir().unwrap();
        let furnaces = vec![
            (IVec3::new(0, 0, 0), burning(ItemId::Sand, 3)),
            (IVec3::new(15, 1, 2), Furnace::default()),
        ];
        write_containers(tmp.path(), IVec3::new(-1, 0, 2), &furnaces).unwrap();

        let read = read_containers::<Furnace>(tmp.path()).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[&IVec3::new(-16, 0, 32)], burning(ItemId::Sand, 3));
        assert_eq!(read[&IVec3::new(-1, 1, 34)], Furnace::default());

        write_containers::<Furnace>(tmp.path(), IVec3::new(-1, 0, 2), &Vec::new()).unwrap();
        assert!(read_containers::<Furnace>(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn the_contents_of_a_chunk_are_read_from_their_folders_and_missing_ones_are_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let chests = vec![(IVec3::ONE, holding(ItemId::Stone, 3))];
        let furnaces = vec![(IVec3::new(2, 3, 4), burning(ItemId::Cobblestone, 1))];
        let world = tmp.path();
        std::fs::create_dir_all(world.join(CONTAINERS_DIR)).unwrap();
        std::fs::create_dir_all(world.join(FURNACES_DIR)).unwrap();
        write_containers(&world.join(CONTAINERS_DIR), IVec3::X, &chests).unwrap();
        write_containers(&world.join(FURNACES_DIR), IVec3::X, &furnaces).unwrap();
        std::fs::create_dir_all(world.join("other")).unwrap();

        let read = read_chunk_contents(world, IVec3::X);
        assert_eq!((read.chests, read.furnaces), (chests, furnaces));

        // A world saved before the furnaces has no folder for them
        std::fs::remove_dir_all(world.join(FURNACES_DIR)).unwrap();
        let read = read_chunk_contents(world, IVec3::X);
        assert!(read.furnaces.is_empty());
        assert_eq!(read.chests.len(), 1);
        assert_eq!(
            read_chunk_contents(world, IVec3::Y),
            ChunkContents::default()
        );
    }

    #[test]
    fn a_furnace_in_every_cell_fits_the_limits() {
        let full = ItemStack {
            item_id: ItemId::Chest,
            item_type: ItemId::Chest.get_default_type(),
            nb: u32::MAX,
        };
        let furnace = Furnace {
            input: Some(full),
            fuel: Some(full),
            output: Some(full),
            burn_left: u16::MAX,
            burn_total: u16::MAX,
            cook: u16::MAX,
        };
        let furnaces: ChunkContainers<Furnace> = (0..CHUNK_SIZE)
            .flat_map(|x| {
                (0..CHUNK_SIZE).flat_map(move |y| (0..CHUNK_SIZE).map(move |z| (x, y, z)))
            })
            .map(|(x, y, z)| (IVec3::new(x, y, z), furnace.clone()))
            .collect();
        let bytes = encode_containers(&furnaces);
        let decompressed = u32::from_le_bytes(*bytes.first_chunk().unwrap()) as u64;
        assert!(
            decompressed <= max_decompressed_size::<Furnace>(),
            "{decompressed}"
        );
        assert!((bytes.len() as u64) <= max_file_size::<Furnace>());
        assert_eq!(decode_containers::<Furnace>(&bytes).unwrap().len(), 4096);
    }

    fn holding(item_id: ItemId, nb: u32) -> Container {
        let mut container = Container::default();
        container.slots[CHEST_SLOTS as usize - 1] = Some(ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        });
        container
    }

    #[test]
    fn the_chests_of_a_chunk_are_written_and_read_back_by_global_position() {
        let tmp = tempfile::tempdir().unwrap();
        let containers = vec![
            (IVec3::new(0, 0, 0), holding(ItemId::Stone, 3)),
            (IVec3::new(15, 1, 2), holding(ItemId::Stick, 64)),
        ];
        write_containers(tmp.path(), IVec3::new(-1, 0, 2), &containers).unwrap();

        assert!(tmp.path().join("-1_0_2.bin").is_file());
        let read = read_containers::<Container>(tmp.path()).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[&IVec3::new(-16, 0, 32)], holding(ItemId::Stone, 3));
        assert_eq!(read[&IVec3::new(-1, 1, 34)], holding(ItemId::Stick, 64));
    }

    #[test]
    fn a_chunk_left_without_chests_loses_its_file() {
        let tmp = tempfile::tempdir().unwrap();
        let containers = vec![(IVec3::ONE, holding(ItemId::Stone, 3))];
        write_containers(tmp.path(), IVec3::ZERO, &containers).unwrap();

        write_containers::<Container>(tmp.path(), IVec3::ZERO, &Vec::new()).unwrap();
        write_containers::<Container>(tmp.path(), IVec3::X, &Vec::new()).unwrap();

        assert!(!tmp.path().join("0_0_0.bin").exists());
        assert!(read_containers::<Container>(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn a_missing_folder_holds_no_chests() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            read_containers::<Container>(&tmp.path().join(CONTAINERS_DIR))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_damaged_file_is_skipped_and_the_others_load() {
        let tmp = tempfile::tempdir().unwrap();
        let containers = vec![(IVec3::ONE, holding(ItemId::Stone, 3))];
        write_containers(tmp.path(), IVec3::X, &containers).unwrap();
        std::fs::write(tmp.path().join("0_0_0.bin"), b"garbage").unwrap();
        let outside = vec![(IVec3::new(16, 0, 0), Container::default())];
        std::fs::write(tmp.path().join("2_0_0.bin"), encode_containers(&outside)).unwrap();

        let read = read_containers::<Container>(tmp.path()).unwrap();

        assert_eq!(
            read.keys().copied().collect::<Vec<_>>(),
            vec![IVec3::new(17, 1, 1)]
        );
    }

    #[test]
    fn a_file_announcing_a_huge_size_is_rejected() {
        let mut bytes = encode_containers(&vec![(IVec3::ONE, Container::default())]);
        bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_containers::<Container>(&bytes).is_err());
    }

    #[test]
    fn a_chest_in_every_cell_fits_the_limits() {
        let mut full = Container::default();
        for (slot, stack) in full.slots.iter_mut().enumerate() {
            *stack = Some(ItemStack {
                item_id: ItemId::Chest,
                item_type: ItemId::Chest.get_default_type(),
                nb: u32::MAX - slot as u32,
            });
        }
        let containers: ChunkContainers = (0..CHUNK_SIZE)
            .flat_map(|x| {
                (0..CHUNK_SIZE).flat_map(move |y| (0..CHUNK_SIZE).map(move |z| (x, y, z)))
            })
            .map(|(x, y, z)| (IVec3::new(x, y, z), full.clone()))
            .collect();
        let bytes = encode_containers(&containers);
        let decompressed = u32::from_le_bytes(*bytes.first_chunk().unwrap()) as u64;
        assert!(
            decompressed <= max_decompressed_size::<Container>(),
            "{decompressed}"
        );
        assert!((bytes.len() as u64) <= max_file_size::<Container>());
        assert_eq!(decode_containers::<Container>(&bytes).unwrap().len(), 4096);
    }
}
