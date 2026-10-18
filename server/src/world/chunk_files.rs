//! One file per chunk, `chunks/<x>_<y>_<z>.bin` in the folder of a world, holding the chunk
//! encoded like a network message: bincode, then lz4.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use bevy::{
    log::{debug, warn},
    math::IVec3,
};
use shared::{game_message_to_payload, payload_to_game_message, world::Chunk, CHUNK_SIZE};

use super::format::{write_atomically, TEMPORARY_SUFFIX};

/// Name of the folder of a world that holds its chunk files.
pub const CHUNKS_DIR: &str = "chunks";

/// Largest size, in bytes, of a valid chunk file. A chunk is a sequence of cells, each
/// encoded by bincode in well under `MAX_ENCODED_CELL_BYTES` (a tag, the block id, its
/// direction, its breaking progress and its level: 5 to 7 bytes), and lz4 never expands its input by
/// more than `n / 255 + 16` bytes, plus the 4 bytes of size prefix. A larger file is not
/// a chunk and is not read into memory.
const MAX_CHUNK_FILE_SIZE: u64 = {
    const MAX_ENCODED_CELL_BYTES: u64 = 16;
    let raw = (CHUNK_SIZE as u64).pow(3) * MAX_ENCODED_CELL_BYTES;
    raw + raw / 255 + 16 + 4
};

/// `-1_4_0.bin` for the chunk at (-1, 4, 0).
pub fn chunk_file_name(chunk_pos: IVec3) -> String {
    format!("{}_{}_{}.bin", chunk_pos.x, chunk_pos.y, chunk_pos.z)
}

/// The chunk position a file name stands for, or `None` for any other name: only the exact
/// names `chunk_file_name` writes are accepted (no `+1`, no leading zeros, no `.tmp` suffix).
pub fn parse_chunk_file_name(name: &str) -> Option<IVec3> {
    let mut coordinates = name.strip_suffix(".bin")?.split('_');
    let x = coordinates.next()?.parse().ok()?;
    let y = coordinates.next()?.parse().ok()?;
    let z = coordinates.next()?.parse().ok()?;
    if coordinates.next().is_some() {
        return None;
    }
    let chunk_pos = IVec3::new(x, y, z);
    (chunk_file_name(chunk_pos) == name).then_some(chunk_pos)
}

pub fn encode_chunk(chunk: &Chunk) -> Vec<u8> {
    game_message_to_payload(chunk)
}

/// Decodes the content of a chunk file. A file is not trusted: a truncated one, one that
/// announces a huge size, or one that does not hold exactly one chunk is an error.
pub fn decode_chunk(bytes: &[u8]) -> Result<Chunk, String> {
    payload_to_game_message(bytes).map_err(|err| err.to_string())
}

/// Writes the file of the chunk at `chunk_pos` into `chunks_dir`, atomically.
pub fn write_chunk(chunks_dir: &Path, chunk_pos: IVec3, chunk: &Chunk) -> io::Result<()> {
    write_atomically(
        &chunks_dir.join(chunk_file_name(chunk_pos)),
        &encode_chunk(chunk),
    )
}

/// Whether `chunks_dir` holds at least one file named like a chunk file.
pub fn has_chunk_files(chunks_dir: &Path) -> io::Result<bool> {
    let entries = match std::fs::read_dir(chunks_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    for entry in entries {
        if entry?
            .file_name()
            .to_str()
            .and_then(parse_chunk_file_name)
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Reads every chunk file of `chunks_dir`. A folder that does not exist holds no chunk. A file
/// that has an unexpected name, is too large to be a chunk, or cannot be read or decoded is
/// logged and skipped: the chunk it stood for is generated again. The temporary file of an
/// interrupted write (a chunk file name plus `.tmp`) is deleted. Only a folder that cannot
/// be listed is an error.
pub fn read_chunks(chunks_dir: &Path) -> io::Result<HashMap<IVec3, Chunk>> {
    read_chunks_with(chunks_dir, decode_chunk)
}

/// `read_chunks`, with the files decoded by `decode`.
pub fn read_chunks_with(
    chunks_dir: &Path,
    decode: fn(&[u8]) -> Result<Chunk, String>,
) -> io::Result<HashMap<IVec3, Chunk>> {
    let entries = match std::fs::read_dir(chunks_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(err) => return Err(err),
    };

    let mut chunks = HashMap::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("Skipping an entry of {}: {err}", chunks_dir.display());
                continue;
            }
        };
        let path = entry.path();
        let name = entry.file_name();
        let Some(chunk_pos) = name.to_str().and_then(parse_chunk_file_name) else {
            let is_orphan = name
                .to_str()
                .and_then(|name| name.strip_suffix(TEMPORARY_SUFFIX))
                .is_some_and(|name| parse_chunk_file_name(name).is_some());
            if is_orphan && path.is_file() {
                match std::fs::remove_file(&path) {
                    Ok(()) => debug!(
                        "Removed the file left by an interrupted write: {}",
                        path.display()
                    ),
                    Err(err) => warn!("Could not remove {}: {err}", path.display()),
                }
            } else {
                warn!("Ignoring {}: not a chunk file name", path.display());
            }
            continue;
        };
        match read_chunk_file(&path, decode) {
            Ok(chunk) => {
                chunks.insert(chunk_pos, chunk);
            }
            Err(err) => warn!(
                "Skipping the unreadable chunk file {}, the chunk will be generated again: {err}",
                path.display()
            ),
        }
    }
    Ok(chunks)
}

