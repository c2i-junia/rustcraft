use crate::constants::{GRASS_COLOR, WATER_COLOR};
use bevy::math::Vec3;
use shared::world::{BlockData, BlockId};

/// Specifies which position in the voxel this face occupies
///
/// These faces do not render under certain circumstances to preserve resources
#[derive(Clone)]
pub enum FaceDirection {
    /// The face is at the top of the voxel. Won't render if the block above this one is full
    Top,
    /// The face is at the bottom of the voxel. Won't render if the block under this one is full
    Bottom,
    /// The face is at the front of the voxel. Won't render if the block in front of this one is full
    Front,
    /// The face is at the back of the voxel. Won't render if the block behind this one is full
    Back,
    /// The face is at the right of the voxel. Won't render if the block at the right of this one is full
    Right,
    /// The face is at the left of the voxel. Won't render if the block at the left of this one is full
    Left,
    /// The face is inside of the voxel. Will always render, except if the block is hidden on _all_ 6 sides
    Inset,
}

/// Which colour map of the texture pack colours a face, from the climate of its column: see
/// `climate.rs`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BiomeTint {
    /// The face keeps the colour of its vertices.
    #[default]
    None,
    Grass,
    Foliage,
}

impl BiomeTint {
    /// The number that names the colour map in `UV_1.y` of a chunk mesh, as `chunk.wgsl` reads it.
    pub fn index(self) -> u16 {
        match self {
            BiomeTint::None => 0,
            BiomeTint::Grass => 1,
            BiomeTint::Foliage => 2,
        }
    }
}

/// The colour a face is drawn with where no colour map colours it: one of the few that
/// `chunk.wgsl` holds, by `index` (`ChunkShading::palette`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FaceColor {
    #[default]
    White,
    Grass,
    Water,
}

impl FaceColor {
    /// Every colour, in the order of their indices.
    pub const ALL: [FaceColor; 3] = [FaceColor::White, FaceColor::Grass, FaceColor::Water];

    /// The number that names the colour in `UV_1.y` of a chunk mesh, as `chunk.wgsl` reads it.
    pub fn index(self) -> u16 {
        self as u16
    }

    pub fn rgba(self) -> [f32; 4] {
        match self {
            FaceColor::White => [1.0; 4],
            FaceColor::Grass => GRASS_COLOR,
            FaceColor::Water => WATER_COLOR,
        }
    }
}

/// How a face moves in the wind, in `chunk.wgsl`: each point of it by how far it is from the
/// ground, so that the faces of two blocks never part.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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

/// Structure for cube voxel rendering
#[derive(Clone)]
pub struct Face {
    pub direction: FaceDirection,
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub texture: String,
    /// The colour of the face where no colour map colours it: an item keeps it.
    pub color: FaceColor,
    /// The colour map that replaces `color` in the world, where the climate is known.
    pub biome_tint: BiomeTint,
    /// A texture drawn over `texture`, as the second element of a Minecraft model: the colour
    /// of the face, `color` or from its colour map, then colours the overlay only.
    pub overlay: Option<String>,
    pub sway: Sway,
}

impl Face {
    /// A flat square: four vertices that share one normal, split in two triangles by `indices`.
    fn quad(
        texture: &str,
        direction: FaceDirection,
        vertices: [[f32; 3]; 4],
        indices: [u32; 6],
        normal: [f32; 3],
        uvs: [[f32; 2]; 4],
    ) -> Self {
        Face {
            direction,
            vertices: vertices.to_vec(),
            indices: indices.to_vec(),
            normals: vec![normal; 4],
            uvs: uvs.to_vec(),
            texture: texture.to_string(),
            color: FaceColor::White,
            biome_tint: BiomeTint::None,
            overlay: None,
            sway: Sway::None,
        }
    }

    /// Colours the face with the colour map `biome_tint` in the world, and with `color` where the
    /// climate is not known.
    fn biome_tint(&mut self, biome_tint: BiomeTint, color: FaceColor) {
        self.color = color;
        self.biome_tint = biome_tint;
    }

    /// Takes the overlay away, for a texture pack without it: its colour went to the overlay
    /// only, so the face is not coloured either.
    pub fn remove_overlay(&mut self) {
        self.overlay = None;
        self.biome_tint(BiomeTint::None, FaceColor::White);
    }
}

/// Structure for voxel rendering
#[derive(Clone)]
pub struct VoxelShape {
    pub faces: Vec<Face>,
}

/// Index of the faces of `full_cube` whose texture differs on some blocks.
const TOP: usize = 0;
const BOTTOM: usize = 1;

