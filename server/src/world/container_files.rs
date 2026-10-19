//! The contents of the chests of a chunk, `containers/<x>_<y>_<z>.bin` in the folder of a world,
//! named like the file of the chunk and encoded the same way: bincode, then lz4. Only a chunk
//! that holds a chest has one.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use bevy::math::IVec3;
use shared::{
    game_message_to_payload, payload_to_game_message_limited,
    world::{to_global_pos, Chunk, Container, CHEST_SLOTS},
    CHUNK_SIZE,
};

use super::chunk_files::{chunk_file_name, read_files_named_by_chunk};
use super::format::write_atomically;

/// Name of the folder of a world that holds the contents of its chests.
pub const CONTAINERS_DIR: &str = "containers";

/// The chests of one chunk: their local position and their contents, sorted by position.
pub type ChunkContainers = Vec<(IVec3, Container)>;

/// Largest decompressed size of a valid file: a chest in every cell of the chunk, each entry
/// well under `MAX_ENCODED_ENTRY_BYTES` (a position of three varints, then 27 slots of an
/// option tag, an item, its type and a count).
const MAX_DECOMPRESSED_SIZE: u64 = {
    const MAX_ENCODED_ENTRY_BYTES: u64 = 16 + CHEST_SLOTS as u64 * 16;
    (CHUNK_SIZE as u64).pow(3) * MAX_ENCODED_ENTRY_BYTES + 16
};

/// Largest size of a valid file: what lz4 makes of the largest decompressed size, plus its
/// size prefix.
const MAX_FILE_SIZE: u64 = MAX_DECOMPRESSED_SIZE + MAX_DECOMPRESSED_SIZE / 255 + 16 + 4;

pub fn encode_containers(containers: &ChunkContainers) -> Vec<u8> {
    game_message_to_payload(containers)
}

/// Decodes the content of a file. A file is not trusted: one that does not hold exactly the
/// chests of one chunk, at positions inside it, is an error.
pub fn decode_containers(bytes: &[u8]) -> Result<ChunkContainers, String> {
    let containers: ChunkContainers =
        payload_to_game_message_limited(bytes, MAX_DECOMPRESSED_SIZE as usize)
            .map_err(|err| err.to_string())?;
    match containers
        .iter()
        .find(|(local, _)| !Chunk::contains(*local))
    {
        Some((local, _)) => Err(format!("{local} is outside a chunk")),
        None => Ok(containers),
    }
}

/// Writes the chests of the chunk at `chunk_pos` into `containers_dir`, atomically, or removes
/// its file when it holds none, so that a broken chest does not come back.
pub fn write_containers(
    containers_dir: &Path,
    chunk_pos: IVec3,
    containers: &ChunkContainers,
) -> io::Result<()> {
    let path = containers_dir.join(chunk_file_name(chunk_pos));
    if !containers.is_empty() {
        return write_atomically(&path, &encode_containers(containers));
    }
    match std::fs::remove_file(&path) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// Reads every file of `containers_dir`, as `read_chunks` reads the chunks: a damaged file is
/// logged and skipped, its chests are found empty. Returns the contents by global position.
pub fn read_containers(containers_dir: &Path) -> io::Result<HashMap<IVec3, Container>> {
    let by_chunk = read_files_named_by_chunk(
        containers_dir,
        MAX_FILE_SIZE,
        decode_containers,
        "its chests will be found empty",
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{ItemId, ItemStack};

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
        let read = read_containers(tmp.path()).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[&IVec3::new(-16, 0, 32)], holding(ItemId::Stone, 3));
        assert_eq!(read[&IVec3::new(-1, 1, 34)], holding(ItemId::Stick, 64));
    }

    #[test]
    fn a_chunk_left_without_chests_loses_its_file() {
        let tmp = tempfile::tempdir().unwrap();
        let containers = vec![(IVec3::ONE, holding(ItemId::Stone, 3))];
        write_containers(tmp.path(), IVec3::ZERO, &containers).unwrap();

        write_containers(tmp.path(), IVec3::ZERO, &Vec::new()).unwrap();
        write_containers(tmp.path(), IVec3::X, &Vec::new()).unwrap();

        assert!(!tmp.path().join("0_0_0.bin").exists());
        assert!(read_containers(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn a_missing_folder_holds_no_chests() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_containers(&tmp.path().join(CONTAINERS_DIR))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_damaged_file_is_skipped_and_the_others_load() {
        let tmp = tempfile::tempdir().unwrap();
        let containers = vec![(IVec3::ONE, holding(ItemId::Stone, 3))];
        write_containers(tmp.path(), IVec3::X, &containers).unwrap();
        std::fs::write(tmp.path().join("0_0_0.bin"), b"garbage").unwrap();
        let outside = vec![(IVec3::new(16, 0, 0), Container::default())];
        std::fs::write(tmp.path().join("2_0_0.bin"), encode_containers(&outside)).unwrap();

        let read = read_containers(tmp.path()).unwrap();

        assert_eq!(
            read.keys().copied().collect::<Vec<_>>(),
            vec![IVec3::new(17, 1, 1)]
        );
    }

    #[test]
    fn a_file_announcing_a_huge_size_is_rejected() {
        let mut bytes = encode_containers(&vec![(IVec3::ONE, Container::default())]);
        bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_containers(&bytes).is_err());
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
        assert!(decompressed <= MAX_DECOMPRESSED_SIZE, "{decompressed}");
        assert!((bytes.len() as u64) <= MAX_FILE_SIZE);
        assert_eq!(decode_containers(&bytes).unwrap().len(), 4096);
    }
}
