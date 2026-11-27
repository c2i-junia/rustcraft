//! How a block is drawn, and what the engine does with it: read from the content files with the
//! rest of its properties. The look is read by the client only
//! (`client::world::rendering::voxel`).

use serde::{Deserialize, Serialize};

/// How a block is drawn: its shape, the texture of each face, and how they are coloured and
/// moved by the wind. A block of the palette that is not a whole block takes its shape from its
/// form, and shows the first texture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub shape: Shape,
    /// The texture of each face, in the order of a cube's: top, bottom, front, back, left,
    /// right. A shape that is not a cube shows the first.
    pub faces: [String; 6],
    /// A texture drawn over the four sides, as the grass's: the tint then colours the overlay
    /// and the top only.
    pub overlay: Option<String>,
    pub tint: Tint,
    pub sway: Sway,
}

/// The shapes of the blocks that are not of the palette. Each is drawn by its own code: a new
/// shape is a Rust change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    /// A whole block.
    #[default]
    Cube,
    /// Two planes crossed, as a flower.
    Flora,
    /// A plate on the front of the cell, as a ladder or a vine.
    Plate,
    /// A stick standing in the middle of the cell.
    Torch,
    /// A stick leaning out of the block that holds it on the front of the cell.
    WallTorch,
    /// A cocoa pod.
    Pod,
    /// The box of the collision of the block.
    Box,
    /// A post with arms toward the blocks around it.
    Fence,
    /// A board on a post.
    Sign,
    /// The box of the collision, and flames in it (`CampfireFlame`).
    Campfire,
}

/// How the faces of a block are coloured: a colour map of the climate (`Grass`, `Foliage`), the
/// colour of grass where the climate is not known, or one colour everywhere.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tint {
    /// The colours of the texture.
    #[default]
    None,
    /// The colour map of the grass.
    Grass,
    /// The colour map of the leaves.
    Foliage,
    /// The colour of grass everywhere, as the needles of the spruce.
    Green,
    /// The colour of water.
    Water,
}

/// How a face moves in the wind, in `chunk.wgsl`: each point of it by how far it is from the
/// ground, so that the faces of two blocks never part.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sway {
    #[default]
    None,
    /// As leaves: every point a little.
    Whole,
    /// As a plant rooted in the block below: its top only.
    Top,
}

impl Sway {
    /// The number that names the sway in `UV_1.y` of a chunk mesh, as `chunk.wgsl` reads it.
    pub fn index(self) -> u16 {
        self as u16
    }
}

/// What the engine does with a block beyond its properties, which the code of the behaviour
/// gives: a block that names one in its file gets it with no Rust. A new behaviour is a Rust
/// change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Behaviour {
    /// Nothing more than its properties.
    #[default]
    None,
    /// It holds a `Furnace`, and smelts; lit, it turns back into its unlit block once out.
    Furnace { lit: bool },
}

/// The yes-or-no properties of a block, a bit each.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockFlags(u8);

impl BlockFlags {
    /// A right click on the block uses it, as a crafting table, instead of placing a block.
    pub const USABLE: Self = Self(1);
    /// A block placed in its cell takes its place, as in water.
    pub const REPLACEABLE: Self = Self(1 << 1);
    /// The cell below reaches into the lowest cells, as a fence does.
    pub const TALL: Self = Self(1 << 2);
    /// A glass, which a pane reaches out to.
    pub const GLASS: Self = Self(1 << 3);
    /// It holds a `Container`.
    pub const CONTAINER: Self = Self(1 << 4);
    /// It grows under water, which fills the rest of its cell.
    pub const SUBMERGED: Self = Self(1 << 5);
    /// It hangs from the block above it, rather than standing on the one below.
    pub const HANGS: Self = Self(1 << 6);

    /// Whether every flag of `other` is set.
    #[inline]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Sets the flags of `other` if `on`.
    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }
}