impl VoxelShape {
    /// Creates a VoxelShape based on the given BlockData
    pub fn create_from_block(block: &BlockData) -> VoxelShape {
        match block.id {
            BlockId::Grass => {
                // As the model of the texture pack: the top is grey and tinted, the sides are
                // dirt under a grey overlay that is tinted, and the bottom is dirt
                let mut shape = Self::full_cube(block);
                shape.faces[TOP].texture += "Top";
                shape.faces[TOP].biome_tint(BiomeTint::Grass, FaceColor::Grass);
                shape.faces[BOTTOM].texture = "Dirt".into();
                for side in shape.faces.iter_mut().skip(BOTTOM + 1) {
                    side.biome_tint(BiomeTint::Grass, FaceColor::Grass);
                    side.overlay = Some("GrassSideOverlay".into());
                }
                shape
            }
            BlockId::CraftingTable => {
                // A texture per face, like the model of the texture pack: the front looks
                // north, toward -Z
                let mut shape = Self::full_cube(block);
                let suffixes = ["Top", "Bottom", "", "Side2", "Side1", "Side"];
                for (face, suffix) in shape.faces.iter_mut().zip(suffixes) {
                    face.texture += suffix;
                }
                shape
            }
            BlockId::OakLog
            | BlockId::SpruceLog
            | BlockId::JungleLog
            | BlockId::Cactus
            | BlockId::Furnace
            | BlockId::Chest => {
                let mut shape = Self::full_cube(block);
                shape.faces[TOP].texture += "Top";
                shape.faces[BOTTOM].texture += "Top";
                shape
            }
            // Like Minecraft's, the needles of the spruce are of one colour everywhere
            BlockId::OakLeaves | BlockId::JungleLeaves => Self::full_cube(block)
                .biome_tinted(BiomeTint::Foliage)
                .swaying(Sway::Whole),
            BlockId::SpruceLeaves => Self::full_cube(block)
                .tinted(FaceColor::Grass)
                .swaying(Sway::Whole),
            BlockId::Debug => {
                let mut shape = Self::full_cube(block);
                let names = ["Top", "Down", "Front", "Back", "Left", "Right"];
                for (face, name) in shape.faces.iter_mut().zip(names) {
                    face.texture = name.into();
                }
                shape
            }
            BlockId::Poppy | BlockId::Dandelion => Self::flora(block).swaying(Sway::Top),
            BlockId::Torch => Self::torch(block),
            BlockId::TallGrass => Self::flora(block)
                .biome_tinted(BiomeTint::Grass)
                .swaying(Sway::Top),
            // Its texture is grey too
            BlockId::Water => Self::full_cube(block).tinted(FaceColor::Water),
            _ => Self::full_cube(block),
        }
    }

    /// Brings each top corner of the shape down to its height above the bottom, in blocks: the
    /// corner at `x` and `z` (each 0 or 1) at index `x + 2 * z`. A sloping top gets the normal
    /// of its mean slope.
    pub fn with_top_corners(mut self, heights: [f32; 4]) -> Self {
        for face in self.faces.iter_mut() {
            for vertex in face.vertices.iter_mut() {
                if vertex[1] == 1.0 {
                    vertex[1] = heights[vertex[0] as usize + 2 * vertex[2] as usize];
                }
            }
            if let FaceDirection::Top = face.direction {
                let [low_x_low_z, high_x_low_z, low_x_high_z, high_x_high_z] = heights;
                let slope_x = (high_x_low_z - low_x_low_z + high_x_high_z - low_x_high_z) / 2.;
                let slope_z = (low_x_high_z - low_x_low_z + high_x_high_z - high_x_low_z) / 2.;
                let normal = Vec3::new(-slope_x, 1., -slope_z).normalize();
                face.normals.fill(normal.into());
            }
        }
        self
    }

    fn tinted(mut self, color: FaceColor) -> Self {
        for face in self.faces.iter_mut() {
            face.color = color;
        }
        self
    }

    /// Every face coloured with the colour map `biome_tint`, or `GRASS_COLOR` where the climate is
    /// not known.
    fn biome_tinted(mut self, biome_tint: BiomeTint) -> Self {
        for face in self.faces.iter_mut() {
            face.biome_tint(biome_tint, FaceColor::Grass);
        }
        self
    }

    fn swaying(mut self, sway: Sway) -> Self {
        for face in self.faces.iter_mut() {
            face.sway = sway;
        }
        self
    }

