//! The buildings of a village, written once as layers of characters and built of the materials
//! of the style of its biome.
//!
//! A piece is seen from above with its front, where its door is, toward `-Z`: each layer is a
//! list of rows from the front (`z = 0`) to the back, each row a character per cell from `x = 0`.
//! The layers go up from the floor (`y = 0`), which takes the place of the surface. Above the
//! floor, every cell of the box of a piece is cleared before it is built, so that `.` is air.

use shared::world::{BlockDirection, BlockId, MobKind, VillageStyle};

/// The kinds of pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceKind {
    Well,
    SmallHouse,
    BigHouse,
    Forge,
    Granary,
    Field,
    Pen,
    Lamp,
}

/// What a character of a layer stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cell {
    Air,
    /// A block of the style, turned toward a side of the piece (`Front` is its front).
    Block(Role, BlockDirection),
}

/// The part a block plays in a building, which the style turns into a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Floor,
    Wall,
    Post,
    Roof,
    Foundation,
    /// The ground of a pen: grass, sand or snow.
    Ground,
    Window,
    Door,
    DoorTop,
    Fence,
    /// A fixed block, whatever the style.
    Fixed(BlockId),
}

/// The blocks of a style for each role.
pub fn material(style: VillageStyle, role: Role) -> BlockId {
    use VillageStyle::*;
    match (role, style) {
        (Role::Fixed(block), _) => block,
        (Role::Floor, Plains) => BlockId::OakPlanks,
        (Role::Floor, Savanna) => BlockId::AcaciaPlanks,
        (Role::Floor, Desert) => BlockId::HewnStone,
        (Role::Floor, Snowy) => BlockId::SprucePlanks,
        (Role::Wall, Plains) => BlockId::OakPlanks,
        (Role::Wall, Savanna | Desert) => BlockId::RammedEarth,
        (Role::Wall, Snowy) => BlockId::SprucePlanks,
        (Role::Post, Plains) => BlockId::OakLog,
        (Role::Post, Savanna) => BlockId::AcaciaLog,
        (Role::Post, Desert) => BlockId::BrickWall,
        (Role::Post, Snowy) => BlockId::SpruceLog,
        (Role::Roof, Plains | Savanna) => BlockId::Thatch,
        (Role::Roof, Desert) => BlockId::BrickWall,
        (Role::Roof, Snowy) => BlockId::SprucePlanks,
        (Role::Foundation, Desert) => BlockId::HewnStone,
        (Role::Foundation, _) => BlockId::Cobblestone,
        (Role::Ground, Plains | Savanna) => BlockId::Grass,
        (Role::Ground, Desert) => BlockId::Sand,
        (Role::Ground, Snowy) => BlockId::Snow,
        (Role::Window, _) => BlockId::GlassPane,
        (Role::Door, _) => BlockId::Door,
        (Role::DoorTop, _) => BlockId::DoorTop,
        (Role::Fence, _) => BlockId::Fence,
    }
}

/// The block that covers the streets.
pub fn road_block(style: VillageStyle) -> BlockId {
    match style {
        VillageStyle::Desert => BlockId::Sand,
        _ => BlockId::Gravel,
    }
}

