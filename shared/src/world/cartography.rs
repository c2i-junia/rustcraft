//! What the maps know of the world: the surface seen from above, which the minimap and the paper
//! maps draw, the paper maps the server keeps by id, and the waypoints of each player.
//!
//! A paper map (`ItemId::FilledMap`, of `ItemType::Map`) shows a square of `PAPER_MAP_SIDE`
//! blocks, one block a pixel, fixed when it is first drawn (`paper_map_centre`): it fills as its
//! holder walks over it. The server holds its cells (`PaperMap`) and sends them to the players
//! who hold it or may see it in a frame.
//!
//! The waypoints (`Waypoints`) are kept with the player: where it last died, the places it
//! found, villages and dungeons, and the markers it named on the world map. The bedroll is the
//! server's (`Places`), and comes with them in the `WaypointsView` it sends.

use bevy::math::{IVec2, IVec3, Vec3};
use serde::{Deserialize, Serialize};

use super::{BlockId, BlockTransparency, Chunk, ChunkMap};
use crate::CHUNK_SIZE;

/// The deepest water the map tells apart, in blocks: deeper water looks the same.
pub const MAX_WATER_DEPTH: u8 = 16;

/// What the map shows of a column of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    /// The top block seen from above. Under water, the floor of the water, or water when the
    /// floor is deeper than `MAX_WATER_DEPTH` or not loaded.
    pub block: BlockId,
    /// The height of that block, or of the surface of the water over it.
    pub y: i32,
    /// The blocks of water over the floor, up to `MAX_WATER_DEPTH`; 0 out of the water.
    pub water_depth: u8,
}

/// Whether the map sees through a block, to the block under it.
pub fn seen_through(block: BlockId) -> bool {
    block == BlockId::Glass || block.get_visibility() == BlockTransparency::Decoration
}

/// What the map shows at `x`, `z`, looking down through `chunks`, the loaded chunks of its
/// column from the top, with their heights.
pub fn scan_cell(chunks: &[(i32, &Chunk)], x: i32, z: i32) -> Option<Surface> {
    let local = IVec2::new(x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));
    let mut water_top: Option<i32> = None;
    let water = |floor: BlockId, top: i32, depth: i32| Surface {
        block: floor,
        y: top,
        water_depth: depth.min(i32::from(MAX_WATER_DEPTH)) as u8,
    };
    for &(chunk_y, chunk) in chunks {
        for local_y in (0..CHUNK_SIZE).rev() {
            let y = chunk_y * CHUNK_SIZE + local_y;
            if let Some(top) = water_top {
                if top - y >= i32::from(MAX_WATER_DEPTH) {
                    return Some(water(BlockId::Water, top, top - y));
                }
            }
            let Some(block) = chunk.get(IVec3::new(local.x, local_y, local.y)) else {
                continue;
            };
            if seen_through(block.id) {
                continue;
            }
            if block.id == BlockId::Water {
                water_top.get_or_insert(y);
                continue;
            }
            return Some(match water_top {
                Some(top) => water(block.id, top, top - y),
                None => Surface {
                    block: block.id,
                    y,
                    water_depth: 0,
                },
            });
        }
    }
    // Water over chunks that are not loaded: as deep as the map tells
    water_top.map(|top| water(BlockId::Water, top, i32::from(MAX_WATER_DEPTH)))
}

/// The id of a paper map, given by the server in the order they are drawn.
pub type MapId = u32;

/// The side of the square a paper map shows, in blocks, one a pixel.
pub const PAPER_MAP_SIDE: i32 = 128;

/// How far from its holder a paper map fills, in blocks.
pub const PAPER_MAP_REACH: i32 = 40;

/// The centre of the paper map drawn at `position`: the maps of a world are laid on a grid of
/// `PAPER_MAP_SIDE`, centred on the origin, so that two drawn side by side meet.
pub fn paper_map_centre(position: Vec3) -> IVec2 {
    let column = position.floor().as_ivec3();
    let half = PAPER_MAP_SIDE / 2;
    IVec2::new(
        (column.x + half).div_euclid(PAPER_MAP_SIDE) * PAPER_MAP_SIDE,
        (column.z + half).div_euclid(PAPER_MAP_SIDE) * PAPER_MAP_SIDE,
    )
}

/// The cells of a paper map: what its holders saw of each column of its square.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaperMap {
    /// The column (x, z) at its centre.
    pub centre: IVec2,
    /// Row after row from the north-west corner, `PAPER_MAP_SIDE` by `PAPER_MAP_SIDE`. `None`
    /// where nobody holding it went.
    pub cells: Vec<Option<Surface>>,
}

impl PaperMap {
    pub fn new(centre: IVec2) -> Self {
        Self {
            centre,
            cells: vec![None; (PAPER_MAP_SIDE * PAPER_MAP_SIDE) as usize],
        }
    }

