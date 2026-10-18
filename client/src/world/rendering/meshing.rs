use std::f32::consts::PI;
use std::{collections::HashMap, time::Instant};

use bevy::{
    math::IVec3,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use shared::world::{
    liquid::liquid_height, to_global_pos, BlockDirection, BlockId, BlockTransparency, Chunk,
    ChunkMap,
};

use super::voxel::{Face, FaceDirection, VoxelShape};

#[derive(Copy, Clone, Debug)]
pub struct UvCoords {
    pub u0: f32,
    pub u1: f32,
    pub v0: f32,
    pub v1: f32,
}

impl UvCoords {
    pub fn new(u0: f32, u1: f32, v0: f32, v1: f32) -> Self {
        Self { u0, u1, v0, v1 }
    }
}

/// The buffers of a chunk mesh while its faces are added one by one.
#[derive(Default)]
struct MeshCreator {
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

/// How far inside its texture a face stops, in atlas units, so that the neighbouring texture
/// of the atlas never bleeds along the edges of a block.
const UV_INSET: f32 = 0.001;

impl MeshCreator {
    /// Appends `face` of a block at `origin` (in chunk coordinates) turned to `direction`,
    /// drawn with the texture at `uv_coords`.
    fn push_face(
        &mut self,
        face: &Face,
        uv_coords: &UvCoords,
        alpha: f32,
        origin: Vec3,
        direction: &BlockDirection,
    ) {
        let first_index = self.vertices.len() as u32;
        self.indices
            .extend(face.indices.iter().map(|index| index + first_index));
        self.vertices.extend(face.vertices.iter().map(|vertex| {
            let vertex = rotate_vertices(vertex, direction);
            [
                vertex[0] + origin.x,
                vertex[1] + origin.y,
                vertex[2] + origin.z,
            ]
        }));
        self.normals.extend(face.normals.iter());
        self.colors.extend(
            face.colors
                .iter()
                .map(|color| [color[0], color[1], color[2], alpha]),
        );
        // !!! DO NOT REMOVE THE INSET !!!
        // It removes seams between blocks in chunk meshes
        self.uvs.extend(face.uvs.iter().map(|uv| {
            [
                (uv[0] + uv_coords.u0 + UV_INSET).min(uv_coords.u1 - UV_INSET),
                (uv[1] + uv_coords.v0 + UV_INSET).min(uv_coords.v1 - UV_INSET),
            ]
        }));
    }

    /// The mesh of the faces added so far, `None` when there is none. It has no tangents: the
    /// block materials have no normal map to use them.
    fn build(self) -> Option<Mesh> {
        if self.vertices.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.vertices);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ChunkMeshResponse {
    pub solid_mesh: Option<Mesh>,
    /// The faces of the liquids, drawn apart with a blended material.
    pub liquid_mesh: Option<Mesh>,
}

/// The mesh of `chunk`: the faces of its blocks that are not hidden by a neighbouring block,
/// in this chunk or in the ones around it.
pub(crate) fn generate_chunk_mesh(
    world_map: &ChunkMap,
    chunk: &Chunk,
    chunk_pos: &IVec3,
    uv_map: &HashMap<String, UvCoords>,
) -> ChunkMeshResponse {
    let start = Instant::now();
    let mut solid = MeshCreator::default();
    let mut liquid = MeshCreator::default();

    for (local_block_pos, block) in chunk.iter() {
        let global_block_pos = &to_global_pos(chunk_pos, &local_block_pos);
        let visibility = block.id.get_visibility();
        if is_block_surrounded(world_map, global_block_pos, &visibility, &block.id) {
            continue;
        }

        let origin = local_block_pos.as_vec3();
        let (mesh_creator, alpha) = match visibility {
            BlockTransparency::Liquid => (&mut liquid, 0.5),
            _ => (&mut solid, 1.0),
        };
        // The cracks of a block being broken (stages 0 to 9) are drawn over each of its
        // faces. A custom texture folder may lack the texture: no cracks then.
        let cracks = (block.breaking_progress > 0)
            .then(|| uv_map.get(&format!("DestroyStage{}", block.get_breaking_level())))
            .flatten();

        let mut shape = VoxelShape::create_from_block(block);
        if let Some(height) = liquid_height(world_map, *global_block_pos) {
            shape = shape.with_top_at(height);
        }
        for face in shape.faces.iter() {
            // A texture pack without this texture nor a default one: nothing to draw
            let Some(uv_coords) = uv_map.get(&face.texture).or_else(|| uv_map.get("_Default"))
            else {
                continue;
            };
            if !should_render_face(world_map, global_block_pos, &face.direction, &visibility) {
                continue;
            }
            mesh_creator.push_face(face, uv_coords, alpha, origin, &block.direction);
            if let Some(cracks) = cracks {
                mesh_creator.push_face(face, cracks, alpha, origin, &block.direction);
            }
        }
    }

    let response = ChunkMeshResponse {
        solid_mesh: solid.build(),
        liquid_mesh: liquid.build(),
    };
    trace!("Render time : {:?}", Instant::now() - start);
    response
}

pub(crate) fn is_block_surrounded(
    world_map: &ChunkMap,
    global_block_pos: &IVec3,
    block_visibility: &BlockTransparency,
    block_id: &BlockId,
) -> bool {
    for offset in &shared::world::SIX_OFFSETS {
        let neighbor_pos = *global_block_pos + *offset;

        // Check if the block exists at the neighboring position
        if let Some(block) = world_map.get_block(neighbor_pos) {
            let vis = block.id.get_visibility();
            match vis {
                BlockTransparency::Solid => {}
                BlockTransparency::Decoration => return false,
                BlockTransparency::Liquid => {
                    if vis != *block_visibility {
                        return false;
                    }
                }
                BlockTransparency::Transparent => {
                    if *block_id != block.id {
                        return false;
                    }
                }
            }
        } else {
            return false;
        }
    }

    true
}

pub fn rotate_vertices(v: &[f32; 3], direction: &BlockDirection) -> [f32; 3] {
    let angle = match *direction {
        BlockDirection::Front => 0.,
        BlockDirection::Right => -PI / 2.,
        BlockDirection::Left => PI / 2.,
        BlockDirection::Back => PI,
    };

    [
        angle.cos() * v[0] + angle.sin() * v[2],
        v[1],
        (-angle).sin() * v[0] + angle.cos() * v[2],
    ]
}

fn should_render_face(
    world_map: &ChunkMap,
    global_block_pos: &IVec3,
    direction: &FaceDirection,
    block_visibility: &BlockTransparency,
) -> bool {
    let offset = match *direction {
        FaceDirection::Front => IVec3::new(0, 0, -1),
        FaceDirection::Back => IVec3::new(0, 0, 1),
        FaceDirection::Top => IVec3::new(0, 1, 0),
        FaceDirection::Bottom => IVec3::new(0, -1, 0),
        FaceDirection::Left => IVec3::new(-1, 0, 0),
        FaceDirection::Right => IVec3::new(1, 0, 0),
        FaceDirection::Inset => return true,
    };

    if let Some(block) = world_map.get_block(*global_block_pos + offset) {
        let vis = block.id.get_visibility();
        match vis {
            BlockTransparency::Solid => false,
            BlockTransparency::Decoration => true,
            BlockTransparency::Transparent | BlockTransparency::Liquid => *block_visibility != vis,
        }
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::BlockData;

    fn cracked_stone_mesh(uv_map: &HashMap<String, UvCoords>, progress: u8) -> usize {
        let mut chunk = Chunk::new();
        let mut block = BlockData::new(BlockId::Stone, BlockDirection::Front);
        block.breaking_progress = progress;
        chunk.set(IVec3::new(1, 1, 1), block);
        let mut world = ChunkMap::default();
        world.insert_chunk(IVec3::ZERO, chunk.clone());
        generate_chunk_mesh(&world, &chunk, &IVec3::ZERO, uv_map)
            .solid_mesh
            .map_or(0, |mesh| mesh.count_vertices())
    }

    #[test]
    fn a_missing_cracks_texture_draws_no_cracks_instead_of_panicking() {
        let mut uv_map = HashMap::new();
        uv_map.insert("_Default".to_string(), UvCoords::new(0.0, 0.1, 0.0, 0.1));
        let plain = cracked_stone_mesh(&uv_map, 0);
        // Mid-way, last hit and beyond the break time: the block is drawn without cracks
        for progress in [30, 60, 200] {
            assert_eq!(cracked_stone_mesh(&uv_map, progress), plain, "{progress}");
        }

        uv_map.insert(
            "DestroyStage9".to_string(),
            UvCoords::new(0.0, 0.1, 0.0, 0.1),
        );
        assert!(cracked_stone_mesh(&uv_map, 200) > plain);
    }

    fn mesh_of(blocks: &[(IVec3, BlockId)]) -> ChunkMeshResponse {
        let mut chunk = Chunk::new();
        for (position, id) in blocks {
            chunk.set(*position, BlockData::new(*id, BlockDirection::Front));
        }
        let mut world = ChunkMap::default();
        world.insert_chunk(IVec3::ZERO, chunk.clone());
        let mut uv_map = HashMap::new();
        uv_map.insert("_Default".to_string(), UvCoords::new(0.0, 0.1, 0.0, 0.1));
        generate_chunk_mesh(&world, &chunk, &IVec3::ZERO, &uv_map)
    }

    fn vertices(mesh: &Option<Mesh>) -> usize {
        mesh.as_ref().map_or(0, |mesh| mesh.count_vertices())
    }

    #[test]
    fn water_is_drawn_in_the_liquid_mesh_and_solid_blocks_in_the_solid_one() {
        let stone = mesh_of(&[(IVec3::ONE, BlockId::Stone)]);
        assert!(vertices(&stone.solid_mesh) > 0);
        assert_eq!(vertices(&stone.liquid_mesh), 0);

        let water = mesh_of(&[(IVec3::ONE, BlockId::Water)]);
        assert_eq!(vertices(&water.solid_mesh), 0);
        assert_eq!(vertices(&water.liquid_mesh), vertices(&stone.solid_mesh));
    }

    #[test]
    fn the_surface_of_the_water_is_below_the_top_of_its_cell() {
        let highest = |response: ChunkMeshResponse| {
            let mesh = response.liquid_mesh.unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("no positions");
            };
            positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max)
        };
        assert_eq!(
            highest(mesh_of(&[(IVec3::ONE, BlockId::Water)])),
            1.0 + 8.0 / 9.0
        );
        // Under more water, a block fills its cell
        let column = mesh_of(&[
            (IVec3::ONE, BlockId::Water),
            (IVec3::new(1, 2, 1), BlockId::Water),
        ]);
        assert_eq!(highest(column), 2.0 + 8.0 / 9.0);
    }

    #[test]
    fn the_faces_between_two_water_blocks_are_hidden() {
        let one = mesh_of(&[(IVec3::ONE, BlockId::Water)]);
        let two = mesh_of(&[
            (IVec3::ONE, BlockId::Water),
            (IVec3::new(2, 1, 1), BlockId::Water),
        ]);
        // Two cubes of six faces, less the two faces they share
        assert_eq!(
            vertices(&two.liquid_mesh),
            vertices(&one.liquid_mesh) * 10 / 6
        );
    }

    #[test]
    fn a_chunk_mesh_has_no_tangents_as_no_normal_map_reads_them() {
        let mesh = mesh_of(&[(IVec3::ONE, BlockId::Stone)]).solid_mesh.unwrap();
        assert!(mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none());
    }

    #[test]
    fn a_block_without_any_texture_is_not_drawn_instead_of_panicking() {
        assert_eq!(cracked_stone_mesh(&HashMap::new(), 0), 0);
    }
}