fn read_chunk_file(
    path: &Path,
    decode: fn(&[u8]) -> Result<Chunk, String>,
) -> Result<Chunk, String> {
    let size = std::fs::metadata(path)
        .map_err(|err| err.to_string())?
        .len();
    if size > MAX_CHUNK_FILE_SIZE {
        return Err(format!(
            "{size} bytes, more than the {MAX_CHUNK_FILE_SIZE} of any chunk"
        ));
    }
    decode(&std::fs::read(path).map_err(|err| err.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BlockData, BlockDirection, BlockId};

    fn sample_chunk() -> Chunk {
        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::new(1, 2, 3),
            BlockData::new(BlockId::Stone, BlockDirection::Right),
        );
        chunk.set(
            IVec3::new(15, 15, 15),
            BlockData::new(BlockId::Sand, BlockDirection::Back),
        );
        chunk
    }

    #[test]
    fn file_names_use_a_minus_sign_for_negative_coordinates() {
        assert_eq!(chunk_file_name(IVec3::new(-1, 4, 0)), "-1_4_0.bin");
        assert_eq!(chunk_file_name(IVec3::new(3, -20, 7)), "3_-20_7.bin");
    }

    #[test]
    fn file_names_round_trip_to_positions() {
        for pos in [
            IVec3::ZERO,
            IVec3::new(-1, 4, 0),
            IVec3::new(i32::MAX, i32::MIN, -7),
        ] {
            assert_eq!(parse_chunk_file_name(&chunk_file_name(pos)), Some(pos));
        }
    }

    #[test]
    fn other_names_are_not_chunk_files() {
        for name in [
            "",
            "0_0_0",
            "0_0_0.bin.tmp",
            "0_0_0.bin.corrupt",
            ".0_0_0.bin",
            "0_0.bin",
            "0_0_0_0.bin",
            "a_b_c.bin",
            "+1_0_0.bin",
            "01_0_0.bin",
            "-0_0_0.bin",
            "0__0.bin",
            "0_ 0_0.bin",
            "99999999999_0_0.bin",
            "../0_0_0.bin",
            "notes.txt",
        ] {
            assert_eq!(parse_chunk_file_name(name), None, "{name:?}");
        }
    }

    #[test]
    fn a_chunk_round_trips_through_its_encoding() {
        let chunk = sample_chunk();
        assert_eq!(decode_chunk(&encode_chunk(&chunk)).unwrap(), chunk);
    }

    #[test]
    fn an_empty_chunk_encodes_small() {
        assert!(encode_chunk(&Chunk::new()).len() < 200);
    }

    #[test]
    fn a_truncated_file_is_rejected() {
        let bytes = encode_chunk(&sample_chunk());
        for len in [0, 3, 4, bytes.len() / 2, bytes.len() - 1] {
            assert!(decode_chunk(&bytes[..len]).is_err(), "{len} bytes");
        }
    }

    #[test]
    fn a_file_announcing_a_huge_size_is_rejected() {
        let mut bytes = encode_chunk(&sample_chunk());
        bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_chunk(&bytes).is_err());
    }

    #[test]
    fn a_file_that_is_not_exactly_one_chunk_is_rejected() {
        let chunk = sample_chunk();
        // trailing bytes after a whole chunk
        assert!(decode_chunk(&game_message_to_payload((&chunk, 0u8))).is_err());
        // something else than a chunk
        assert!(decode_chunk(&game_message_to_payload("hello")).is_err());
        assert!(decode_chunk(&game_message_to_payload(vec![chunk.clone(), chunk])).is_err());
        // garbage
        assert!(decode_chunk(&[0xff; 64]).is_err());
    }

    #[test]
    fn chunks_are_written_and_read_back() {
        let tmp = tempfile::tempdir().unwrap();
        let chunk = sample_chunk();
        write_chunk(tmp.path(), IVec3::new(-1, 4, 0), &chunk).unwrap();
        write_chunk(tmp.path(), IVec3::ZERO, &Chunk::new()).unwrap();

        assert!(tmp.path().join("-1_4_0.bin").is_file());
        let chunks = read_chunks(tmp.path()).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[&IVec3::new(-1, 4, 0)], chunk);
        assert!(chunks[&IVec3::ZERO].is_empty());
    }

    #[test]
    fn a_missing_folder_holds_no_chunks() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_chunks(&tmp.path().join("chunks")).unwrap().is_empty());
    }

    #[test]
    fn corrupt_and_stray_files_are_skipped_without_failing_the_load() {
        let tmp = tempfile::tempdir().unwrap();
        write_chunk(tmp.path(), IVec3::X, &sample_chunk()).unwrap();
        std::fs::write(tmp.path().join("0_0_0.bin"), b"garbage").unwrap();
        std::fs::write(tmp.path().join("0_0_1.bin"), b"").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), b"hello").unwrap();
        std::fs::create_dir(tmp.path().join("3_0_0.bin")).unwrap();

        let chunks = read_chunks(tmp.path()).unwrap();

        assert_eq!(chunks.keys().copied().collect::<Vec<_>>(), vec![IVec3::X]);
    }

    #[test]
    fn the_leftover_of_an_interrupted_write_is_deleted_and_nothing_else_is() {
        let tmp = tempfile::tempdir().unwrap();
        write_chunk(tmp.path(), IVec3::X, &sample_chunk()).unwrap();
        std::fs::write(tmp.path().join("2_0_0.bin.tmp"), b"half a chunk").unwrap();
        std::fs::write(tmp.path().join("-2_0_0.bin.tmp"), b"half a chunk").unwrap();
        std::fs::write(tmp.path().join("notes.txt.tmp"), b"mine").unwrap();
        std::fs::write(tmp.path().join("02_0_0.bin.tmp"), b"mine").unwrap();
        std::fs::create_dir(tmp.path().join("4_0_0.bin.tmp")).unwrap();

        let chunks = read_chunks(tmp.path()).unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(!tmp.path().join("2_0_0.bin.tmp").exists());
        assert!(!tmp.path().join("-2_0_0.bin.tmp").exists());
        assert!(tmp.path().join("notes.txt.tmp").exists());
        assert!(tmp.path().join("02_0_0.bin.tmp").exists());
        assert!(tmp.path().join("4_0_0.bin.tmp").is_dir());
    }

    #[test]
    fn no_valid_chunk_file_is_larger_than_the_limit_and_larger_files_are_skipped() {
        // The worst case: every cell holds a different block, so nothing compresses
        let mut chunk = Chunk::new();
        let ids = [BlockId::Stone, BlockId::Sand, BlockId::Dirt, BlockId::Glass];
        let dirs = [
            BlockDirection::Front,
            BlockDirection::Back,
            BlockDirection::Left,
        ];
        let mut state = 12345u32;
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                    let mut block = BlockData::new(
                        ids[(state >> 8) as usize % 4],
                        dirs[(state >> 12) as usize % 3],
                    );
                    block.breaking_progress = (state >> 20) as u8;
                    chunk.set(IVec3::new(x, y, z), block);
                }
            }
        }
        let size = encode_chunk(&chunk).len() as u64;
        assert!(size <= MAX_CHUNK_FILE_SIZE, "{size}");
        assert!(
            size * 2 > 16 * 16 * 16,
            "the worst case is not compressing: {size}"
        );

        let tmp = tempfile::tempdir().unwrap();
        write_chunk(tmp.path(), IVec3::ZERO, &chunk).unwrap();
        std::fs::write(
            tmp.path().join("1_0_0.bin"),
            vec![0u8; MAX_CHUNK_FILE_SIZE as usize + 1],
        )
        .unwrap();

        let chunks = read_chunks(tmp.path()).unwrap();

        assert_eq!(
            chunks.keys().copied().collect::<Vec<_>>(),
            vec![IVec3::ZERO]
        );
    }

    #[test]
    fn chunk_files_are_detected_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!has_chunk_files(&tmp.path().join("missing")).unwrap());
        assert!(!has_chunk_files(tmp.path()).unwrap());
        std::fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        std::fs::write(tmp.path().join("0_0_0.bin.tmp"), b"x").unwrap();
        assert!(!has_chunk_files(tmp.path()).unwrap());
        std::fs::write(tmp.path().join("0_0_0.bin"), b"x").unwrap();
        assert!(has_chunk_files(tmp.path()).unwrap());
    }

    #[test]
    fn an_unlistable_folder_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("chunks");
        std::fs::write(&file, b"not a folder").unwrap();
        assert!(read_chunks(&file).is_err());
    }
}
