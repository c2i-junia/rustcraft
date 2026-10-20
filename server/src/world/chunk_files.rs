//! The chunks of a world, in its folder `chunks`: each encoded like a network message, bincode
//! then lz4. A save writes them by regions of `REGION_SIZE^3` chunks, a file each,
//! `<x>_<y>_<z>.region`: creating a file costs far more than writing it, and a large render
//! distance saves tens of thousands of chunks. Worlds saved before have a file per chunk,
//! `<x>_<y>_<z>.bin`, still read for the chunks their regions do not hold.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use bevy::{
    log::{debug, warn},
    math::IVec3,
};
use shared::{game_message_to_payload, payload_to_game_message, world::Chunk, CHUNK_SIZE};

use super::format::{commit, write_atomically, write_pending, PendingFile, TEMPORARY_SUFFIX};

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

/// Chunks along each side of a region.
pub const REGION_SIZE: i32 = 8;

/// Chunks of a region.
const REGION_SLOTS: usize = (REGION_SIZE * REGION_SIZE * REGION_SIZE) as usize;

/// What a region file starts with, before its index.
const REGION_MAGIC: &[u8; 4] = b"RCR1";

/// The bytes of a region file before the chunks: the magic, then for each slot (`region_slot`)
/// where its chunk starts in the file and how many bytes it takes, both little-endian `u32`, 0
/// bytes for a chunk the region does not hold.
const REGION_HEADER_BYTES: usize = REGION_MAGIC.len() + REGION_SLOTS * 8;

/// The region of the chunk at `chunk_pos`.
pub fn region_of(chunk_pos: IVec3) -> IVec3 {
    chunk_pos.div_euclid(IVec3::splat(REGION_SIZE))
}

/// The place of the chunk at `chunk_pos` in the index of its region.
fn region_slot(chunk_pos: IVec3) -> usize {
    let local = chunk_pos.rem_euclid(IVec3::splat(REGION_SIZE));
    ((local.x * REGION_SIZE + local.y) * REGION_SIZE + local.z) as usize
}

/// `-1_4_0.region` for the region at (-1, 4, 0).
pub fn region_file_name(region: IVec3) -> String {
    format!("{}_{}_{}.region", region.x, region.y, region.z)
}

/// Whether `name` is a name `region_file_name` writes.
fn is_region_file_name(name: &str) -> bool {
    name.strip_suffix(".region")
        .and_then(|stem| parse_chunk_file_name(&format!("{stem}.bin")))
        .is_some_and(|region| region_file_name(region) == name)
}

/// The encoded chunks of the region file at `path`, by slot. A region that has no file holds
/// none. A file is not trusted: one whose index does not fit it, or that holds a chunk larger
/// than any valid one, is an error.
fn read_region(path: &Path) -> io::Result<Vec<Option<Vec<u8>>>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(vec![None; REGION_SLOTS]),
        Err(err) => return Err(err),
    };
    let entries = read_region_index(&mut file)?;
    entries
        .into_iter()
        .map(|entry| {
            entry
                .map(|entry| read_region_entry(&mut file, entry))
                .transpose()
        })
        .collect()
}

/// The encoded chunk at `slot` of the region file at `path`, `None` when the region has no
/// file or does not hold it.
fn read_region_slot(path: &Path, slot: usize) -> io::Result<Option<Vec<u8>>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let entry = read_region_index(&mut file)?[slot];
    entry
        .map(|entry| read_region_entry(&mut file, entry))
        .transpose()
}

/// Where each chunk of a region file starts and how long it is, checked against the size of
/// the file.
fn read_region_index(file: &mut File) -> io::Result<Vec<Option<(u64, u64)>>> {
    let invalid = |what: String| io::Error::new(io::ErrorKind::InvalidData, what);
    let size = file.metadata()?.len();
    let mut header = vec![0; REGION_HEADER_BYTES];
    file.read_exact(&mut header)
        .map_err(|_| invalid(format!("{size} bytes, shorter than the index of a region")))?;
    if &header[..REGION_MAGIC.len()] != REGION_MAGIC {
        return Err(invalid("not a region file".into()));
    }
    header[REGION_MAGIC.len()..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|entry| {
            let [offset, len] = [&entry[..4], &entry[4..]]
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()) as u64);
            match len {
                0 => Ok(None),
                _ if len > MAX_CHUNK_FILE_SIZE || offset + len > size => Err(invalid(format!(
                    "a chunk of {len} bytes at {offset} in a file of {size}"
                ))),
                _ => Ok(Some((offset, len))),
            }
        })
        .collect()
}

