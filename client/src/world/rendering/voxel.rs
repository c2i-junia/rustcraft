use crate::constants::GRASS_COLOR;
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

/// Structure for cube voxel rendering
#[derive(Clone)]
pub struct Face {
    pub direction: FaceDirection,
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub texture: String,
}

const WHITE: [f32; 4] = [1.0; 4];

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
            colors: vec![WHITE; 4],
            uvs: uvs.to_vec(),
            texture: texture.to_string(),
        }
    }

    fn tint(&mut self, color: [f32; 4]) {
        self.colors.fill(color);
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
                // Only the top of a grass block is green; its texture is grey and tinted
                let mut shape = Self::full_cube(block);
                shape.faces[TOP].texture += "Top";
                shape.faces[TOP].tint(GRASS_COLOR);
                shape
            }
            BlockId::OakLog
            | BlockId::SpruceLog
            | BlockId::Cactus
            | BlockId::CraftingTable
            | BlockId::Furnace => {
                let mut shape = Self::full_cube(block);
                shape.faces[TOP].texture += "Top";
                shape.faces[BOTTOM].texture += "Top";
                shape
            }
            BlockId::OakLeaves | BlockId::SpruceLeaves => {
                Self::full_cube(block).tinted(GRASS_COLOR)
            }
            BlockId::Debug => {
                let mut shape = Self::full_cube(block);
                let names = ["Top", "Down", "Front", "Back", "Left", "Right"];
                for (face, name) in shape.faces.iter_mut().zip(names) {
                    face.texture = name.into();
                }
                shape
            }
            BlockId::Poppy | BlockId::Dandelion => Self::flora(block),
            BlockId::TallGrass => Self::flora(block).tinted(GRASS_COLOR),
            _ => Self::full_cube(block),
        }
    }

    /// Brings the top of the shape down to `height` blocks above its bottom.
    pub fn with_top_at(mut self, height: f32) -> Self {
        for vertex in self
            .faces
            .iter_mut()
            .flat_map(|face| face.vertices.iter_mut())
        {
            if vertex[1] == 1.0 {
                vertex[1] = height;
            }
        }
        self
    }

    fn tinted(mut self, color: [f32; 4]) -> Self {
        for face in self.faces.iter_mut() {
            face.tint(color);
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
                colors: vec![WHITE; 8],
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
        assert_eq!(grass.faces[TOP].colors, vec![GRASS_COLOR; 4]);
        assert_eq!(grass.faces[BOTTOM].texture, "Grass");
        assert_eq!(grass.faces[BOTTOM].colors, vec![WHITE; 4]);

        let log = shape_of(BlockId::OakLog);
        assert_eq!(log.faces[TOP].texture, "OakLogTop");
        assert_eq!(log.faces[BOTTOM].texture, "OakLogTop");
        assert_eq!(log.faces[2].texture, "OakLog");
    }

    #[test]
    fn crafting_table_and_furnace_have_their_own_top_texture() {
        for (id, top, side) in [
            (BlockId::CraftingTable, "CraftingTableTop", "CraftingTable"),
            (BlockId::Furnace, "FurnaceTop", "Furnace"),
        ] {
            let shape = shape_of(id);
            assert_eq!(shape.faces[TOP].texture, top);
            assert_eq!(shape.faces[BOTTOM].texture, top);
            assert_eq!(shape.faces[2].texture, side);
        }
    }

    #[test]
    fn leaves_and_tall_grass_are_tinted_on_every_face() {
        for id in [BlockId::OakLeaves, BlockId::TallGrass] {
            for face in shape_of(id).faces {
                assert!(
                    face.colors.iter().all(|color| *color == GRASS_COLOR),
                    "{id:?}"
                );
            }
        }
    }
}