/// The cell a character stands for.
pub fn cell(c: char) -> Cell {
    use BlockDirection::*;
    let block = |role| Cell::Block(role, Front);
    let fixed = |id| Cell::Block(Role::Fixed(id), Front);
    match c {
        '.' => Cell::Air,
        'F' => block(Role::Floor),
        'W' => block(Role::Wall),
        'P' => block(Role::Post),
        'R' => block(Role::Roof),
        'S' => block(Role::Foundation),
        'N' => block(Role::Ground),
        'G' => block(Role::Window),
        'D' => block(Role::Door),
        'd' => block(Role::DoorTop),
        'E' => block(Role::Fence),
        'h' => fixed(BlockId::HewnStone),
        'f' => fixed(BlockId::WoodenSlab),
        'w' => fixed(BlockId::Water),
        'o' => fixed(BlockId::TilledSoil),
        'y' => fixed(BlockId::RipeRye),
        'Y' => fixed(BlockId::YoungRye),
        'B' => fixed(BlockId::Bedroll),
        'C' => fixed(BlockId::Chest),
        'A' => fixed(BlockId::Barrel),
        'K' => fixed(BlockId::CraftingTable),
        'L' => fixed(BlockId::Lantern),
        'H' => fixed(BlockId::HangingLantern),
        'c' => fixed(BlockId::Campfire),
        // a furnace turned toward the front, a ladder against the back wall
        'U' => Cell::Block(Role::Fixed(BlockId::Furnace), Front),
        'X' => Cell::Block(Role::Fixed(BlockId::Ladder), Back),
        _ => panic!("no cell for {c:?}"),
    }
}

/// A building, as layers of characters (see the module).
pub struct Piece {
    pub kind: PieceKind,
    /// The layers, from the floor up, each a row per `z` from the front.
    pub layers: &'static [&'static [&'static str]],
    /// The column of the front row the street leads to: the door.
    pub door: i32,
    /// The chance of the piece among those along the streets, and how many a village holds at
    /// most.
    pub weight: u32,
    pub most: usize,
}

impl Piece {
    /// Width along x, depth along z, and height.
    pub fn size(&self) -> (i32, i32, i32) {
        let floor = self.layers[0];
        (
            floor[0].len() as i32,
            floor.len() as i32,
            self.layers.len() as i32,
        )
    }

    /// Every cell of the piece but the cleared ones: `(x, y, z)` and the cell.
    pub fn cells(&self) -> impl Iterator<Item = ((i32, i32, i32), Cell)> + '_ {
        self.layers.iter().enumerate().flat_map(|(y, layer)| {
            layer.iter().enumerate().flat_map(move |(z, row)| {
                row.chars()
                    .enumerate()
                    .map(move |(x, c)| ((x as i32, y as i32, z as i32), cell(c)))
            })
        })
    }

    /// The animals a pen of this style holds, chosen from.
    pub fn animals(&self, style: VillageStyle) -> &'static [MobKind] {
        if self.kind != PieceKind::Pen {
            return &[];
        }
        match style {
            VillageStyle::Plains => &[MobKind::Sheep, MobKind::Pig, MobKind::Chicken, MobKind::Cow],
            VillageStyle::Savanna => &[MobKind::Cow, MobKind::Goat, MobKind::Chicken],
            VillageStyle::Desert => &[MobKind::Goat, MobKind::Chicken],
            VillageStyle::Snowy => &[MobKind::Sheep, MobKind::Rabbit],
        }
    }
}

/// The well in the middle of the square.
pub const WELL: Piece = Piece {
    kind: PieceKind::Well,
    layers: &[
        &["hhhhh", "hwwwh", "hwwwh", "hwwwh", "hhhhh"],
        &["hhhhh", "h...h", "h...h", "h...h", "hhhhh"],
        &["E...E", ".....", ".....", ".....", "E...E"],
        &["E...E", ".....", "..H..", ".....", "E...E"],
        &["fffff", "fffff", "fffff", "fffff", "fffff"],
    ],
    door: 2,
    weight: 0,
    most: 1,
};

/// A lantern on a post, along the streets.
pub const LAMP: Piece = Piece {
    kind: PieceKind::Lamp,
    layers: &[&["N"], &["E"], &["E"], &["L"]],
    door: 0,
    weight: 0,
    most: usize::MAX,
};