    /// The column (x, z) of its north-west corner.
    pub fn corner(&self) -> IVec2 {
        self.centre - IVec2::splat(PAPER_MAP_SIDE / 2)
    }

    /// The index of the cell of the column `(x, z)`, if the map shows it.
    pub fn index(&self, x: i32, z: i32) -> Option<usize> {
        let at = IVec2::new(x, z) - self.corner();
        let inside = at.cmpge(IVec2::ZERO).all() && at.cmplt(IVec2::splat(PAPER_MAP_SIDE)).all();
        inside.then(|| (at.y * PAPER_MAP_SIDE + at.x) as usize)
    }

    /// What the map shows at `(x, z)`.
    pub fn get(&self, x: i32, z: i32) -> Option<Surface> {
        self.cells.get(self.index(x, z)?).copied().flatten()
    }

    /// Fills the cells within `reach` columns of `around` from the loaded chunks of `map` from
    /// the chunk `top` down to the chunk `bottom`. Returns whether a cell changed.
    pub fn fill(
        &mut self,
        map: &ChunkMap,
        around: Vec3,
        reach: i32,
        (top, bottom): (i32, i32),
    ) -> bool {
        let centre = IVec2::new(around.x.floor() as i32, around.z.floor() as i32);
        let mut changed = false;
        let mut columns: Vec<(IVec2, Vec<(i32, &Chunk)>)> = Vec::new();
        for z in centre.y - reach..=centre.y + reach {
            for x in centre.x - reach..=centre.x + reach {
                let offset = IVec2::new(x, z) - centre;
                if offset.length_squared() > reach * reach {
                    continue;
                }
                let Some(index) = self.index(x, z) else {
                    continue;
                };
                let column = IVec2::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
                let chunks = match columns.iter().position(|(at, _)| *at == column) {
                    Some(found) => &columns[found].1,
                    None => {
                        let chunks = (bottom..=top)
                            .rev()
                            .filter_map(|y| {
                                Some((y, map.chunk(IVec3::new(column.x, y, column.y))?))
                            })
                            .filter(|(_, chunk)| chunk.uniform() != Some(None))
                            .collect();
                        columns.push((column, chunks));
                        &columns[columns.len() - 1].1
                    }
                };
                if chunks.is_empty() {
                    continue;
                }
                let seen = scan_cell(chunks, x, z);
                if seen.is_some() && self.cells[index] != seen {
                    self.cells[index] = seen;
                    changed = true;
                }
            }
        }
        changed
    }
}

/// The kinds of the places a player finds by going near them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    Village,
    Ruin,
    Crypt,
    Sanctuary,
    /// The lair of the Ember Mother.
    Lair,
}

impl PlaceKind {
    /// The key of its name in the translations.
    pub fn key(self) -> &'static str {
        match self {
            Self::Village => "waypoint.Village",
            Self::Ruin => "waypoint.Ruin",
            Self::Crypt => "waypoint.Crypt",
            Self::Sanctuary => "waypoint.Sanctuary",
            Self::Lair => "waypoint.Lair",
        }
    }
}

/// A place a player found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundPlace {
    pub kind: PlaceKind,
    pub at: IVec3,
}

/// The colours a marker may take, as sRGB: their index is what is kept.
pub const MARKER_COLORS: [[u8; 3]; 8] = [
    [0xE5, 0x3B, 0x3B],
    [0xF2, 0x9B, 0x2E],
    [0xF2, 0xD7, 0x3A],
    [0x5C, 0xC2, 0x4A],
    [0x3B, 0xB5, 0xD9],
    [0x4A, 0x6C, 0xE5],
    [0xB0, 0x5C, 0xE0],
    [0xF2, 0xF2, 0xF2],
];

/// The most markers a player names.
pub const MAX_MARKERS: usize = 64;

/// The longest name of a marker, in characters.
pub const MAX_MARKER_NAME: usize = 24;

/// A place a player named on the world map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub name: String,
    /// Its index in `MARKER_COLORS`.
    pub color: u8,
    pub at: IVec3,
}

/// Two places found of one kind are the same within this many blocks of each other.
const SAME_PLACE: i32 = 48;

/// What a player keeps of where it was: saved with it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Waypoints {
    /// Where it last died.
    #[serde(default)]
    pub last_death: Option<IVec3>,
    /// The places it found, in the order it found them.
    #[serde(default)]
    pub found: Vec<FoundPlace>,
    /// The places it named.
    #[serde(default)]
    pub markers: Vec<Marker>,
}

impl Waypoints {
    /// Adds the place `kind` at `at`, unless one of its kind is already known near it. Returns
    /// whether it was new.
    pub fn find(&mut self, kind: PlaceKind, at: IVec3) -> bool {
        let known = self
            .found
            .iter()
            .any(|place| place.kind == kind && (place.at - at).abs().max_element() <= SAME_PLACE);
        if !known {
            self.found.push(FoundPlace { kind, at });
        }
        !known
    }

