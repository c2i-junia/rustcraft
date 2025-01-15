use crate::constants::GRASS_COLOR;
use shared::world::{BlockData, BlockId};

/// Specifies which position in the voxel this face occupies
///
/// These faces do not render under certain circumstances to preserve resources
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
pub struct Face {
    pub direction: FaceDirection,
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub texture: String,
}

/// Structure for voxel rendering
pub struct VoxelShape {
    pub faces: Vec<Face>,
}

impl VoxelShape {
    /// Creates a VoxelShape based on the given BlockData
    pub fn create_from_block(block: &BlockData) -> VoxelShape {
        match block.id {
            BlockId::Grass => {
                let mut shape = Self::full_cube(block);

                // Apply grass-specific textures and coloring
                for (index, face) in shape.faces.iter_mut().enumerate() {
                    if index == 0 {
                        face.texture += "Top";
                        for col in face.colors.iter_mut() {
                            *col = GRASS_COLOR;
                        }
                    }
                }

                shape
            }
            BlockId::OakLog | BlockId::SpruceLog | BlockId::Cactus => {
                let mut shape = Self::full_cube(block);
                shape.faces[0].texture += "Top";
                shape.faces[1].texture += "Top";
                shape
            }
            BlockId::OakLeaves | BlockId::SpruceLeaves => {
                let mut shape = Self::full_cube(block);

                // Apply leaves color
                for face in shape.faces.iter_mut() {
                    for col in face.colors.iter_mut() {
                        *col = GRASS_COLOR;
                    }
                }

                shape
            }
            BlockId::Debug => {
                let mut shape = Self::full_cube(block);
                shape.faces[0].texture = "Top".into();
                shape.faces[1].texture = "Down".into();
                shape.faces[2].texture = "Front".into();
                shape.faces[3].texture = "Back".into();
                shape.faces[4].texture = "Left".into();
                shape.faces[5].texture = "Right".into();
                shape
            }
            BlockId::Poppy | BlockId::Dandelion => Self::flora(block),
            BlockId::TallGrass => {
                let mut shape = Self::flora(block);

                // Apply grass color to TallGrass
                for face in shape.faces.iter_mut() {
                    for col in face.colors.iter_mut() {
                        *col = GRASS_COLOR;
                    }
                }

                shape
            }
            _ => Self::full_cube(block),
        }
    }

    pub fn full_cube(block: &BlockData) -> Self {
        VoxelShape {
            faces: vec![
                Face {
                    texture: format!("{:?}", block.id),
                    direction: FaceDirection::Top,
                    vertices: vec![[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
                    indices: vec![0, 1, 2, 2, 3, 0],
                    normals: vec![
                        [0.0, 1.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 1.0, 0.0],
                    ],
                    colors: vec![
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    ],
                    uvs: vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
                },
                Face {
                    texture: format!("{:?}", block.id),
                    direction: FaceDirection::Bottom,
                    vertices: vec![[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
                    indices: vec![0, 1, 2, 2, 3, 0],
                    normals: vec![
                        [0.0, -1.0, 0.0],
                        [0.0, -1.0, 0.0],
                        [0.0, -1.0, 0.0],
                        [0.0, -1.0, 0.0],
                    ],
                    colors: vec![
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    ],
                    uvs: vec![[1., 0.], [0., 0.], [0., 1.], [1., 1.]],
                },
                Face {
                    texture: format!("{:?}", block.id),
                    direction: FaceDirection::Front,
                    vertices: vec![[1., 1., 0.], [0., 1., 0.], [0., 0., 0.], [1., 0., 0.]],
                    indices: vec![0, 3, 2, 2, 1, 0],
                    normals: vec![
                        [0.0, 0.0, -1.0],
                        [0.0, 0.0, -1.0],
                        [0.0, 0.0, -1.0],
                        [0.0, 0.0, -1.0],
                    ],
                    colors: vec![
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    ],
                    uvs: vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
                },
                Face {
                    texture: format!("{:?}", block.id),
                    direction: FaceDirection::Back,
                    vertices: vec![[1., 1., 1.], [0., 1., 1.], [0., 0., 1.], [1., 0., 1.]],
                    indices: vec![0, 1, 2, 2, 3, 0],
                    normals: vec![
                        [0.0, 0.0, 1.0],
                        [0.0, 0.0, 1.0],
                        [0.0, 0.0, 1.0],
                        [0.0, 0.0, 1.0],
                    ],
                    colors: vec![
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    ],
                    uvs: vec![[1., 0.], [0., 0.], [0., 1.], [1., 1.]],
                },
                Face {
                    texture: format!("{:?}", block.id),
                    direction: FaceDirection::Left,
                    vertices: vec![[0., 1., 1.], [0., 1., 0.], [0., 0., 0.], [0., 0., 1.]],
                    indices: vec![3, 0, 1, 1, 2, 3],
                    normals: vec![
                        [-1.0, 0.0, 0.0],
                        [-1.0, 0.0, 0.0],
                        [-1.0, 0.0, 0.0],
                        [-1.0, 0.0, 0.0],
                    ],
                    colors: vec![
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    ],
                    uvs: vec![[1., 0.], [0., 0.], [0., 1.], [1., 1.]],
                },
                Face {
                    texture: format!("{:?}", block.id),
                    direction: FaceDirection::Right,
                    vertices: vec![[1., 1., 0.], [1., 1., 1.], [1., 0., 1.], [1., 0., 0.]],
                    indices: vec![0, 1, 2, 2, 3, 0],
                    normals: vec![
                        [1.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                    ],
                    colors: vec![
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    ],
                    uvs: vec![[1., 0.], [0., 0.], [0., 1.], [1., 1.]],
                },
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
                colors: vec![
                    [1., 1., 1., 1.],
                    [1., 1., 1., 1.],
                    [1., 1., 1., 1.],
                    [1., 1., 1., 1.],
                    [1., 1., 1., 1.],
                    [1., 1., 1., 1.],
                    [1., 1., 1., 1.],
                    [1., 1., 1., 1.],
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
            }],
        }
    }
}
