use std::fmt;

use bevy::math::IVec3;
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{world::BlockData, CHUNK_SIZE};

/// Number of cells of a chunk.
const CHUNK_VOLUME: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;

/// A cube of `CHUNK_SIZE^3` cells, each holding a block or air (`None`), in one allocation.
///
/// Positions are local: every coordinate is in `0..CHUNK_SIZE`.
#[derive(Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Indexed by `(x * CHUNK_SIZE + y) * CHUNK_SIZE + z`.
    cells: Box<[Option<BlockData>]>,
}

impl Chunk {
    /// A chunk made of air only.
    pub fn new() -> Self {
        Self {
            cells: vec![None; CHUNK_VOLUME].into_boxed_slice(),
        }
    }

    /// Whether a local position is inside the chunk: every coordinate is in `0..CHUNK_SIZE`.
    pub fn contains(local: IVec3) -> bool {
        local.cmpge(IVec3::ZERO).all() && local.cmplt(IVec3::splat(CHUNK_SIZE)).all()
    }

    /// The index of a local position, `None` if it is outside the chunk.
    fn index(local: IVec3) -> Option<usize> {
        Self::contains(local)
            .then(|| ((local.x * CHUNK_SIZE + local.y) * CHUNK_SIZE + local.z) as usize)
    }

    /// The local position of a cell index. Inverse of `index` for `index < CHUNK_VOLUME`.
    fn local_pos(index: usize) -> IVec3 {
        let index = index as i32;
        IVec3::new(
            index / (CHUNK_SIZE * CHUNK_SIZE),
            index / CHUNK_SIZE % CHUNK_SIZE,
            index % CHUNK_SIZE,
        )
    }

    /// The block at a local position, `None` for air and for a position outside the chunk.
    pub fn get(&self, local: IVec3) -> Option<&BlockData> {
        self.cells[Self::index(local)?].as_ref()
    }

    /// Stores `block` at a local position. A position outside the chunk is a programming
    /// error: it is ignored (and trips a `debug_assert!` in debug builds).
    pub fn set(&mut self, local: IVec3, block: BlockData) {
        let Some(index) = Self::index(local) else {
            debug_assert!(false, "local position {local} is outside the chunk");
            return;
        };
        self.cells[index] = Some(block);
    }

    /// Empties a local position and returns the block that was there. Same contract as `set`
    /// for a position outside the chunk.
    pub fn remove(&mut self, local: IVec3) -> Option<BlockData> {
        let Some(index) = Self::index(local) else {
            debug_assert!(false, "local position {local} is outside the chunk");
            return None;
        };
        self.cells[index].take()
    }

    /// Whether the chunk is made of air only.
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(Option::is_none)
    }

    /// The blocks of the chunk with their local position, in ascending order of `x`, then `y`,
    /// then `z`.
    pub fn iter(&self) -> impl Iterator<Item = (IVec3, &BlockData)> {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| Some((Self::local_pos(index), cell.as_ref()?)))
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Chunk({} blocks)", self.iter().count())
    }
}

/// A chunk is serialized as its sequence of `CHUNK_VOLUME` cells, air included: a plain
/// sequence is as compact as it gets once compressed, and stays readable in RON.
impl Serialize for Chunk {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.cells.iter())
    }
}

impl<'de> Deserialize<'de> for Chunk {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CellsVisitor;