fn read_region_entry(file: &mut File, (offset, len): (u64, u64)) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; len as usize];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// The region file of `slots`, the encoded chunk of each slot of the region.
fn encode_region(slots: &[Option<Vec<u8>>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(
        REGION_HEADER_BYTES + slots.iter().flatten().map(Vec::len).sum::<usize>(),
    );
    bytes.extend_from_slice(REGION_MAGIC);
    let mut offset = REGION_HEADER_BYTES;
    for slot in slots {
        let len = slot.as_ref().map_or(0, Vec::len);
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        bytes.extend_from_slice(&(len as u32).to_le_bytes());
        offset += len;
    }
    for slot in slots.iter().flatten() {
        bytes.extend_from_slice(slot);
    }
    bytes
}

/// Writes the file of the region `region` into `chunks_dir`, to be committed: the chunks it
/// held, with those of `chunks` in their place. A region file that cannot be read is logged
/// and written again with `chunks` only.
pub fn write_region_pending<'a>(
    chunks_dir: &Path,
    region: IVec3,
    chunks: impl IntoIterator<Item = (IVec3, &'a Chunk)>,
) -> io::Result<PendingFile> {
    let path = chunks_dir.join(region_file_name(region));
    let mut slots = read_region(&path).unwrap_or_else(|err| {
        warn!(
            "Writing the unreadable file {} again without the chunks it held: {err}",
            path.display()
        );
        vec![None; REGION_SLOTS]
    });
    for (chunk_pos, chunk) in chunks {
        debug_assert_eq!(region_of(chunk_pos), region);
        slots[region_slot(chunk_pos)] = Some(encode_chunk(chunk));
    }
    write_pending(&path, &encode_region(&slots))
}

/// Writes the chunk at `chunk_pos` into its region in `chunks_dir`, atomically.
pub fn write_chunk_in_region(chunks_dir: &Path, chunk_pos: IVec3, chunk: &Chunk) -> io::Result<()> {
    let file = write_region_pending(chunks_dir, region_of(chunk_pos), [(chunk_pos, chunk)])?;
    commit(chunks_dir, vec![file])
        .pop()
        .expect("one result per file")
}

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
            .is_some_and(|name| parse_chunk_file_name(name).is_some() || is_region_file_name(name))
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
/// The chunks of the regions, read the same way, take the place of those of their own files.
#[cfg(test)]
pub fn read_chunks(chunks_dir: &Path) -> io::Result<HashMap<IVec3, Chunk>> {
    let mut chunks = read_chunks_with(chunks_dir, decode_chunk)?;
    let Ok(entries) = std::fs::read_dir(chunks_dir) else {
        return Ok(chunks);
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(region) = name
            .to_str()
            .filter(|name| is_region_file_name(name))
            .and_then(|name| parse_chunk_file_name(&name.replace(".region", ".bin")))
        else {
            continue;
        };
        let Ok(slots) = read_region(&entry.path()) else {
            continue;
        };
        for (slot, bytes) in slots.into_iter().enumerate() {
            let local = IVec3::new(
                slot as i32 / (REGION_SIZE * REGION_SIZE),
                slot as i32 / REGION_SIZE % REGION_SIZE,
                slot as i32 % REGION_SIZE,
            );
            if let Some(chunk) = bytes.and_then(|bytes| decode_chunk(&bytes).ok()) {
                chunks.insert(region * REGION_SIZE + local, chunk);
            }
        }
    }
    Ok(chunks)
}

/// Reads the chunk at `chunk_pos` in `chunks_dir` from its region, else from its own file, or
/// `None` when neither holds it. A file that cannot be read or decoded is logged and skipped,
/// as by `read_chunks`.
pub fn read_chunk(chunks_dir: &Path, chunk_pos: IVec3) -> Option<Chunk> {
    let region = chunks_dir.join(region_file_name(region_of(chunk_pos)));
    match read_region_slot(&region, region_slot(chunk_pos))
        .map_err(|err| err.to_string())
        .and_then(|bytes| bytes.map(|bytes| decode_chunk(&bytes)).transpose())
    {
        Ok(Some(chunk)) => return Some(chunk),
        Ok(None) => {}
        Err(err) => {
            warn!(
                "Skipping the chunk {chunk_pos} of the unreadable file {}, it will be generated \
                 again: {err}",
                region.display()
            );
            return None;
        }
    }
    read_file_of_chunk(
        chunks_dir,
        chunk_pos,
        MAX_CHUNK_FILE_SIZE,
        decode_chunk,
        "the chunk will be generated again",
    )
}

/// Reads the file named after `chunk_pos` in `dir`, as `read_files_named_by_chunk` reads each
/// of its files: a missing file is `None`, and so is one that is too large or that `decode`
/// refuses, which is logged with `when_skipped`.
pub(super) fn read_file_of_chunk<T>(
    dir: &Path,
    chunk_pos: IVec3,
    max_size: u64,
    decode: fn(&[u8]) -> Result<T, String>,
    when_skipped: &str,
) -> Option<T> {
    let path = dir.join(chunk_file_name(chunk_pos));
    match read_chunk_file(&path, max_size, decode) {
        Ok(value) => Some(value),
        Err(_) if !path.exists() => None,
        Err(err) => {
            warn!(
                "Skipping the unreadable file {}, {when_skipped}: {err}",
                path.display()
            );
            None
        }
    }
}

/// Deletes the temporary files that interrupted writes left in `dir`: a chunk file name plus
/// `.tmp`.
pub fn remove_interrupted_writes(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_orphan = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(TEMPORARY_SUFFIX))
            .is_some_and(|name| parse_chunk_file_name(name).is_some() || is_region_file_name(name));
        if is_orphan && path.is_file() {
            match std::fs::remove_file(&path) {
                Ok(()) => debug!(
                    "Removed the file left by an interrupted write: {}",
                    path.display()
                ),
                Err(err) => warn!("Could not remove {}: {err}", path.display()),
            }
        }
    }
}