    /// Adds a marker named `name` (cleaned and cut to `MAX_MARKER_NAME`) of the colour `color`
    /// at `at`. Refused when the player has `MAX_MARKERS` already, the name has no letter left,
    /// or the colour is not one of `MARKER_COLORS`.
    pub fn mark(&mut self, name: &str, color: u8, at: IVec3) -> bool {
        let name: String = name
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_MARKER_NAME)
            .collect();
        let name = name.trim().to_string();
        if name.is_empty()
            || usize::from(color) >= MARKER_COLORS.len()
            || self.markers.len() >= MAX_MARKERS
        {
            return false;
        }
        self.markers.push(Marker { name, color, at });
        true
    }

    /// Takes away the marker of index `index`, if there is one.
    pub fn unmark(&mut self, index: usize) -> bool {
        if index < self.markers.len() {
            self.markers.remove(index);
            true
        } else {
            false
        }
    }
}

/// What the server tells a player of its waypoints.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WaypointsView {
    /// Where it comes back to: its bedroll, or the spawn of the world without one. The compass
    /// points there.
    pub home: IVec3,
    /// Whether `home` is a bedroll of its own.
    pub bedroll: bool,
    pub waypoints: Waypoints,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockData, BlockDirection};

    #[test]
    fn the_paper_maps_lie_on_a_grid_around_the_origin() {
        assert_eq!(paper_map_centre(Vec3::new(0.5, 70.0, -0.5)), IVec2::ZERO);
        assert_eq!(
            paper_map_centre(Vec3::new(63.9, 0.0, -64.5)),
            IVec2::new(0, -128)
        );
        assert_eq!(
            paper_map_centre(Vec3::new(64.0, 0.0, 200.0)),
            IVec2::new(128, 256)
        );
        let map = PaperMap::new(IVec2::new(128, 0));
        assert_eq!(map.index(64, -64), Some(0));
        assert_eq!(
            map.index(191, 63),
            Some((PAPER_MAP_SIDE * PAPER_MAP_SIDE - 1) as usize)
        );
        assert_eq!(map.index(63, 0), None);
        assert_eq!(map.index(192, 0), None);
    }

    #[test]
    fn a_paper_map_fills_around_its_holder_only() {
        let mut world = ChunkMap::default();
        let mut chunk = Chunk::new();
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                chunk.set(
                    IVec3::new(x, 3, z),
                    BlockData::new(BlockId::Grass, BlockDirection::Front),
                );
            }
        }
        world.insert_chunk(IVec3::ZERO, chunk);
        let mut map = PaperMap::new(IVec2::ZERO);
        assert!(map.fill(&world, Vec3::new(8.0, 5.0, 8.0), 4, (1, -1)));
        let grass = Some(Surface {
            block: BlockId::Grass,
            y: 3,
            water_depth: 0,
        });
        assert_eq!(map.get(8, 8), grass);
        assert_eq!(map.get(12, 8), grass);
        // beyond the reach, and beyond the loaded chunks
        assert_eq!(map.get(13, 8), None);
        assert_eq!(map.get(-1, 8), None);
        // nothing new the second time
        assert!(!map.fill(&world, Vec3::new(8.0, 5.0, 8.0), 4, (1, -1)));
    }

    #[test]
    fn markers_are_cleaned_and_bounded_and_places_found_once() {
        let mut waypoints = Waypoints::default();
        assert!(waypoints.mark("  Home\tsweet home  ", 2, IVec3::ONE));
        assert_eq!(waypoints.markers[0].name, "Homesweet home");
        assert!(!waypoints.mark("   ", 0, IVec3::ZERO));
        assert!(!waypoints.mark("Mine", MARKER_COLORS.len() as u8, IVec3::ZERO));
        assert!(waypoints.mark(&"x".repeat(40), 0, IVec3::ZERO));
        assert_eq!(waypoints.markers[1].name.chars().count(), MAX_MARKER_NAME);
        while waypoints.markers.len() < MAX_MARKERS {
            assert!(waypoints.mark("More", 1, IVec3::ZERO));
        }
        assert!(!waypoints.mark("One too many", 1, IVec3::ZERO));
        assert!(waypoints.unmark(0));
        assert!(!waypoints.unmark(MAX_MARKERS));

        assert!(waypoints.find(PlaceKind::Village, IVec3::new(100, 60, 100)));
        assert!(!waypoints.find(PlaceKind::Village, IVec3::new(120, 64, 90)));
        assert!(waypoints.find(PlaceKind::Ruin, IVec3::new(120, 64, 90)));
        assert!(waypoints.find(PlaceKind::Village, IVec3::new(400, 60, 100)));
        assert_eq!(waypoints.found.len(), 3);
    }
}
