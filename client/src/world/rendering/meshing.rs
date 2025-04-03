use std::f32::consts::PI;
use std::{collections::HashMap, time::Instant};

use crate::world::{ClientChunk, ClientWorldMap};
use bevy::{
    math::IVec3,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
use shared::world::{to_global_pos, BlockDirection, BlockId, BlockTransparency, WorldMap};

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

#[derive(Default)]
pub struct MeshCreator {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices_offset: u32,
}

fn build_mesh(creator: &MeshCreator) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, creator.vertices.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, creator.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, creator.uvs.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, creator.colors.clone());
    mesh.insert_indices(Indices::U32(creator.indices.clone()));
    mesh
}

#[derive(Debug, Default, Clone)]
pub struct ChunkMeshResponse {
    pub solid_mesh: Option<Mesh>,
    pub liquid_mesh: Option<Mesh>,
}

pub(crate) fn generate_chunk_mesh(
    world_map: &ClientWorldMap,
    chunk: &ClientChunk,
    chunk_pos: &IVec3,
    uv_map: &HashMap<String, UvCoords>,
) -> ChunkMeshResponse {
    let start = Instant::now();

    let mut solid_mesh_creator = MeshCreator::default();
    let mut liquid_mesh_creator = MeshCreator::default();

    for (local_block_pos, block) in chunk.map.iter() {
        let x = local_block_pos.x as f32;
        let y = local_block_pos.y as f32;
        let z = local_block_pos.z as f32;

        let global_block_pos = &to_global_pos(chunk_pos, local_block_pos);
        let visibility = block.id.get_visibility();

        if is_block_surrounded(world_map, global_block_pos, &visibility, &block.id) {
            continue;
        }

        let mut local_vertices: Vec<[f32; 3]> = vec![];
        let mut local_indices: Vec<u32> = vec![];
        let mut local_normals: Vec<[f32; 3]> = vec![];
        let mut local_uvs: Vec<[f32; 2]> = vec![];
        let mut local_colors: Vec<[f32; 4]> = vec![];

        let indices_offset = if visibility == BlockTransparency::Liquid {
            &mut liquid_mesh_creator.indices_offset
        } else {
            &mut solid_mesh_creator.indices_offset
        };

        let voxel: VoxelShape = VoxelShape::create_from_block(block);

        for face in voxel.faces.iter() {
            let uv_coords: &UvCoords;

            if let Some(uvs) = uv_map.get(&face.texture) {
                uv_coords = uvs;
            } else {
                uv_coords = uv_map.get("_Default").unwrap();
            }

            let color_multiplier = 1.0 - block.breaking_progress as f32 / 60.0;

            let alpha = match visibility {
                BlockTransparency::Liquid => 0.5,
                _ => 1.0,
            };

            if should_render_face(world_map, global_block_pos, &face.direction, &visibility) {
                render_face(
                    &mut local_vertices,
                    &mut local_indices,
                    &mut local_normals,
                    &mut local_uvs,
                    &mut local_colors,
                    indices_offset,
                    face,
                    uv_coords,
                    color_multiplier,
                    alpha,
                );
            }
        }

        let local_vertices: Vec<[f32; 3]> = local_vertices
            .iter()
            .map(|v| {
                let v = rotate_vertices(v, &block.direction);
                [
                    v[0] + x,
                    if block.flipped { 1. - v[1] } else { v[1] } + y,
                    v[2] + z,
                ]
            })
            .collect();

        if visibility == BlockTransparency::Liquid {
            liquid_mesh_creator.vertices.extend(local_vertices);
            liquid_mesh_creator.indices.extend(local_indices);
            liquid_mesh_creator.normals.extend(local_normals);
            liquid_mesh_creator.uvs.extend(local_uvs);
            liquid_mesh_creator.colors.extend(local_colors);
        } else {
            solid_mesh_creator.vertices.extend(local_vertices);
            solid_mesh_creator.indices.extend(local_indices);
            solid_mesh_creator.normals.extend(local_normals);
            solid_mesh_creator.uvs.extend(local_uvs);
            solid_mesh_creator.colors.extend(local_colors);
        }
    }

    let mut solid_mesh = build_mesh(&solid_mesh_creator);
    let mut liquid_mesh = build_mesh(&liquid_mesh_creator);

    trace!("Render time : {:?}", Instant::now() - start);

    let should_return_solid = !solid_mesh_creator.vertices.is_empty();
    if should_return_solid {
        if let Err(e) = solid_mesh.generate_tangents() {
            warn!(
                "Error while generating tangents for the mesh SOLID : {:?} | {:?}",
                e, solid_mesh
            );
        }
    };

    let should_return_liquid = !liquid_mesh_creator.vertices.is_empty();
    if should_return_liquid {
        if let Err(e) = liquid_mesh.generate_tangents() {
            warn!(
                "Error while generating tangents for the mesh LIQUID : {:?} | {:?}",
                e, liquid_mesh
            );
        }
    };

    ChunkMeshResponse {
        solid_mesh: if should_return_solid {
            Some(solid_mesh)
        } else {
            None
        },
        liquid_mesh: if should_return_liquid {
            Some(liquid_mesh)
        } else {
            None
        },
    }
}

pub(crate) fn is_block_surrounded(
    world_map: &ClientWorldMap,
    global_block_pos: &IVec3,
    block_visibility: &BlockTransparency,
    block_id: &BlockId,
) -> bool {
    for offset in &shared::world::SIX_OFFSETS {
        let neighbor_pos = *global_block_pos + *offset;

        // Check if the block exists at the neighboring position
        if let Some(block) = world_map.get_block_by_coordinates(&neighbor_pos) {
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

fn render_face(
    local_vertices: &mut Vec<[f32; 3]>,
    local_indices: &mut Vec<u32>,
    local_normals: &mut Vec<[f32; 3]>,
    local_uvs: &mut Vec<[f32; 2]>,
    local_colors: &mut Vec<[f32; 4]>,
    indices_offset: &mut u32,
    face: &Face,
    uv_coords: &UvCoords,
    color_multiplier: f32,
    alpha: f32,
) {
    local_vertices.extend(face.vertices.iter());

    local_indices.extend(face.indices.iter().map(|x| x + *indices_offset));
    *indices_offset += face.vertices.len() as u32;

    local_normals.extend(face.normals.iter());

    let colors = face.colors.iter();
    let mut new_colors = vec![];
    for color in colors {
        new_colors.push([
            color[0] * color_multiplier,
            color[1] * color_multiplier,
            color[2] * color_multiplier,
            alpha,
        ]);
    }

    local_colors.extend(new_colors);

    local_uvs.extend(face.uvs.iter().map(|uv| {
        // !!! DO NOT REMOVE THE FLOAT OFFSET !!!
        // It removes seams between blocks in chunk meshes
        [
            (uv[0] + uv_coords.u0 + 0.001).min(uv_coords.u1 - 0.001),
            (uv[1] + uv_coords.v0 + 0.001).min(uv_coords.v1 - 0.001),
        ]
    }));
}

fn should_render_face(
    world_map: &ClientWorldMap,
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

    if let Some(block) = world_map.get_block_by_coordinates(&(*global_block_pos + offset)) {
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