/// `read_chunks`, with the files decoded by `decode`.
pub fn read_chunks_with(
    chunks_dir: &Path,
    decode: fn(&[u8]) -> Result<Chunk, String>,
) -> io::Result<HashMap<IVec3, Chunk>> {
    read_files_named_by_chunk(
        chunks_dir,
        MAX_CHUNK_FILE_SIZE,
        decode,
        "the chunk will be generated again",
    )
}

/// Reads every file of `chunks_dir` named like a chunk file, as `read_chunks` does: a file
/// larger than `max_size` or that `decode` refuses is logged, with `when_skipped` saying what
/// that means, and skipped.
pub(super) fn read_files_named_by_chunk<T>(
    chunks_dir: &Path,
    max_size: u64,
    decode: fn(&[u8]) -> Result<T, String>,
    when_skipped: &str,
) -> io::Result<HashMap<IVec3, T>> {
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
        if name.to_str().is_some_and(is_region_file_name) {
            continue;
        }
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
        match read_chunk_file(&path, max_size, decode) {
            Ok(chunk) => {
                chunks.insert(chunk_pos, chunk);
            }
            Err(err) => warn!(
                "Skipping the unreadable file {}, {when_skipped}: {err}",
                path.display()
            ),
        }
    }
    Ok(chunks)
}