        impl<'de> Visitor<'de> for CellsVisitor {
            type Value = Chunk;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a sequence of exactly {CHUNK_VOLUME} cells")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Chunk, A::Error> {
                // Never reads past the expected length: the announced one comes from the network.
                let mut chunk = Chunk::new();
                for (index, cell) in chunk.cells.iter_mut().enumerate() {
                    *cell = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(index, &self))?;
                }
                if seq.next_element::<Option<BlockData>>()?.is_some() {
                    return Err(de::Error::invalid_length(CHUNK_VOLUME + 1, &self));
                }
                Ok(chunk)
            }
        }

        deserializer.deserialize_seq(CellsVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockDirection, BlockId};

    fn block(id: BlockId) -> BlockData {
        BlockData::new(id, BlockDirection::Front)
    }

    #[test]
    fn a_new_chunk_is_air() {
        let chunk = Chunk::new();
        assert!(chunk.is_empty());
        assert_eq!(chunk.iter().count(), 0);
        assert_eq!(chunk.get(IVec3::new(3, 4, 5)), None);
    }

    #[test]
    fn set_get_and_remove_round_trip_on_every_corner() {
        let corners = [0, CHUNK_SIZE - 1];
        let mut chunk = Chunk::new();
        for &x in &corners {
            for &y in &corners {
                for &z in &corners {
                    let pos = IVec3::new(x, y, z);
                    chunk.set(pos, block(BlockId::Stone));
                    assert_eq!(chunk.get(pos), Some(&block(BlockId::Stone)));
                }
            }
        }
        assert_eq!(chunk.iter().count(), 8);
        assert_eq!(
            chunk.remove(IVec3::new(0, 15, 0)),
            Some(block(BlockId::Stone))
        );
        assert_eq!(chunk.get(IVec3::new(0, 15, 0)), None);
        assert_eq!(chunk.remove(IVec3::new(0, 15, 0)), None);
        assert_eq!(chunk.iter().count(), 7);
    }

    #[test]
    fn every_position_has_its_own_cell() {
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let pos = IVec3::new(x, y, z);
                    let mut chunk = Chunk::new();
                    chunk.set(pos, block(BlockId::Stone));
                    let listed: Vec<IVec3> = chunk.iter().map(|(p, _)| p).collect();
                    assert_eq!(listed, vec![pos]);
                    assert_eq!(chunk.get(pos), Some(&block(BlockId::Stone)));
                }
            }
        }
    }

    #[test]
    fn contains_matches_the_range() {
        assert!(Chunk::contains(IVec3::ZERO));
        assert!(Chunk::contains(IVec3::splat(CHUNK_SIZE - 1)));
        assert!(!Chunk::contains(IVec3::new(-1, 0, 0)));
        assert!(!Chunk::contains(IVec3::new(0, CHUNK_SIZE, 0)));
        assert!(!Chunk::contains(IVec3::new(0, 0, i32::MAX)));
    }

    #[test]
    fn set_replaces_the_block() {
        let mut chunk = Chunk::new();
        let pos = IVec3::new(1, 2, 3);
        chunk.set(pos, block(BlockId::Dirt));
        chunk.set(pos, block(BlockId::Sand));
        assert_eq!(chunk.get(pos).map(|b| b.id), Some(BlockId::Sand));
        assert_eq!(chunk.iter().count(), 1);
    }

    #[test]
    fn is_empty_follows_the_content() {
        let mut chunk = Chunk::new();
        chunk.set(IVec3::new(7, 7, 7), block(BlockId::Glass));
        assert!(!chunk.is_empty());
        chunk.remove(IVec3::new(7, 7, 7));
        assert!(chunk.is_empty());
    }

    #[test]
    fn iter_gives_local_positions_in_x_then_y_then_z_order() {
        let mut chunk = Chunk::new();
        let positions = [
            IVec3::new(15, 0, 0),
            IVec3::new(0, 0, 15),
            IVec3::new(0, 1, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(2, 3, 4),
        ];
        for pos in positions {
            chunk.set(pos, block(BlockId::Stone));
        }
        let listed: Vec<IVec3> = chunk.iter().map(|(pos, _)| pos).collect();
        assert_eq!(
            listed,
            vec![
                IVec3::new(0, 0, 1),
                IVec3::new(0, 0, 15),
                IVec3::new(0, 1, 0),
                IVec3::new(2, 3, 4),
                IVec3::new(15, 0, 0),
            ]
        );
    }

    #[test]
    fn out_of_range_reads_return_none() {
        let mut chunk = Chunk::new();
        chunk.set(IVec3::new(0, 0, 0), block(BlockId::Stone));
        for pos in [
            IVec3::new(-1, 0, 0),
            IVec3::new(0, -1, 0),
            IVec3::new(0, 0, -1),
            IVec3::new(CHUNK_SIZE, 0, 0),
            IVec3::new(0, CHUNK_SIZE, 0),
            IVec3::new(0, 0, CHUNK_SIZE),
            IVec3::new(0, 0, CHUNK_SIZE * CHUNK_SIZE),
        ] {
            assert_eq!(chunk.get(pos), None, "{pos}");
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "outside the chunk")]
    fn out_of_range_set_is_a_programming_error() {
        Chunk::new().set(IVec3::new(CHUNK_SIZE, 0, 0), block(BlockId::Stone));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "outside the chunk")]
    fn out_of_range_remove_is_a_programming_error() {
        Chunk::new().remove(IVec3::new(0, -1, 0));
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn out_of_range_writes_are_ignored() {
        let mut chunk = Chunk::new();
        chunk.set(IVec3::new(CHUNK_SIZE, 0, 0), block(BlockId::Stone));
        assert_eq!(chunk.remove(IVec3::new(0, -1, 0)), None);
        assert!(chunk.is_empty());
    }

    fn sample() -> Chunk {
        let mut chunk = Chunk::new();
        chunk.set(IVec3::new(0, 0, 0), block(BlockId::Bedrock));
        chunk.set(
            IVec3::new(5, 6, 7),
            BlockData {
                breaking_progress: 3,
                ..BlockData::new(BlockId::OakLog, BlockDirection::Left)
            },
        );
        chunk.set(IVec3::new(15, 15, 15), block(BlockId::Water));
        chunk
    }

    #[test]
    fn bincode_round_trip() {
        let chunk = sample();
        let bytes = bincode::serialize(&chunk).unwrap();
        assert_eq!(bincode::deserialize::<Chunk>(&bytes).unwrap(), chunk);
    }

    #[test]
    fn a_payload_with_the_wrong_cell_count_is_rejected() {
        let cells = vec![None::<BlockData>; CHUNK_VOLUME];
        for count in [0, 1, CHUNK_VOLUME - 1, CHUNK_VOLUME + 1, CHUNK_VOLUME * 2] {
            let payload: Vec<Option<BlockData>> =
                cells.iter().copied().cycle().take(count).collect();
            let bytes = bincode::serialize(&payload).unwrap();
            assert!(
                bincode::deserialize::<Chunk>(&bytes).is_err(),
                "{count} cells were accepted"
            );
        }
    }

    #[test]
    fn a_payload_announcing_an_absurd_length_is_rejected() {
        let mut bytes = bincode::serialize(&Vec::<Option<BlockData>>::new()).unwrap();
        bytes.splice(0..8, u64::MAX.to_le_bytes());
        assert!(bincode::deserialize::<Chunk>(&bytes).is_err());
    }
}