    /// The six faces of a unit cube, each with the texture named after the block, in the
    /// order top, bottom, front, back, left, right.
    pub fn full_cube(block: &BlockData) -> Self {
        // The texture as seen from outside the face, and mirrored
        const UVS: [[f32; 2]; 4] = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        const MIRRORED_UVS: [[f32; 2]; 4] = [[1., 0.], [0., 0.], [0., 1.], [1., 1.]];
        const CLOCKWISE: [u32; 6] = [0, 1, 2, 2, 3, 0];

        let texture = &format!("{:?}", block.id);
        use FaceDirection::*;
        VoxelShape {
            faces: vec![
                Face::quad(
                    texture,
                    Top,
                    [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
                    CLOCKWISE,
                    [0., 1., 0.],
                    UVS,
                ),
                Face::quad(
                    texture,
                    Bottom,
                    [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
                    CLOCKWISE,
                    [0., -1., 0.],
                    MIRRORED_UVS,
                ),
                Face::quad(
                    texture,
                    Front,
                    [[1., 1., 0.], [0., 1., 0.], [0., 0., 0.], [1., 0., 0.]],
                    [0, 3, 2, 2, 1, 0],
                    [0., 0., -1.],
                    UVS,
                ),
                Face::quad(
                    texture,
                    Back,
                    [[1., 1., 1.], [0., 1., 1.], [0., 0., 1.], [1., 0., 1.]],
                    CLOCKWISE,
                    [0., 0., 1.],
                    MIRRORED_UVS,
                ),
                Face::quad(
                    texture,
                    Left,
                    [[0., 1., 1.], [0., 1., 0.], [0., 0., 0.], [0., 0., 1.]],
                    [3, 0, 1, 1, 2, 3],
                    [-1., 0., 0.],
                    MIRRORED_UVS,
                ),
                Face::quad(
                    texture,
                    Right,
                    [[1., 1., 0.], [1., 1., 1.], [1., 0., 1.], [1., 0., 0.]],
                    CLOCKWISE,
                    [1., 0., 0.],
                    MIRRORED_UVS,
                ),
            ],
        }
    }

    /// A stick standing in the middle of the block, as Minecraft's model of the torch: the
    /// texture on four planes two pixels apart, which show the stick and the flame from every
    /// side, under a cap and over a bottom cut from the texture.
    pub fn torch(block: &BlockData) -> VoxelShape {
        const NEAR: f32 = 7. / 16.;
        const FAR: f32 = 9. / 16.;
        const TOP: f32 = 10. / 16.;
        let mut shape = Self::full_cube(block);
        for face in shape.faces.iter_mut() {
            let across = Vec3::from(face.normals[0]).abs().max_position();
            for (vertex, uv) in face.vertices.iter_mut().zip(face.uvs.iter_mut()) {
                if across == 1 {
                    // The cap and the bottom: the middle of the stick
                    vertex[0] = NEAR + vertex[0] * (FAR - NEAR);
                    vertex[2] = NEAR + vertex[2] * (FAR - NEAR);
                    let first_row = if vertex[1] == 1. { 6. } else { 13. };
                    *uv = [(7. + uv[0] * 2.) / 16., (first_row + uv[1] * 2.) / 16.];
                    vertex[1] = vertex[1].min(TOP);
                } else {
                    vertex[across] = if vertex[across] == 0. { NEAR } else { FAR };
                }
            }
            face.direction = FaceDirection::Inset;
        }
        shape
    }

    pub fn flora(block: &BlockData) -> VoxelShape {
        VoxelShape {
            faces: vec![Face {
                direction: FaceDirection::Inset,
                vertices: vec![
                    [0., 0., 0.], // 0
                    [0., 0., 1.], // 1
                    [0., 1., 0.], // 2
                    [0., 1., 1.], // 3
                    [1., 0., 0.], // 4
                    [1., 0., 1.], // 5
                    [1., 1., 0.], // 6
                    [1., 1., 1.], // 7
                ],
                indices: vec![
                    0, 5, 7, // A1
                    7, 5, 0, // B1
                    0, 2, 7, // A2
                    7, 2, 0, // B2
                    1, 4, 6, // C1
                    6, 4, 1, // C2
                    1, 3, 6, // D1
                    6, 3, 1, // D2
                ],
                normals: vec![
                    [0.5, 1., 0.5],
                    [0.5, 1., -0.5],
                    [0.5, 0., 0.5],
                    [0.5, 0., -0.5],
                    [-0.5, 1., 0.5],
                    [-0.5, 1., -0.5],
                    [-0.5, 0., 0.5],
                    [-0.5, 0., -0.5],
                ],
                uvs: vec![
                    [0., 1.],
                    [0., 1.],
                    [0., 0.],
                    [0., 0.],
                    [1., 1.],
                    [1., 1.],
                    [1., 0.],
                    [1., 0.],
                ],
                texture: format!("{:?}", block.id),
                color: FaceColor::White,
                biome_tint: BiomeTint::None,
                overlay: None,
                sway: Sway::None,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::BlockDirection;

    fn shape_of(id: BlockId) -> VoxelShape {
        VoxelShape::create_from_block(&BlockData::new(id, BlockDirection::Front))
    }

    #[test]
    fn a_cube_has_six_square_faces_whose_normal_points_out_of_the_cube() {
        let cube = shape_of(BlockId::Stone);
        assert_eq!(cube.faces.len(), 6);
        for face in &cube.faces {
            assert_eq!(face.vertices.len(), 4);
            assert_eq!(face.texture, "Stone");
            assert!(face.indices.iter().all(|index| *index < 4));
            // From the centre of the cube to the centre of the face, along the normal
            let normal = face.normals[0];
            let offset: f32 = (0..3)
                .map(|axis| {
                    let centre = face.vertices.iter().map(|v| v[axis]).sum::<f32>() / 4.;
                    (centre - 0.5) * normal[axis]
                })
                .sum();
            assert_eq!(offset, 0.5);
        }
    }

    #[test]
    fn grass_and_logs_have_their_own_top_texture() {
        let grass = shape_of(BlockId::Grass);
        assert_eq!(grass.faces[TOP].texture, "GrassTop");
        assert_eq!(grass.faces[TOP].color, FaceColor::Grass);
        assert_eq!(grass.faces[BOTTOM].texture, "Dirt");
        assert_eq!(grass.faces[BOTTOM].color, FaceColor::White);
        assert_eq!(grass.faces[BOTTOM].overlay, None);
        // The sides: dirt under the tinted overlay, as the model of the texture pack
        for side in &grass.faces[BOTTOM + 1..] {
            assert_eq!(side.texture, "Grass");
            assert_eq!(side.overlay.as_deref(), Some("GrassSideOverlay"));
            assert_eq!(side.biome_tint, BiomeTint::Grass);
            assert_eq!(side.color, FaceColor::Grass);
        }

        let log = shape_of(BlockId::OakLog);
        assert_eq!(log.faces[TOP].texture, "OakLogTop");
        assert_eq!(log.faces[BOTTOM].texture, "OakLogTop");
        assert_eq!(log.faces[2].texture, "OakLog");
    }

    #[test]
    fn the_crafting_table_has_a_texture_per_face() {
        let textures: Vec<_> = shape_of(BlockId::CraftingTable)
            .faces
            .into_iter()
            .map(|face| face.texture)
            .collect();
        assert_eq!(
            textures,
            [
                "CraftingTableTop",
                "CraftingTableBottom",
                "CraftingTable",
                "CraftingTableSide2",
                "CraftingTableSide1",
                "CraftingTableSide",
            ]
        );
    }

    #[test]
    fn furnace_and_chest_have_their_own_top_texture() {
        for (id, top, side) in [
            (BlockId::Furnace, "FurnaceTop", "Furnace"),
            (BlockId::Chest, "ChestTop", "Chest"),
        ] {
            let shape = shape_of(id);
            assert_eq!(shape.faces[TOP].texture, top);
            assert_eq!(shape.faces[BOTTOM].texture, top);
            assert_eq!(shape.faces[2].texture, side);
        }
    }

    #[test]
    fn leaves_and_tall_grass_are_tinted_on_every_face() {
        for (id, biome_tint) in [
            (BlockId::OakLeaves, BiomeTint::Foliage),
            (BlockId::TallGrass, BiomeTint::Grass),
            (BlockId::SpruceLeaves, BiomeTint::None),
        ] {
            for face in shape_of(id).faces {
                assert_eq!(face.color, FaceColor::Grass, "{id:?}");
                assert_eq!(face.biome_tint, biome_tint, "{id:?}");
            }
        }
    }

    #[test]
    fn a_torch_is_a_stick_in_the_middle_of_its_block_drawn_from_every_side() {
        let torch = shape_of(BlockId::Torch);
        assert_eq!(torch.faces.len(), 6);
        for face in &torch.faces {
            assert!(matches!(face.direction, FaceDirection::Inset));
            assert_eq!(face.texture, "Torch");
            // Each face lies within the stick, across its normal
            let across = Vec3::from(face.normals[0]).abs().max_position();
            for vertex in &face.vertices {
                let middle = if across == 1 { 0 } else { across };
                assert!(
                    (7. / 16. ..=9. / 16.).contains(&vertex[middle]),
                    "{vertex:?}"
                );
            }
        }
        // The planes span the height of the texture, the cap stops at the top of the stick
        assert!(torch.faces[2].vertices.iter().any(|v| v[1] == 1.));
        assert!(torch.faces[TOP].vertices.iter().all(|v| v[1] == 10. / 16.));
        assert!(torch.faces[TOP]
            .uvs
            .iter()
            .all(|uv| (7. / 16. ..=9. / 16.).contains(&uv[0])));
    }

    #[test]
    fn leaves_sway_whole_and_plants_by_their_top_and_the_rest_stands_still() {
        for (id, sway) in [
            (BlockId::OakLeaves, Sway::Whole),
            (BlockId::SpruceLeaves, Sway::Whole),
            (BlockId::TallGrass, Sway::Top),
            (BlockId::Poppy, Sway::Top),
            (BlockId::Grass, Sway::None),
            (BlockId::Water, Sway::None),
        ] {
            for face in shape_of(id).faces {
                assert_eq!(face.sway, sway, "{id:?}");
            }
        }
    }

    #[test]
    fn all_but_the_bottom_of_a_grass_block_take_the_colour_of_the_climate() {
        let tints: Vec<_> = shape_of(BlockId::Grass)
            .faces
            .iter()
            .map(|face| face.biome_tint)
            .collect();
        assert_eq!(tints[BOTTOM], BiomeTint::None);
        assert!(tints
            .iter()
            .enumerate()
            .all(|(index, tint)| index == BOTTOM || *tint == BiomeTint::Grass));
        assert!(shape_of(BlockId::Stone)
            .faces
            .iter()
            .all(|face| face.biome_tint == BiomeTint::None));
    }

    #[test]
    fn the_jungle_log_has_its_own_top_texture() {
        let log = shape_of(BlockId::JungleLog);
        assert_eq!(log.faces[TOP].texture, "JungleLogTop");
        assert_eq!(log.faces[BOTTOM].texture, "JungleLogTop");
        assert_eq!(log.faces[2].texture, "JungleLog");
    }

    #[test]
    fn jungle_leaves_are_tinted_and_sway_like_oak_leaves() {
        let (leaves, oak) = (
            shape_of(BlockId::JungleLeaves),
            shape_of(BlockId::OakLeaves),
        );
        assert_eq!(leaves.faces.len(), oak.faces.len());
        for (face, oak_face) in leaves.faces.iter().zip(&oak.faces) {
            assert_eq!(face.texture, "JungleLeaves");
            assert_eq!(face.biome_tint, BiomeTint::Foliage);
            assert_eq!(face.color, oak_face.color);
            assert_eq!(face.sway, oak_face.sway);
        }
    }

    /// An item with no texture of its own shows its block texture untinted, as the oak leaves
    /// and the tall grass do. Checks that the item of `id` shows its block texture as that of
    /// `sibling` does: neither has a texture of its own.
    fn assert_the_icon_is_the_block_texture_as_for(id: BlockId, sibling: BlockId) {
        let textures =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/graphics/base_textures");
        for block in [id, sibling] {
            assert!(textures.join(format!("blocks/{block:?}.png")).is_file());
            assert!(
                !textures.join(format!("items/{block:?}.png")).exists(),
                "{block:?} has an icon of its own"
            );
        }
    }

    #[test]
    fn the_jungle_leaves_icon_is_its_block_texture_as_for_the_oak_leaves() {
        assert_the_icon_is_the_block_texture_as_for(BlockId::JungleLeaves, BlockId::OakLeaves);
    }

    #[test]
    fn each_top_corner_goes_to_its_own_height_and_a_sloping_top_leans_to_the_low_side() {
        let heights = [0.25, 0.5, 0.75, 1.0];
        let shape = shape_of(BlockId::Water).with_top_corners(heights);
        for face in &shape.faces {
            for vertex in &face.vertices {
                let corner = heights[vertex[0] as usize + 2 * vertex[2] as usize];
                assert!(vertex[1] == 0. || vertex[1] == corner, "{vertex:?}");
            }
        }
        let top = Vec3::from(shape.faces[TOP].normals[0]);
        assert!(top.y > 0. && top.x < 0. && top.z < 0., "{top}");
        assert!((top.length() - 1.).abs() < 1e-6);

        let level = shape_of(BlockId::Water).with_top_corners([0.5; 4]);
        assert_eq!(level.faces[TOP].normals[0], [0., 1., 0.]);
    }
}