/// The pieces built along the streets.
pub const PIECES: [Piece; 6] = [
    Piece {
        kind: PieceKind::SmallHouse,
        layers: &[
            &["SSSSS", "SFFFS", "SFFFS", "SFFFS", "SSSSS"],
            &["PWDWP", "W...W", "W...W", "WB.KW", "PWWWP"],
            &["PWdWP", "W...W", "G...G", "W...W", "PWGWP"],
            &["PWWWP", "W...W", "W.H.W", "W...W", "PWWWP"],
            &["RRRRR", "RRRRR", "RRRRR", "RRRRR", "RRRRR"],
            &[".....", ".RRR.", ".RRR.", ".RRR.", "....."],
            &[".....", ".....", "..R..", ".....", "....."],
        ],
        door: 2,
        weight: 40,
        most: usize::MAX,
    },
    Piece {
        kind: PieceKind::BigHouse,
        layers: &[
            &[
                "SSSSSSS", "SFFFFFS", "SFFFFFS", "SFFFFFS", "SFFFFFS", "SFFFFFS", "SFFFFFS",
                "SFFFFFS", "SSSSSSS",
            ],
            &[
                "PWWDWWP", "W.....W", "G.....G", "W.....W", "WK...AW", "W.....W", "G.....G",
                "WC...XW", "PWWWWWP",
            ],
            &[
                "PWWdWWP", "W.....W", "G.....G", "W.....W", "W.....W", "W.....W", "G.....G",
                "W....XW", "PWGWGWP",
            ],
            &[
                "PWWWWWP", "W.....W", "W.....W", "W.....W", "W..H..W", "W.....W", "W.....W",
                "W....XW", "PWWWWWP",
            ],
            &[
                "PWWWWWP", "WFFFFFW", "WFFFFFW", "WFFFFFW", "WFFFFFW", "WFFFFFW", "WFFFFFW",
                "WFFFFXW", "PWWWWWP",
            ],
            &[
                "PWGWGWP", "W.....W", "W.....W", "WB....W", "W.....W", "WB....W", "W.....W",
                "W.....W", "PWWWWWP",
            ],
            &[
                "PWWWWWP", "W.....W", "G.....G", "W.....W", "W..H..W", "W.....W", "G.....G",
                "W.....W", "PWWWWWP",
            ],
            &[
                "RRRRRRR", "RRRRRRR", "RRRRRRR", "RRRRRRR", "RRRRRRR", "RRRRRRR", "RRRRRRR",
                "RRRRRRR", "RRRRRRR",
            ],
            &[
                ".......", ".RRRRR.", ".RRRRR.", ".RRRRR.", ".RRRRR.", ".RRRRR.", ".RRRRR.",
                ".RRRRR.", ".......",
            ],
            &[
                ".......", ".......", "..RRR..", "..RRR..", "..RRR..", "..RRR..", "..RRR..",
                ".......", ".......",
            ],
            &[
                ".......", ".......", ".......", "...R...", "...R...", "...R...", ".......",
                ".......", ".......",
            ],
        ],
        door: 3,
        weight: 14,
        most: usize::MAX,
    },
    Piece {
        kind: PieceKind::Forge,
        layers: &[
            &[
                "SSSSSSS", "SSSSSSS", "SSSSSSS", "SSSSSSS", "SSSSSSS", "SSSSSSS",
            ],
            &[
                "P.....P", "W.....W", "W.....W", "WC..U.W", "WA..hhW", "PWWWWWP",
            ],
            &[
                "P.....P", "G.....G", "W.....W", "W.....W", "W.....W", "PWGWGWP",
            ],
            &[
                "P.....P", "W.....W", "W..H..W", "W.....W", "W.....W", "PWWWWWP",
            ],
            &[
                "RRRRRRR", "RRRRRRR", "RRRRRRR", "RRRRRRR", "RRRRRRR", "RRRRRRR",
            ],
            &[
                ".......", ".RRRRR.", ".RRRRR.", ".RRRRR.", ".RRRRR.", ".......",
            ],
        ],
        door: 3,
        weight: 7,
        most: 1,
    },
    Piece {
        kind: PieceKind::Granary,
        layers: &[
            &[
                "SSSSS", "SFFFS", "SFFFS", "SFFFS", "SFFFS", "SFFFS", "SSSSS",
            ],
            &[
                "PWDWP", "W...W", "WA.AW", "W...W", "WA.CW", "WA.AW", "PWWWP",
            ],
            &[
                "PWdWP", "W...W", "W...W", "G...G", "W...W", "W...W", "PWWWP",
            ],
            &[
                "PWWWP", "W...W", "W...W", "W.H.W", "W...W", "W...W", "PWWWP",
            ],
            &[
                "RRRRR", "RRRRR", "RRRRR", "RRRRR", "RRRRR", "RRRRR", "RRRRR",
            ],
            &[
                ".....", ".RRR.", ".RRR.", ".RRR.", ".RRR.", ".RRR.", ".....",
            ],
            &[
                ".....", ".....", "..R..", "..R..", "..R..", ".....", ".....",
            ],
        ],
        door: 2,
        weight: 10,
        most: 2,
    },
    Piece {
        kind: PieceKind::Field,
        layers: &[
            &[
                "PPPPPPPPP",
                "PooowoooP",
                "PooowoooP",
                "PooowoooP",
                "PooowoooP",
                "PooowoooP",
                "PPPPPPPPP",
            ],
            &[
                ".........",
                ".yyy.YYY.",
                ".YYY.yyy.",
                ".yyy.YYY.",
                ".YYY.yyy.",
                ".yyy.YYY.",
                ".........",
            ],
        ],
        door: 4,
        weight: 15,
        most: 4,
    },
    Piece {
        kind: PieceKind::Pen,
        layers: &[
            &[
                "NNNNNNN", "NNNNNNN", "NNNNNNN", "NNNNNNN", "NNNNNNN", "NNNNNNN", "NNNNNNN",
            ],
            &[
                "EEEDEEE", "E.....E", "E.....E", "E.....E", "E.....E", "E.....E", "EEEEEEE",
            ],
            &[
                "L..d..L", ".......", ".......", ".......", ".......", ".......", "L.....L",
            ],
        ],
        door: 3,
        weight: 9,
        most: 2,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_of_a_piece_has_the_same_size_and_known_cells() {
        for piece in PIECES.iter().chain([&WELL, &LAMP]) {
            let (width, depth, _) = piece.size();
            for (y, layer) in piece.layers.iter().enumerate() {
                assert_eq!(layer.len() as i32, depth, "{:?} layer {y}", piece.kind);
                for row in layer.iter() {
                    assert_eq!(row.len() as i32, width, "{:?} layer {y}: {row}", piece.kind);
                    row.chars().for_each(|c| {
                        cell(c);
                    });
                }
            }
            assert!((0..width).contains(&piece.door), "{:?}", piece.kind);
        }
    }

    #[test]
    fn a_door_has_its_top_above_it_and_opens_on_the_front() {
        for piece in PIECES.iter() {
            let doors: Vec<_> = piece
                .cells()
                .filter(|(_, cell)| *cell == Cell::Block(Role::Door, BlockDirection::Front))
                .map(|(at, _)| at)
                .collect();
            for (x, y, z) in doors {
                assert_eq!((x, z), (piece.door, 0), "{:?}", piece.kind);
                let top = piece.layers[y as usize + 1][z as usize].as_bytes()[x as usize];
                assert_eq!(top, b'd', "{:?}", piece.kind);
            }
        }
    }

    #[test]
    fn every_style_has_a_block_for_every_role() {
        let styles = [
            VillageStyle::Plains,
            VillageStyle::Savanna,
            VillageStyle::Desert,
            VillageStyle::Snowy,
        ];
        for style in styles {
            for role in [
                Role::Floor,
                Role::Wall,
                Role::Post,
                Role::Roof,
                Role::Ground,
            ] {
                assert_ne!(material(style, role), BlockId::Debug);
            }
        }
    }
}