fn read_chunk_file<T>(
    path: &Path,
    max_size: u64,
    decode: fn(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    let size = std::fs::metadata(path)
        .map_err(|err| err.to_string())?
        .len();
    if size > max_size {
        return Err(format!(
            "{size} bytes, more than the {max_size} of any valid file"
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
    fn a_chunk_written_cell_by_cell_by_format_version_3_is_read() {
        let chunk = sample_chunk();
        let mut cells = Vec::new();
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    cells.push(chunk.get(IVec3::new(x, y, z)).copied());
                }
            }
        }
        let version_3 = game_message_to_payload(&cells);
        assert!(version_3.len() > encode_chunk(&chunk).len());
        assert_eq!(decode_chunk(&version_3).unwrap(), chunk);
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
    fn chunks_are_written_in_their_regions_and_read_back() {
        let tmp = tempfile::tempdir().unwrap();
        let first = IVec3::new(-1, 4, 0);
        let same_region = IVec3::new(-8, 0, 7);
        let other_region = IVec3::new(8, 0, 0);
        assert_eq!(region_of(first), region_of(same_region));
        assert_ne!(region_of(first), region_of(other_region));
        let chunk = sample_chunk();
        write_chunk_in_region(tmp.path(), first, &chunk).unwrap();
        write_chunk_in_region(tmp.path(), same_region, &Chunk::new()).unwrap();
        write_chunk_in_region(tmp.path(), other_region, &chunk).unwrap();

        // Writing a chunk keeps the others of its region
        assert_eq!(read_chunk(tmp.path(), first), Some(chunk.clone()));
        assert_eq!(read_chunk(tmp.path(), same_region), Some(Chunk::new()));
        assert_eq!(read_chunk(tmp.path(), other_region), Some(chunk));
        assert_eq!(read_chunk(tmp.path(), IVec3::new(-1, 4, 1)), None);
        let mut files: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        assert_eq!(files, vec!["-1_0_0.region", "1_0_0.region"]);
        assert!(has_chunk_files(tmp.path()).unwrap());
    }

    #[test]
    fn a_chunk_its_region_does_not_hold_is_read_from_its_own_file() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = IVec3::new(1, 2, 3);
        let both = IVec3::new(1, 2, 4);
        write_chunk(tmp.path(), legacy, &sample_chunk()).unwrap();
        write_chunk(tmp.path(), both, &sample_chunk()).unwrap();
        write_chunk_in_region(tmp.path(), both, &Chunk::new()).unwrap();

        assert_eq!(read_chunk(tmp.path(), legacy), Some(sample_chunk()));
        // The region was written after the file of the chunk
        assert_eq!(read_chunk(tmp.path(), both), Some(Chunk::new()));
    }

    #[test]
    fn a_damaged_region_is_skipped_and_written_again() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(region_file_name(IVec3::ZERO));
        write_chunk_in_region(tmp.path(), IVec3::ZERO, &sample_chunk()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        for damaged in [
            bytes[..REGION_HEADER_BYTES - 1].to_vec(),
            [b"XXXX".as_slice(), &bytes[4..]].concat(),
            bytes[..bytes.len() - 1].to_vec(),
        ] {
            std::fs::write(&path, damaged).unwrap();
            assert_eq!(read_chunk(tmp.path(), IVec3::ZERO), None);
        }
        write_chunk_in_region(tmp.path(), IVec3::X, &sample_chunk()).unwrap();
        assert_eq!(read_chunk(tmp.path(), IVec3::X), Some(sample_chunk()));
        assert_eq!(read_chunk(tmp.path(), IVec3::ZERO), None);
    }

    #[test]
    fn the_leftover_of_an_interrupted_region_write_is_deleted() {
        let tmp = tempfile::tempdir().unwrap();
        write_chunk_in_region(tmp.path(), IVec3::ZERO, &sample_chunk()).unwrap();
        std::fs::write(tmp.path().join("0_0_0.region.tmp"), b"partial").unwrap();
        std::fs::write(tmp.path().join("notes.region.tmp"), b"mine").unwrap();
        remove_interrupted_writes(tmp.path());

        assert!(!tmp.path().join("0_0_0.region.tmp").exists());
        assert!(tmp.path().join("notes.region.tmp").exists());
        assert_eq!(read_chunk(tmp.path(), IVec3::ZERO), Some(sample_chunk()));
    }

    #[test]
    fn region_file_names_are_told_from_others() {
        assert!(is_region_file_name("-1_4_0.region"));
        for name in [
            "0_0.region",
            "+1_0_0.region",
            "0_0_0.region.tmp",
            "0_0_0.bin",
        ] {
            assert!(!is_region_file_name(name), "{name}");
        }
    }

    #[test]
    fn an_unlistable_folder_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("chunks");
        std::fs::write(&file, b"not a folder").unwrap();
        assert!(read_chunks(&file).is_err());
    }
}
