use std::collections::HashMap;
use std::sync::LazyLock;

use bevy::{
    asset::RenderAssetUsages,
    math::IVec3,
    mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexAttributeValues, VertexFormat},
    prelude::*,
};
use shared::{
    world::{
        liquid::corner_heights, to_global_pos, BlockData, BlockDirection, BlockId,
        BlockTransparency, Chunk, ChunkMap, SIX_OFFSETS,
    },
    CHUNK_SIZE,
};

use super::lighting::{Light, LightVolume, MAX_LIGHT};
use super::voxel::{BiomeTint, Face, FaceColor, FaceDirection, Sway, VoxelShape};

/// The buffers of a chunk mesh while its faces are added one by one. The normals, the tiles
/// and the lights are held in the formats of `NORMAL`, `TILE` and `LIGHT`, a fraction of
/// Bevy's: the meshes of a large render distance are hundreds of MiB to send to the GPU.
#[derive(Default)]
pub(crate) struct MeshCreator {
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
    normals: Vec<[i8; 4]>,
    uvs: Vec<[f32; 2]>,
    /// The block texture of each vertex, in `x`: its index in the table of `BlockLayers`. In
    /// `y`, how it is coloured and how it sways: see `colouring`.
    tiles: Vec<[u16; 2]>,
    lights: Vec<[u8; 4]>,
}

/// `Mesh::ATTRIBUTE_NORMAL`, each coordinate in a signed byte: the faces are square to the
/// axes, but for the tops of sloping liquids.
const NORMAL: MeshVertexAttribute = MeshVertexAttribute {
    format: VertexFormat::Snorm8x4,
    ..Mesh::ATTRIBUTE_NORMAL
};

/// `Mesh::ATTRIBUTE_UV_1`, the block texture in `x` and how it is coloured in `y`, as fractions
/// of `u16::MAX`, which `chunk.wgsl` multiplies back.
const TILE: MeshVertexAttribute = MeshVertexAttribute {
    format: VertexFormat::Unorm16x2,
    ..Mesh::ATTRIBUTE_UV_1
};

/// `Mesh::ATTRIBUTE_COLOR`, which holds how the vertex is lit, each in a byte: the light of the
/// sky, that of the blocks, both from 0 to `MAX_LIGHT`, and the ambient occlusion from 0, the
/// darkest, to 3, then an alpha of 1. The colour of a face is in `TILE`.
const LIGHT: MeshVertexAttribute = MeshVertexAttribute {
    format: VertexFormat::Unorm8x4,
    ..Mesh::ATTRIBUTE_COLOR
};

/// `LIGHT` of a vertex under the open sky, in no corner: that of an item.
const FULL_LIGHT: [u8; 4] = [u8::MAX, 0, u8::MAX, u8::MAX];

fn normal(normal: &[f32; 3]) -> [i8; 4] {
    let [x, y, z] = normal.map(|c| (c.clamp(-1., 1.) * 127.).round() as i8);
    [x, y, z, 0]
}

fn tile(tile: u32, colouring: u16) -> [u16; 2] {
    [tile as u16, colouring]
}

/// How far inside its face a UV stops, in blocks. The texture of a face repeats past 1, and must
/// not wrap around on the edges of the face, where the interpolated UVs may stray a little past
/// 0 or 1: the pixels of the other side of the texture would show there.
const UV_EDGE_INSET: f32 = 1. / 1024.;

/// How the vertices of a face are coloured and how they sway, as `chunk.wgsl` reads them.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Colouring {
    biome_tint: BiomeTint,
    color: FaceColor,
    sway: Sway,
    /// The block texture drawn over the face, if any.
    overlay: Option<u32>,
}

impl Colouring {
    fn of(face: &Face, overlay: Option<u32>) -> Self {
        Self {
            biome_tint: face.biome_tint,
            color: face.color,
            sway: face.sway,
            overlay,
        }
    }

    /// `UV_1.y` of the vertices: from the lowest bits, the colour map (`BiomeTint::index`) and
    /// the colour (`FaceColor::index`) in two bits each, the sway (`Sway::index`) in two more,
    /// and above them one more than the overlay, 0 for none. It fits, as the textures are at
    /// most `MAX_TILES`.
    fn code(self) -> u16 {
        self.biome_tint.index()
            | self.color.index() << 2
            | self.sway.index() << 4
            | self.overlay.map_or(0, |tile| tile as u16 + 1) << 6
    }
}

/// `uv` of a corner of a face moved towards the middle of the face, `centre`.
fn inset_uv(uv: [f32; 2], centre: [f32; 2]) -> [f32; 2] {
    [0, 1].map(|axis| uv[axis] + UV_EDGE_INSET.copysign(centre[axis] - uv[axis]))
}

impl MeshCreator {
    /// Appends `face` of a block at `origin` (in chunk coordinates) turned to `direction`,
    /// drawn with the block texture `tile`, and `overlay` over it if any, each vertex lit by
    /// `lights` (`LIGHT`). A square is split across the corners that are the most alike, so
    /// that the shade of a corner does not spill along the diagonal.
    fn push_face(
        &mut self,
        face: &Face,
        tile: u32,
        overlay: Option<u32>,
        origin: Vec3,
        direction: &BlockDirection,
        lights: &[[u8; 4]],
    ) {
        let first_index = self.vertices.len() as u32;
        match square_corners(&face.indices) {
            Some([a, b, c, d])
                if shade(lights[a]) + shade(lights[c]) < shade(lights[b]) + shade(lights[d]) =>
            {
                self.indices
                    .extend([b, c, d, d, a, b].map(|index| index as u32 + first_index));
            }
            _ => self
                .indices
                .extend(face.indices.iter().map(|index| index + first_index)),
        }
        self.vertices.extend(face.vertices.iter().map(|vertex| {
            let vertex = rotate_vertices(vertex, direction);
            [
                vertex[0] + origin.x,
                vertex[1] + origin.y,
                vertex[2] + origin.z,
            ]
        }));
        self.normals.extend(face.normals.iter().map(|n| {
            let [x, z] = direction.turn([n[0], n[2]]);
            normal(&[x, n[1], z])
        }));
        self.lights.extend_from_slice(lights);
        self.uvs
            .extend(face.uvs.iter().map(|uv| inset_uv(*uv, [0.5, 0.5])));
        let code = Colouring::of(face, overlay).code();
        self.tiles
            .extend(face.uvs.iter().map(|_| self::tile(tile, code)));
    }

    /// Appends the face `face_index` of `CUBE` stretched over `size` blocks along the face, from
    /// the block at `corner`: its texture repeats on each block.
    fn push_rectangle(&mut self, face_index: usize, corner: IVec3, size: Vec2, look: &FaceLook) {
        let face = &CUBE.faces[face_index];
        let [_, along_u, along_v] = CUBE.axes[face_index];
        let [origin, step_u, step_v] = CUBE.uv_steps[face_index];
        let centre = origin + (step_u * size.x + step_v * size.y) / 2.;

        let first_index = self.vertices.len() as u32;
        self.indices
            .extend(face.indices.iter().map(|index| index + first_index));
        for vertex in &face.vertices {
            let mut position = Vec3::from(*vertex);
            if position.y == 1. {
                position.y = look.height;
            }
            position[along_u] *= size.x;
            position[along_v] *= size.y;
            self.vertices.push((corner.as_vec3() + position).into());
            let uv = origin + step_u * vertex[along_u] * size.x + step_v * vertex[along_v] * size.y;
            self.uvs.push(inset_uv(uv.into(), centre.into()));
        }
        self.normals.extend(face.normals.iter().map(normal));
        self.lights.extend(face.vertices.iter().map(|_| look.light));
        let code = look.colouring.code();
        self.tiles
            .extend(face.vertices.iter().map(|_| tile(look.tile, code)));
    }

    /// Appends the faces of `other`, moved by `offset`.
    fn append(&mut self, other: MeshCreator, offset: Vec3) {
        let first_index = self.vertices.len() as u32;
        self.indices
            .extend(other.indices.iter().map(|index| index + first_index));
        self.vertices.extend(
            other
                .vertices
                .iter()
                .map(|vertex| <[f32; 3]>::from(Vec3::from(*vertex) + offset)),
        );
        self.normals.extend(other.normals);
        self.uvs.extend(other.uvs);
        self.tiles.extend(other.tiles);
        self.lights.extend(other.lights);
    }

    /// The mesh of the faces added so far, `None` when there is none. It has no tangents: the
    /// block materials have no normal map to use them. Its data goes to the GPU and leaves the
    /// main world, which never reads it: the meshes of a large render distance weigh hundreds of
    /// MiB, and copying them to the render world would stall the frames that show them.
    fn build(self) -> Option<Mesh> {
        if self.vertices.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.vertices);
        mesh.insert_attribute(NORMAL, VertexAttributeValues::Snorm8x4(self.normals));
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(TILE, VertexAttributeValues::Unorm16x2(self.tiles));
        mesh.insert_attribute(LIGHT, VertexAttributeValues::Unorm8x4(self.lights));
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
}

/// The mesh of `block` shown as an item lying on the ground: the faces of a whole block, one
/// block wide and centred on the origin, drawn with the chunk material. `None` for a block that
/// is not a whole cube, such as a flower, which is shown as a picture instead. An item keeps
/// its colour wherever it lies, and is lit as under the open sky.
pub(crate) fn block_item_mesh(block: BlockId, tiles: &HashMap<String, u32>) -> Option<Mesh> {
    let block = BlockData::new(block, BlockDirection::Front);
    let mut look = BlockLook::new(&block, tiles);
    if look
        .shape
        .faces
        .iter()
        .any(|face| matches!(face.direction, FaceDirection::Inset))
    {
        return None;
    }
    let mut mesh = MeshCreator::default();
    for ((face, tile), overlay) in look
        .shape
        .faces
        .iter_mut()
        .zip(&look.tiles)
        .zip(&look.overlays)
    {
        face.biome_tint = BiomeTint::None;
        face.sway = Sway::None;
        if let Some(tile) = tile {
            let lights = vec![FULL_LIGHT; face.vertices.len()];
            mesh.push_face(
                face,
                *tile,
                *overlay,
                Vec3::splat(-0.5),
                &block.direction,
                &lights,
            );
        }
    }
    mesh.build()
}

/// Cells on a side of a `Neighbourhood`: the chunk and one layer around it.
const SIDE: i32 = CHUNK_SIZE + 2;

/// The kind of a block and how it hides the faces next to it.
type Kind = (BlockId, BlockTransparency);

fn kind(block: &BlockData) -> Kind {
    (block.id, block.id.get_visibility())
}

/// The blocks a chunk mesh depends on: those of the chunk, and those of the neighbouring chunks
/// that touch its faces, by local position from -1 to `CHUNK_SIZE`. Reading one does not look
/// its chunk up.
struct Neighbourhood {
    cells: Vec<Option<Kind>>,
}

/// How far apart in a `Neighbourhood` the cells next to one another are, along x, y and z.
const STRIDES: [usize; 3] = [(SIDE * SIDE) as usize, SIDE as usize, 1];

impl Neighbourhood {
    fn new(world_map: &ChunkMap, chunk: &Chunk, chunk_pos: IVec3) -> Self {
        let mut cells = vec![None; (SIDE * SIDE * SIDE) as usize];
        // A row along z of the chunk is a row of the neighbourhood
        let size = CHUNK_SIZE as usize;
        let ours = chunk.map_cells(|block| block.map(kind));
        for (row, kinds) in ours.chunks_exact(size).enumerate() {
            let start = Self::index(IVec3::new((row / size) as i32, (row % size) as i32, 0));
            cells[start..start + size].copy_from_slice(kinds);
        }
        for offset in SIX_OFFSETS {
            let Some(neighbour) = world_map.chunk(chunk_pos + offset) else {
                continue;
            };
            let uniform = neighbour.uniform().map(|block| block.map(kind));
            let axis = offset.abs().max_position();
            // The layer of the neighbour against the chunk, and where it lies around the chunk
            let (theirs, ours) = if offset[axis] > 0 {
                (0, CHUNK_SIZE)
            } else {
                (CHUNK_SIZE - 1, -1)
            };
            for a in 0..CHUNK_SIZE {
                for b in 0..CHUNK_SIZE {
                    let mut local = IVec3::ZERO;
                    local[(axis + 1) % 3] = a;
                    local[(axis + 2) % 3] = b;
                    local[axis] = theirs;
                    let block = uniform.unwrap_or_else(|| neighbour.get(local).map(kind));
                    local[axis] = ours;
                    cells[Self::index(local)] = block;
                }
            }
        }
        Self { cells }
    }

    /// The block at a local position from -1 to `CHUNK_SIZE`: `None` for air, in a chunk that
    /// is not loaded, and on the edges and the corners around the chunk, which no face touches.
    fn get(&self, local: IVec3) -> Option<Kind> {
        self.cells[Self::index(local)]
    }

    fn index(local: IVec3) -> usize {
        let cell = local + IVec3::ONE;
        ((cell.x * SIDE + cell.y) * SIDE + cell.z) as usize
    }
}

/// How a kind of block is drawn: its shape, and the block texture of each of its faces, `None` for
/// a face that the texture pack cannot draw.
struct BlockLook {
    shape: VoxelShape,
    tiles: Vec<Option<u32>>,
    /// The block texture drawn over each face, if any.
    overlays: Vec<Option<u32>>,
}

impl BlockLook {
    fn new(block: &BlockData, tiles: &HashMap<String, u32>) -> Self {
        let mut shape = VoxelShape::create_from_block(block);
        // A texture pack without an overlay: the face is drawn without it
        let overlays = shape
            .faces
            .iter_mut()
            .map(|face| {
                let overlay = tiles.get(face.overlay.as_ref()?).copied();
                if overlay.is_none() {
                    face.remove_overlay();
                }
                overlay
            })
            .collect();
        // A texture pack without this texture nor a default one: nothing to draw
        let tiles = shape
            .faces
            .iter()
            .map(|face| {
                tiles
                    .get(&face.texture)
                    .or_else(|| tiles.get("_Default"))
                    .copied()
            })
            .collect();
        Self {
            shape,
            tiles,
            overlays,
        }
    }
}

/// The faces of a whole block, in the order of `VoxelShape::full_cube`: the faces of every
/// block that is a cube come in this order.
const CUBE_FACES: usize = 6;

/// The faces of a whole block, which merged faces stretch, with their axes.
struct Cube {
    faces: Vec<Face>,
    /// For each face, the axis across it, then the two axes along it, u and v.
    axes: [[usize; 3]; CUBE_FACES],
    /// For each face, its UV where u and v are 0, then how the UV changes along u and along v.
    uv_steps: [[Vec2; 3]; CUBE_FACES],
}

static CUBE: LazyLock<Cube> = LazyLock::new(|| {
    let faces = VoxelShape::full_cube(&BlockData::new(BlockId::Stone, BlockDirection::Front)).faces;
    let axes = std::array::from_fn(|index| {
        let across = Vec3::from(faces[index].normals[0]).abs().max_position();
        [across, (across + 1) % 3, (across + 2) % 3]
    });
    let uv_steps = std::array::from_fn(|index| {
        let [_, along_u, along_v] = axes[index];
        let face = &faces[index];
        let uv_at = |u, v| {
            let corner = face
                .vertices
                .iter()
                .position(|vertex| vertex[along_u] == u && vertex[along_v] == v);
            Vec2::from(face.uvs[corner.expect("a face is a unit square")])
        };
        let origin = uv_at(0., 0.);
        [origin, uv_at(1., 0.) - origin, uv_at(0., 1.) - origin]
    });
    Cube {
        faces,
        axes,
        uv_steps,
    }
});

/// What a face of a whole block looks like: neighbouring faces merge when theirs are equal.
#[derive(Clone, Copy)]
struct FaceLook {
    tile: u32,
    colouring: Colouring,
    liquid: bool,
    /// Where the top of the block is: below 1 under the surface of a liquid.
    height: f32,
    /// `LIGHT` of each of its corners, which are lit alike.
    light: [u8; 4],
}

/// Cells in a layer of a chunk.
const LAYER: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// The faces of whole blocks, waiting to be merged into rectangles: for each face of the cube,
/// the layers of the chunk across it, and in each layer what face lies on each cell.
struct FaceLayers {
    /// By face of the cube, layer, then cell `u * CHUNK_SIZE + v`: 0 for no face, else 1 + the
    /// index of the face in `looks`.
    cells: Vec<u32>,
    /// By face of the cube then layer, whether the layer has a face: most have none.
    used: [bool; CUBE_FACES * CHUNK_SIZE as usize],
    looks: Vec<FaceLook>,
    /// The index in `looks` of each look, by what it comes from.
    ids: bevy::platform::collections::HashMap<(BlockId, usize, u32, [u8; 4]), u32>,
}

impl FaceLayers {
    fn new() -> Self {
        Self {
            cells: vec![0; CUBE_FACES * CHUNK_SIZE as usize * LAYER],
            used: [false; CUBE_FACES * CHUNK_SIZE as usize],
            looks: Vec::new(),
            ids: default(),
        }
    }

    /// Lays the face `face_index` of the block at `local`, which looks like `look` as all the
    /// faces of `block_id` that are at that index, that `height` and that light do.
    fn add(&mut self, face_index: usize, local: IVec3, block_id: BlockId, look: FaceLook) {
        let next_id = self.looks.len() as u32 + 1;
        let id = *self
            .ids
            .entry((block_id, face_index, look.height.to_bits(), look.light))
            .or_insert_with(|| {
                self.looks.push(look);
                next_id
            });
        let [across, along_u, along_v] = CUBE.axes[face_index].map(|axis| local[axis] as usize);
        let size = CHUNK_SIZE as usize;
        self.cells[(face_index * size + across) * LAYER + along_u * size + along_v] = id;
        self.used[face_index * size + across] = true;
    }

    /// Merges the faces of each layer into rectangles of one look, and appends them to the
    /// solid or the liquid faces. A rectangle grows along v first, then along u as long as the
    /// next row matches.
    fn merge_into(mut self, solid: &mut MeshCreator, liquid: &mut MeshCreator) {
        let size = CHUNK_SIZE as usize;
        for (face_index, &[axis, along_u, along_v]) in CUBE.axes.iter().enumerate() {
            for across in 0..size {
                if !self.used[face_index * size + across] {
                    continue;
                }
                let start = (face_index * size + across) * LAYER;
                let cells = &mut self.cells[start..start + LAYER];
                for u in 0..size {
                    let mut v = 0;
                    while v < size {
                        let id = cells[u * size + v];
                        if id == 0 {
                            v += 1;
                            continue;
                        }
                        let mut length = 1;
                        while v + length < size && cells[u * size + v + length] == id {
                            length += 1;
                        }
                        let row = |u: usize| u * size + v..u * size + v + length;
                        let mut width = 1;
                        while u + width < size && cells[row(u + width)].iter().all(|c| *c == id) {
                            width += 1;
                        }
                        for row_u in u..u + width {
                            cells[row(row_u)].fill(0);
                        }

                        let look = &self.looks[id as usize - 1];
                        let mut corner = IVec3::ZERO;
                        corner[axis] = across as i32;
                        corner[along_u] = u as i32;
                        corner[along_v] = v as i32;
                        let mesh_creator = if look.liquid {
                            &mut *liquid
                        } else {
                            &mut *solid
                        };
                        let size = Vec2::new(width as f32, length as f32);
                        mesh_creator.push_rectangle(face_index, corner, size, look);
                        v += length;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ChunkMeshResponse {
    pub solid_mesh: Option<Mesh>,
    /// The faces of the liquids, drawn apart with a blended material.
    pub liquid_mesh: Option<Mesh>,
}

/// How the faces are lit: by the light of the cells in front of them, from `volume`, which
/// holds those of the chunks meshed.
#[derive(Clone, Copy)]
pub(crate) struct Lighting<'a> {
    pub volume: &'a LightVolume,
    /// Whether each corner of a face takes the mean light of the four cells in front of it,
    /// darkened by the blocks next to them (ambient occlusion), as Minecraft's smooth lighting.
    /// Otherwise a face takes the light of the cell in front of it.
    pub smooth: bool,
}

/// The mesh of `chunk`: the faces of its blocks that are not hidden by a neighbouring block,
/// in this chunk or in the ones around it. `tiles` gives the index of each block texture. Every
/// face is lit as under the open sky.
///
/// Neighbouring faces of whole blocks that look the same are merged into one rectangle (greedy
/// meshing), which keeps the meshes far smaller than one square per face.
#[cfg(test)]
pub(crate) fn generate_chunk_mesh(
    world_map: &ChunkMap,
    chunk: &Chunk,
    chunk_pos: &IVec3,
    tiles: &HashMap<String, u32>,
) -> ChunkMeshResponse {
    let lighting = Lighting {
        volume: &LightVolume::open_sky(),
        smooth: true,
    };
    mesh_chunk(world_map, chunk, chunk_pos, tiles, lighting, true)
}

/// `generate_chunk_mesh`, with the faces lit smoothly by the light of the chunk and of those
/// around it, where what is not loaded is air.
#[cfg(test)]
pub(crate) fn generate_lit_chunk_mesh(
    world_map: &ChunkMap,
    chunk: &Chunk,
    chunk_pos: &IVec3,
    tiles: &HashMap<String, u32>,
) -> ChunkMeshResponse {
    let prior = &mut super::lighting::TerrainPrior::new(None);
    let volume = LightVolume::of_chunks(world_map, &[*chunk_pos], prior);
    let lighting = Lighting {
        volume: &volume,
        smooth: true,
    };
    mesh_chunk(world_map, chunk, chunk_pos, tiles, lighting, true)
}

/// The faces of a chunk, solid and liquid, before they are drawn with those of its group.
pub(crate) struct ChunkFaces(MeshCreator, MeshCreator);

/// The faces of the chunk at `chunk_pos` that `generate_chunk_mesh` draws, lit by `lighting`,
/// `None` when it is not loaded.
pub(crate) fn chunk_faces_at(
    world_map: &ChunkMap,
    chunk_pos: IVec3,
    tiles: &HashMap<String, u32>,
    lighting: Lighting,
) -> Option<ChunkFaces> {
    let chunk = world_map.chunk(chunk_pos)?;
    let (solid, liquid) = chunk_faces(world_map, chunk, &chunk_pos, tiles, lighting, true);
    Some(ChunkFaces(solid, liquid))
}

/// The meshes of the faces of `chunks`, each made by `chunk_faces_at` for the chunk at its
/// position, drawn as one: a vertex is placed from the block `origin`.
pub(crate) fn group_mesh(
    chunks: impl IntoIterator<Item = (IVec3, ChunkFaces)>,
    origin: IVec3,
) -> ChunkMeshResponse {
    let mut solid = MeshCreator::default();
    let mut liquid = MeshCreator::default();
    for (chunk_pos, ChunkFaces(chunk_solid, chunk_liquid)) in chunks {
        let offset = (chunk_pos * CHUNK_SIZE - origin).as_vec3();
        solid.append(chunk_solid, offset);
        liquid.append(chunk_liquid, offset);
    }
    ChunkMeshResponse {
        solid_mesh: solid.build(),
        liquid_mesh: liquid.build(),
    }
}

/// `generate_chunk_mesh`, lit by `lighting`, with the faces merged or not.
#[cfg(test)]
fn mesh_chunk(
    world_map: &ChunkMap,
    chunk: &Chunk,
    chunk_pos: &IVec3,
    tiles: &HashMap<String, u32>,
    lighting: Lighting,
    merge: bool,
) -> ChunkMeshResponse {
    let start = std::time::Instant::now();
    let (solid, liquid) = chunk_faces(world_map, chunk, chunk_pos, tiles, lighting, merge);
    let response = ChunkMeshResponse {
        solid_mesh: solid.build(),
        liquid_mesh: liquid.build(),
    };
    trace!("Render time : {:?}", start.elapsed());
    response
}

/// The solid and the liquid faces of `chunk` that `mesh_chunk` draws, lit by `lighting`.
fn chunk_faces(
    world_map: &ChunkMap,
    chunk: &Chunk,
    chunk_pos: &IVec3,
    tiles: &HashMap<String, u32>,
    lighting: Lighting,
    merge: bool,
) -> (MeshCreator, MeshCreator) {
    let blocks = Neighbourhood::new(world_map, chunk, *chunk_pos);
    let mut looks: bevy::platform::collections::HashMap<BlockId, BlockLook> = default();
    let mut lowered_shapes: bevy::platform::collections::HashMap<(BlockId, [u32; 4]), VoxelShape> =
        default();
    let mut solid = MeshCreator::default();
    let mut liquid = MeshCreator::default();
    let mut layers = FaceLayers::new();
    let mut lights = Vec::new();

    for local_block_pos in chunk_positions() {
        let index = Neighbourhood::index(local_block_pos);
        let Some(kind) = blocks.cells[index] else {
            continue;
        };
        if is_block_surrounded(&blocks, index, kind) {
            continue;
        }
        let Some(block) = chunk.get(local_block_pos) else {
            continue;
        };
        let visibility = kind.1;

        let origin = local_block_pos.as_vec3();
        // How much a liquid is seen through is the alpha of its texture
        let mesh_creator = match visibility {
            BlockTransparency::Liquid => &mut liquid,
            _ => &mut solid,
        };
        // The cracks of a block being broken (stages 0 to 9) are drawn over each of its
        // faces. A custom texture folder may lack the texture: no cracks then.
        let cracks = (block.breaking_progress > 0)
            .then(|| tiles.get(&format!("DestroyStage{}", block.get_breaking_level())))
            .flatten();

        let look = looks
            .entry(block.id)
            .or_insert_with(|| BlockLook::new(block, tiles));
        let corners = (visibility == BlockTransparency::Liquid)
            .then(|| corner_heights(world_map, to_global_pos(chunk_pos, &local_block_pos)))
            .flatten();
        // Most of the surface of a liquid is level: its blocks share a few shapes
        let shape = match corners {
            Some(corners) => &*lowered_shapes
                .entry((block.id, corners.map(f32::to_bits)))
                .or_insert_with(|| look.shape.clone().with_top_corners(corners)),
            None => &look.shape,
        };
        // A face that is not drawn the same everywhere stays one square, as do the faces of
        // the shapes that are not whole blocks
        let turned = block.direction != BlockDirection::Front;
        let mergeable = merge && !turned && cracks.is_none();
        let block_pos = to_global_pos(chunk_pos, &local_block_pos);
        // Where the top of the block is when it is level, `None` on a sloping liquid
        let level_top = match corners {
            Some(corners) => corners
                .iter()
                .all(|c| *c == corners[0])
                .then_some(corners[0]),
            None => Some(1.),
        };
        let faces = shape.faces.iter().zip(&look.tiles).zip(&look.overlays);
        for (face_index, ((face, tile), overlay)) in faces.enumerate() {
            let Some(tile) = *tile else {
                continue;
            };
            if !should_render_face(&blocks, local_block_pos, &face.direction, &visibility) {
                continue;
            }
            lighting.light_face(face, block_pos, turned, &mut lights);
            // A swaying face moves at each of its corners: it cannot stretch over several
            let lit_alike = face.sway == Sway::None && lights.iter().all(|l| *l == lights[0]);
            let (merges, face_height) = match face.direction {
                FaceDirection::Inset => (false, 1.),
                FaceDirection::Top => (mergeable && level_top.is_some(), level_top.unwrap_or(1.)),
                FaceDirection::Bottom => (mergeable, 1.),
                // Below the surface of a liquid, a side is shorter than a block
                _ => (mergeable && level_top == Some(1.), 1.),
            };
            if merges && lit_alike {
                let look = FaceLook {
                    tile,
                    colouring: Colouring::of(face, *overlay),
                    liquid: visibility == BlockTransparency::Liquid,
                    height: face_height,
                    light: lights[0],
                };
                layers.add(face_index, local_block_pos, block.id, look);
                continue;
            }
            mesh_creator.push_face(face, tile, *overlay, origin, &block.direction, &lights);
            if let Some(cracks) = cracks {
                mesh_creator.push_face(face, *cracks, None, origin, &block.direction, &lights);
            }
        }
    }

    layers.merge_into(&mut solid, &mut liquid);
    (solid, liquid)
}

/// The four corners of a square split in two triangles by `indices`, in the order they go
/// round, from the first corner of the first triangle: the first and the third are those it is
/// split across. `None` for another shape.
fn square_corners(indices: &[u32]) -> Option<[usize; 4]> {
    let [a, b, c, d, e, f] = *indices else {
        return None;
    };
    // The second triangle shares the side from c to a, and brings the fourth corner
    let second = [d, e, f];
    let fourth = second.into_iter().find(|i| ![a, b, c].contains(i))?;
    let at = second.iter().position(|i| *i == fourth)?;
    (second[(at + 1) % 3] == a && second[(at + 2) % 3] == c)
        .then_some([a, b, c, fourth].map(|i| i as usize))
}

/// How bright a corner looks, to split a square across the corners most alike.
fn shade(light: [u8; 4]) -> u32 {
    let [sky, block, occlusion, _] = light.map(u32::from);
    occlusion * 2 + sky.max(block)
}

/// `LIGHT` of `light`, the sums of the light of the sky and of the blocks of `count` cells, in
/// a corner of ambient occlusion `occlusion`.
fn light_bytes(light: [u32; 2], count: u32, occlusion: u8) -> [u8; 4] {
    let full = count * u32::from(MAX_LIGHT);
    let [sky, block] = light.map(|sum| ((sum * 255 + full / 2) / full) as u8);
    [sky, block, occlusion * 85, u8::MAX]
}

/// The side of a block a face of it lies on, `None` for a face inside it.
fn face_normal(direction: &FaceDirection) -> Option<IVec3> {
    Some(match direction {
        FaceDirection::Top => IVec3::Y,
        FaceDirection::Bottom => IVec3::NEG_Y,
        FaceDirection::Front => IVec3::NEG_Z,
        FaceDirection::Back => IVec3::Z,
        FaceDirection::Left => IVec3::NEG_X,
        FaceDirection::Right => IVec3::X,
        FaceDirection::Inset => return None,
    })
}

impl Lighting<'_> {
    /// Writes into `lights` the `LIGHT` of each vertex of `face`, of the block at `block` in the
    /// world, which is `turned` away from the front. A face that covers a side of the block is
    /// lit by the cells in front of it; one inside the block, sunk under a liquid or of a
    /// turned block, as a whole by the brightest of its own cell and the one it looks to.
    fn light_face(&self, face: &Face, block: IVec3, turned: bool, lights: &mut Vec<[u8; 4]>) {
        lights.clear();
        let normal = face_normal(&face.direction);
        let covers_side = normal.filter(|normal| {
            !turned && face.vertices.iter().all(|vertex| on_side(vertex, *normal))
        });
        match covers_side {
            Some(normal) if self.smooth => {
                let front = self.front_cells(block + normal, normal);
                lights.extend(
                    face.vertices
                        .iter()
                        .map(|vertex| corner(&front, normal, vertex)),
                );
            }
            Some(normal) => lights.resize(face.vertices.len(), self.brightest(&[block + normal])),
            None => {
                let ahead = block + normal.unwrap_or_default();
                lights.resize(face.vertices.len(), self.brightest(&[block, ahead]));
            }
        }
    }

    /// `LIGHT` of the brightest of `cells` that let light through, in no corner.
    fn brightest(&self, cells: &[IVec3]) -> [u8; 4] {
        let mut light = [0, 0];
        for cell in cells {
            if !self.volume.is_opaque(*cell) {
                let lit = self.volume.light(*cell);
                light = [light[0].max(lit.sky), light[1].max(lit.block)];
            }
        }
        light_bytes(light.map(u32::from), 1, 3)
    }

    /// The cells of the plane of the cell `front` across `normal`, the face of a block looks to:
    /// by step along the two other axes, in their order after the axis of `normal`, from -1.
    fn front_cells(&self, front: IVec3, normal: IVec3) -> FrontCells {
        let across = normal.abs().max_position();
        let (axis_a, axis_b) = ((across + 1) % 3, (across + 2) % 3);
        std::array::from_fn(|a| {
            std::array::from_fn(|b| {
                let mut cell = front;
                cell[axis_a] += a as i32 - 1;
                cell[axis_b] += b as i32 - 1;
                self.volume.cell(cell)
            })
        })
    }
}

/// The light of the cells of a plane in front of a face, and whether each is opaque, as
/// `Lighting::front_cells` reads them.
type FrontCells = [[(Light, bool); 3]; 3];

/// `LIGHT` of the corner `vertex` of a face that looks along `normal` to the cells `front`:
/// the mean light of the four cells around the corner in front of the face, but for those no
/// light goes through, which shade it instead (ambient occlusion). With both cells along the
/// sides blocked, the one across the corner is hidden.
fn corner(front: &FrontCells, normal: IVec3, vertex: &[f32; 3]) -> [u8; 4] {
    let across = normal.abs().max_position();
    let toward = |axis: usize| if vertex[axis] > 0.5 { 2 } else { 0 };
    let (a, b) = (toward((across + 1) % 3), toward((across + 2) % 3));
    let side_a = front[a][1].1;
    let side_b = front[1][b].1;
    let diagonal = (side_a && side_b) || front[a][b].1;
    let occlusion = if side_a && side_b {
        0
    } else {
        3 - side_a as u8 - side_b as u8 - diagonal as u8
    };
    let mut light = [0, 0];
    let mut count = 0;
    for (lit, blocked) in [
        (front[1][1].0, false),
        (front[a][1].0, side_a),
        (front[1][b].0, side_b),
        (front[a][b].0, diagonal),
    ] {
        if !blocked {
            light[0] += u32::from(lit.sky);
            light[1] += u32::from(lit.block);
            count += 1;
        }
    }
    light_bytes(light, count, occlusion)
}

/// Whether `vertex`, in the unit cube of a block, is a corner of the side `normal` looks to.
fn on_side(vertex: &[f32; 3], normal: IVec3) -> bool {
    (0..3).all(|axis| match normal[axis] {
        1 => vertex[axis] == 1.,
        -1 => vertex[axis] == 0.,
        _ => vertex[axis] == 0. || vertex[axis] == 1.,
    })
}

/// The local positions of a chunk, in the order of their indices in the chunk.
fn chunk_positions() -> impl Iterator<Item = IVec3> {
    (0..CHUNK_SIZE).flat_map(|x| {
        (0..CHUNK_SIZE).flat_map(move |y| (0..CHUNK_SIZE).map(move |z| IVec3::new(x, y, z)))
    })
}

/// Whether every face of the block of `kind` at the index `index` of `blocks` is hidden by the
/// block next to it.
fn is_block_surrounded(
    blocks: &Neighbourhood,
    index: usize,
    (block_id, block_visibility): Kind,
) -> bool {
    for stride in STRIDES {
        for neighbour in [index - stride, index + stride] {
            let hides = match blocks.cells[neighbour] {
                Some((_, BlockTransparency::Solid)) => true,
                Some((_, BlockTransparency::Decoration)) | None => false,
                Some((_, BlockTransparency::Liquid)) => {
                    block_visibility == BlockTransparency::Liquid
                }
                Some((id, BlockTransparency::Transparent)) => block_id == id,
            };
            if !hides {
                return false;
            }
        }
    }
    true
}

/// `v`, a point of a shape in its cell, turned to `direction` about the vertical axis through
/// the centre of the cell: `Right` turns the front of the shape, toward -Z, toward +X.
pub fn rotate_vertices(v: &[f32; 3], direction: &BlockDirection) -> [f32; 3] {
    let [x, z] = direction.turn([v[0] - 0.5, v[2] - 0.5]);
    [x + 0.5, v[1], z + 0.5]
}

fn should_render_face(
    blocks: &Neighbourhood,
    local_block_pos: IVec3,
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

    if let Some((_, vis)) = blocks.get(local_block_pos + offset) {
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
    use shared::world::VINE_GAP;

    fn cracked_stone_mesh(tiles: &HashMap<String, u32>, progress: u8) -> usize {
        let mut chunk = Chunk::new();
        let mut block = BlockData::new(BlockId::Stone, BlockDirection::Front);
        block.breaking_progress = progress;
        chunk.set(IVec3::new(1, 1, 1), block);
        let mut world = ChunkMap::default();
        world.insert_chunk(IVec3::ZERO, chunk.clone());
        generate_chunk_mesh(&world, &chunk, &IVec3::ZERO, tiles)
            .solid_mesh
            .map_or(0, |mesh| mesh.count_vertices())
    }

    #[test]
    fn a_missing_cracks_texture_draws_no_cracks_instead_of_panicking() {
        let mut tiles = HashMap::new();
        tiles.insert("_Default".to_string(), 0);
        let plain = cracked_stone_mesh(&tiles, 0);
        // Mid-way, last hit and beyond the break time: the block is drawn without cracks
        for progress in [30, 60, 200] {
            assert_eq!(cracked_stone_mesh(&tiles, progress), plain, "{progress}");
        }

        tiles.insert("DestroyStage9".to_string(), 1);
        assert!(cracked_stone_mesh(&tiles, 200) > plain);
    }

    fn mesh_of(blocks: &[(IVec3, BlockId)]) -> ChunkMeshResponse {
        let mut chunk = Chunk::new();
        for (position, id) in blocks {
            chunk.set(*position, BlockData::new(*id, BlockDirection::Front));
        }
        let mut world = ChunkMap::default();
        world.insert_chunk(IVec3::ZERO, chunk.clone());
        let mut tiles = HashMap::new();
        tiles.insert("_Default".to_string(), 0);
        generate_chunk_mesh(&world, &chunk, &IVec3::ZERO, &tiles)
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

    /// The normal of each vertex of `mesh`.
    fn normals_of(mesh: &Mesh) -> Vec<Vec3> {
        let Some(VertexAttributeValues::Snorm8x4(normals)) = mesh.attribute(NORMAL) else {
            panic!("a chunk mesh has normals");
        };
        normals
            .iter()
            .map(|n| Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32) / 127.)
            .collect()
    }

    /// The block texture of each vertex of `mesh`.
    fn tiles_of(mesh: &Mesh) -> Vec<u32> {
        let Some(VertexAttributeValues::Unorm16x2(tiles)) = mesh.attribute(TILE) else {
            panic!("a chunk mesh has tiles");
        };
        tiles.iter().map(|tile| tile[0] as u32).collect()
    }

    /// The light of each vertex of `mesh`.
    fn lights_of(mesh: &Mesh) -> Vec<[u8; 4]> {
        let Some(VertexAttributeValues::Unorm8x4(colors)) = mesh.attribute(LIGHT) else {
            panic!("a chunk mesh has lights");
        };
        colors.clone()
    }

    /// The positions of the vertices of the faces of `mesh` that face up, level or sloping.
    fn top_vertices(mesh: &Option<Mesh>) -> Vec<Vec3> {
        use bevy::mesh::VertexAttributeValues::Float32x3;
        let Some(mesh) = mesh else {
            return Vec::new();
        };
        let Some(Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("a chunk mesh has positions");
        };
        positions
            .iter()
            .zip(normals_of(mesh))
            .filter(|(_, normal)| normal[1] > 0.5)
            .map(|(position, _)| Vec3::from(*position))
            .collect()
    }

    /// `blocks`, each walled with stone on its four sides.
    fn walled(blocks: &[(IVec3, BlockId)]) -> Vec<(IVec3, BlockId)> {
        let walls = blocks.iter().flat_map(|(position, _)| {
            [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                .map(|side| *position + side)
                .into_iter()
                .filter(|wall| blocks.iter().all(|(position, _)| position != wall))
                .map(|wall| (wall, BlockId::Stone))
        });
        blocks.iter().copied().chain(walls).collect()
    }

    #[test]
    fn the_surface_of_the_water_is_below_the_top_of_its_cell() {
        let highest = |response: ChunkMeshResponse| {
            top_vertices(&response.liquid_mesh)
                .iter()
                .map(|p| p.y)
                .fold(f32::MIN, f32::max)
        };
        assert_eq!(
            highest(mesh_of(&walled(&[(IVec3::ONE, BlockId::Water)]))),
            1.0 + 8.0 / 9.0
        );
        // Under more water, a block fills its cell
        let column = mesh_of(&walled(&[
            (IVec3::ONE, BlockId::Water),
            (IVec3::new(1, 2, 1), BlockId::Water),
        ]));
        assert_eq!(highest(column), 2.0 + 8.0 / 9.0);
        // In the open, the surface goes down towards the air on every side
        assert_eq!(
            highest(mesh_of(&[(IVec3::ONE, BlockId::Water)])),
            1.0 + 80.0 / 117.0
        );
    }

    #[test]
    fn the_surface_of_a_flow_has_no_steps() {
        let mut chunk = Chunk::new();
        for x in 0..8 {
            for z in 0..8 {
                chunk.set(
                    IVec3::new(x, 0, z),
                    BlockData::new(BlockId::Stone, BlockDirection::Front),
                );
            }
        }
        // A source and the flow around it, as the server spreads it, down to the air
        let source = IVec3::new(3, 1, 3);
        for x in 1..6 {
            for z in 1..6 {
                let pos = IVec3::new(x, 1, z);
                let distance = (pos - source).abs();
                chunk.set(
                    pos,
                    BlockData {
                        level: (distance.x + distance.z) as u8,
                        ..BlockData::new(BlockId::Water, BlockDirection::Front)
                    },
                );
            }
        }
        let mut world = ChunkMap::default();
        world.insert_chunk(IVec3::ZERO, chunk.clone());
        let tiles = HashMap::from([("_Default".to_string(), 0)]);
        let water = generate_chunk_mesh(&world, &chunk, &IVec3::ZERO, &tiles);

        let mut heights: HashMap<[i32; 2], f32> = HashMap::new();
        let tops = top_vertices(&water.liquid_mesh);
        assert_eq!(
            tops.len(),
            4 * 25,
            "one square for each cell, as none is level"
        );
        for vertex in tops {
            let corner = [vertex.x as i32, vertex.z as i32];
            let height = *heights.entry(corner).or_insert(vertex.y);
            assert_eq!(height, vertex.y, "two heights at {corner:?}");
        }
        assert!(heights[&[3, 3]] > heights[&[1, 1]]);
    }

    #[test]
    fn the_faces_between_two_water_blocks_are_hidden() {
        let two = mesh_of(&[
            (IVec3::ONE, BlockId::Water),
            (IVec3::new(2, 1, 1), BlockId::Water),
        ]);
        let cells = drawn_cells(&two.liquid_mesh);
        // Eight faces across an axis: the two cubes less the two faces they share, at x = 2,
        // and their tops, which slope down to the air
        assert_eq!(cells.values().sum::<usize>(), 8);
        assert_eq!(top_vertices(&two.liquid_mesh).len(), 2 * 4);
        assert!(cells
            .keys()
            .all(|cell| cell.normal[0] == 0 || cell.plane != 2 * 9));
    }

    /// What a face draws on a block it covers, in a plane across an axis.
    #[derive(Hash, PartialEq, Eq, Debug)]
    struct DrawnCell {
        normal: [i32; 3],
        /// Where the plane is along the normal, in ninths of a block: liquids end at ninths.
        plane: i32,
        /// The block covered in the plane, along the two other axes.
        cell: [i32; 2],
        tile: u32,
        color: [u8; 4],
        /// The texel of a 16 by 16 texture drawn at a point of the block.
        texel: [i32; 2],
    }

    /// The cells drawn by the faces of `mesh` across an axis, each with how many faces draw it.
    /// A point of each block is looked up in the triangles: it never lies on their edges.
    fn drawn_cells(mesh: &Option<Mesh>) -> HashMap<DrawnCell, usize> {
        use bevy::mesh::VertexAttributeValues::{Float32x2, Float32x3};
        let mut cells = HashMap::new();
        let Some(mesh) = mesh else {
            return cells;
        };
        let (Some(Float32x3(positions)), Some(Float32x2(uvs)), Some(Indices::U32(indices))) = (
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
            mesh.indices(),
        ) else {
            panic!("a chunk mesh has positions, UVs and indices");
        };
        let (normals, tiles, colors) = (normals_of(mesh), tiles_of(mesh), lights_of(mesh));
        for triangle in indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| triangle[k] as usize);
            let normal = normals[a];
            let across = normal.abs().max_position();
            if normal.abs().cmpgt(Vec3::ZERO).bitmask().count_ones() != 1 {
                // The crossed squares of the plants are never merged
                continue;
            }
            let (along_u, along_v) = ((across + 1) % 3, (across + 2) % 3);
            let corners =
                [a, b, c].map(|k| Vec2::new(positions[k][along_u], positions[k][along_v]));
            let low = corners[0]
                .min(corners[1])
                .min(corners[2])
                .floor()
                .as_ivec2();
            let high = corners[0].max(corners[1]).max(corners[2]).ceil().as_ivec2();
            for u in low.x..high.x {
                for v in low.y..high.y {
                    let point = Vec2::new(u as f32 + 0.3141, v as f32 + 0.2718);
                    let Some(weights) = barycentric(corners, point) else {
                        continue;
                    };
                    let uv = weights[0] * Vec2::from(uvs[a])
                        + weights[1] * Vec2::from(uvs[b])
                        + weights[2] * Vec2::from(uvs[c]);
                    let texel = (uv.fract_gl() * 16.).floor().as_ivec2();
                    let cell = DrawnCell {
                        normal: normal.round().as_ivec3().into(),
                        plane: (positions[a][across] * 9.).round() as i32,
                        cell: [u, v],
                        tile: tiles[a],
                        color: colors[a],
                        texel: texel.into(),
                    };
                    *cells.entry(cell).or_default() += 1;
                }
            }
        }
        cells
    }

    /// The weights of the corners of a triangle at `point`, `None` outside it.
    fn barycentric([a, b, c]: [Vec2; 3], point: Vec2) -> Option<[f32; 3]> {
        let area = (b - a).perp_dot(c - a);
        let weight_b = (point - a).perp_dot(c - a) / area;
        let weight_c = (b - a).perp_dot(point - a) / area;
        let weight_a = 1. - weight_b - weight_c;
        [weight_a, weight_b, weight_c]
            .iter()
            .all(|weight| *weight >= 0.)
            .then_some([weight_a, weight_b, weight_c])
    }

    #[test]
    fn merged_faces_draw_the_same_texels_on_the_same_blocks_as_one_square_per_face() {
        let tiles: HashMap<String, u32> = ["_Default", "Grass", "GrassTop", "OakLog", "OakLogTop"]
            .into_iter()
            .chain((0..10).map(|_| "DestroyStage"))
            .enumerate()
            .map(|(index, name)| match name {
                "DestroyStage" => (format!("DestroyStage{}", index - 5), index as u32),
                _ => (name.to_string(), index as u32),
            })
            .collect();
        let mut water = 0;
        // A snow plain, then the shore of a lake, with its water
        for center in [IVec3::new(0, 4, 0), IVec3::new(6, 3, 6)] {
            let mut world = ChunkMap::default();
            for chunk_pos in std::iter::once(center).chain(SIX_OFFSETS.map(|o| center + o)) {
                world.insert_chunk(chunk_pos, server::generate_chunk(chunk_pos, 42));
            }
            // A block being broken, and a turned one: neither is merged
            let surface = |x, z| {
                let column = center * CHUNK_SIZE + IVec3::new(x, 0, z);
                (0..CHUNK_SIZE)
                    .rev()
                    .map(|y| column.with_y(column.y + y))
                    .find(|pos| world.get_block(*pos).is_some())
                    .expect("the chunk has ground")
            };
            let (cracked, turned) = (surface(5, 5), surface(9, 3));
            let mut block = *world.get_block(cracked).unwrap();
            block.breaking_progress = 30;
            world.set_block(cracked, block);
            let mut block = *world.get_block(turned).unwrap();
            block.direction = BlockDirection::Right;
            world.set_block(turned, block);

            let chunk = world.chunk(center).unwrap();
            let prior = &mut crate::world::lighting::TerrainPrior::new(None);
            let volume = LightVolume::of_chunks(&world, &[center], prior);
            let lighting = Lighting {
                volume: &volume,
                smooth: true,
            };
            let merged = mesh_chunk(&world, chunk, &center, &tiles, lighting, true);
            let squares = mesh_chunk(&world, chunk, &center, &tiles, lighting, false);
            assert!(vertices(&merged.solid_mesh) < vertices(&squares.solid_mesh));
            water += vertices(&squares.liquid_mesh);
            for (merged, squares) in [
                (&merged.solid_mesh, &squares.solid_mesh),
                (&merged.liquid_mesh, &squares.liquid_mesh),
            ] {
                assert_eq!(drawn_cells(merged), drawn_cells(squares), "{center}");
            }
        }
        assert!(water > 0, "the shore has water");
    }

    #[test]
    fn a_floor_of_one_block_is_one_rectangle_on_each_side() {
        let floor: Vec<_> = (0..CHUNK_SIZE)
            .flat_map(|x| (0..CHUNK_SIZE).map(move |z| (IVec3::new(x, 0, z), BlockId::Stone)))
            .collect();
        let mesh = mesh_of(&floor).solid_mesh;
        assert_eq!(vertices(&mesh), 6 * 4);
        // The texture repeats on each block: across the top, the UVs go from 0 to 16
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
            mesh.unwrap().attribute(Mesh::ATTRIBUTE_UV_0).cloned()
        else {
            panic!("no UVs");
        };
        let top = &uvs[..4];
        let span = |axis: usize| {
            let values = top.iter().map(|uv| uv[axis]);
            values.clone().fold(f32::MIN, f32::max) - values.fold(f32::MAX, f32::min)
        };
        assert!((span(0) - 16.).abs() < 0.01 && (span(1) - 16.).abs() < 0.01);
    }

    #[test]
    fn only_faces_that_look_the_same_merge() {
        // Stone, stone, dirt: two rectangles on top
        let row = mesh_of(&[
            (IVec3::new(0, 0, 0), BlockId::Stone),
            (IVec3::new(1, 0, 0), BlockId::Stone),
            (IVec3::new(2, 0, 0), BlockId::Dirt),
        ]);
        let tops = |mesh: &Option<Mesh>| {
            drawn_cells(mesh)
                .keys()
                .filter(|cell| cell.normal == [0, 1, 0])
                .count()
        };
        assert_eq!(tops(&row.solid_mesh), 3);
        assert_eq!(vertices(&row.solid_mesh), 4 * (2 + 2 + 2 + 2 + 2));

        // A level pool of water is one rectangle on top, but a sloping surface is not merged
        let pool = [IVec3::new(1, 1, 1), IVec3::new(2, 1, 1)].map(|pos| (pos, BlockId::Water));
        assert_eq!(top_vertices(&mesh_of(&walled(&pool)).liquid_mesh).len(), 4);
        assert_eq!(top_vertices(&mesh_of(&pool).liquid_mesh).len(), 2 * 4);
    }

    #[test]
    fn a_block_against_a_block_of_a_neighbouring_chunk_hides_that_face_on_every_side() {
        let stone = BlockData::new(BlockId::Stone, BlockDirection::Front);
        let mut tiles = HashMap::new();
        tiles.insert("_Default".to_string(), 0);
        for offset in shared::world::SIX_OFFSETS {
            // A block on the face of the chunk turned to `offset`, and one beyond it
            let block = IVec3::splat(CHUNK_SIZE / 2) + offset * (CHUNK_SIZE / 2);
            let block = block.min(IVec3::splat(CHUNK_SIZE - 1));
            let mut world = ChunkMap::default();
            world.insert_chunk(IVec3::ZERO, Chunk::new());
            world.insert_chunk(offset, Chunk::new());
            world.set_block(block, stone);
            world.set_block(block + offset, stone);

            let chunk = world.chunk(IVec3::ZERO).unwrap();
            let mesh = generate_chunk_mesh(&world, chunk, &IVec3::ZERO, &tiles);
            assert_eq!(vertices(&mesh.solid_mesh), 5 * 4, "{offset}");
        }
    }

    /// The `UV_1` and normal of each vertex of the mesh of a lone grass block, drawn with
    /// `tiles`.
    fn grass_block_tiles(tiles: &[(&str, u32)]) -> Vec<([u16; 2], Vec3)> {
        let mut chunk = Chunk::new();
        chunk.set(
            IVec3::ONE,
            BlockData::new(BlockId::Grass, BlockDirection::Front),
        );
        let mut world = ChunkMap::default();
        world.insert_chunk(IVec3::ZERO, chunk.clone());
        let tiles = tiles
            .iter()
            .map(|(name, tile)| (name.to_string(), *tile))
            .collect();
        let mesh = generate_chunk_mesh(&world, &chunk, &IVec3::ZERO, &tiles)
            .solid_mesh
            .unwrap();
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("no UVs");
        };
        for uv in uvs {
            assert!(uv.iter().all(|c| 0. < *c && *c < 1.), "{uv:?}");
        }
        let Some(bevy::mesh::VertexAttributeValues::Unorm16x2(tiles)) = mesh.attribute(TILE) else {
            panic!("a chunk mesh has tiles");
        };
        tiles.iter().copied().zip(normals_of(&mesh)).collect()
    }

    #[test]
    fn each_vertex_names_the_block_texture_of_its_face_and_a_point_inside_it() {
        let tiles = [
            ("_Default", 0),
            ("Grass", 1),
            ("GrassTop", 2),
            ("Dirt", 3),
            ("GrassSideOverlay", 4),
        ];
        // The colour map of the grass, and the colour of the grass where the climate is not
        // known yet
        let grass = 1 | 1 << 2;
        for (tile, normal) in grass_block_tiles(&tiles) {
            // The top is coloured by the climate, the sides under their overlay, one more than
            // its tile, and the bottom is dirt
            let expected = match normal.y {
                1. => [2, grass],
                -1. => [3, 0],
                _ => [1, grass | (4 + 1) << 6],
            };
            assert_eq!(tile, expected, "normal {normal:?}");
        }
    }

    #[test]
    fn a_texture_pack_without_the_overlay_draws_the_sides_uncoloured() {
        let tiles = [("_Default", 0), ("Grass", 1), ("GrassTop", 2)];
        for (tile, normal) in grass_block_tiles(&tiles) {
            if normal.y == 0. {
                assert_eq!(tile, [1, 0], "normal {normal:?}");
            }
        }
    }

    #[test]
    fn an_item_keeps_its_colour_and_is_lit_as_under_the_sky_wherever_it_lies() {
        let tiles = HashMap::from([("_Default".to_string(), 0)]);
        for block in [BlockId::Grass, BlockId::OakLeaves] {
            let mesh = block_item_mesh(block, &tiles).unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Unorm16x2(tiles)) = mesh.attribute(TILE)
            else {
                panic!("no tiles");
            };
            // No colour map nor sway: the colour of the face, from bits 2 and 3, stays
            assert!(
                tiles.iter().all(|tile| tile[1] & !(3 << 2) == 0),
                "{block:?}"
            );
            assert!(tiles.iter().any(|tile| tile[1] == 1 << 2), "{block:?}");
            assert!(lights_of(&mesh).iter().all(|light| *light == FULL_LIGHT));
        }
    }

    /// The mesh of `blocks` in the chunk at the origin, lit by their light.
    fn lit_mesh_of(blocks: &[(IVec3, BlockId)]) -> ChunkMeshResponse {
        let mut chunk = Chunk::new();
        for (position, id) in blocks {
            chunk.set(*position, BlockData::new(*id, BlockDirection::Front));
        }
        let mut world = ChunkMap::default();
        world.insert_chunk(IVec3::ZERO, chunk.clone());
        let tiles = HashMap::from([("_Default".to_string(), 0)]);
        generate_lit_chunk_mesh(&world, &chunk, &IVec3::ZERO, &tiles)
    }

    /// The light of the vertices of the faces of `mesh` that look along `normal`, by position.
    fn lights_toward(mesh: &Mesh, normal: Vec3) -> Vec<(Vec3, [u8; 4])> {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("a chunk mesh has positions");
        };
        positions
            .iter()
            .zip(normals_of(mesh))
            .zip(lights_of(mesh))
            .filter(|((_, n), _)| n.distance(normal) < 0.01)
            .map(|((position, _), light)| (Vec3::from(*position), light))
            .collect()
    }

    #[test]
    fn a_face_is_lit_by_the_cell_in_front_of_it() {
        // A slab of stone, a block thick: its top is under the sky, its bottom too, through
        // the air around the chunk, but a level a block darker
        let mut blocks = Vec::new();
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                blocks.push((IVec3::new(x, 8, z), BlockId::Stone));
            }
        }
        let mesh = lit_mesh_of(&blocks).solid_mesh.unwrap();
        for (_, light) in lights_toward(&mesh, Vec3::Y) {
            assert_eq!(light, [255, 0, 255, 255]);
        }
        let below = lights_toward(&mesh, Vec3::NEG_Y);
        let darkest = below.iter().map(|(_, light)| light[0]).min().unwrap();
        assert!(darkest < 255 - 6 * 17, "{darkest}");
        // From the edge, where the sky comes in, to the middle
        let at = |x: f32| below.iter().find(|(p, _)| p.x == x && p.z == 8.).unwrap().1[0];
        assert!(at(0.) > at(4.) && at(4.) > at(8.));
    }

    #[test]
    fn a_corner_against_two_walls_is_shaded_and_lit_by_the_open_cells_only() {
        // A floor block with walls along -x and -z above it
        let mesh = lit_mesh_of(&[
            (IVec3::new(5, 5, 5), BlockId::Stone),
            (IVec3::new(4, 6, 5), BlockId::Stone),
            (IVec3::new(5, 6, 4), BlockId::Stone),
        ])
        .solid_mesh
        .unwrap();
        let top: Vec<_> = lights_toward(&mesh, Vec3::Y)
            .into_iter()
            .filter(|(p, _)| p.y == 6. && (5. ..=6.).contains(&p.x) && (5. ..=6.).contains(&p.z))
            .collect();
        let corner = |x: f32, z: f32| top.iter().find(|(p, _)| p.x == x && p.z == z).unwrap().1;
        // Both sides blocked: the darkest corner, lit by the cell in front only
        assert_eq!(corner(5., 5.), [255, 0, 0, 255]);
        // One side blocked
        assert_eq!(corner(6., 5.)[2], 170);
        assert_eq!(corner(5., 6.)[2], 170);
        // In the open
        assert_eq!(corner(6., 6.)[2], 255);
    }

    #[test]
    fn faces_lit_alike_merge_and_shaded_ones_stay_squares() {
        // A floor of 4 blocks in a row, a wall along one end of it
        let row: Vec<_> = (0..4)
            .map(|x| (IVec3::new(x + 2, 5, 5), BlockId::Stone))
            .collect();
        let open = lit_mesh_of(&row).solid_mesh.unwrap();
        assert_eq!(lights_toward(&open, Vec3::Y).len(), 4);
        let mut walled = row.clone();
        walled.push((IVec3::new(1, 6, 5), BlockId::Stone));
        let walled = lit_mesh_of(&walled).solid_mesh.unwrap();
        // The square next to the wall stands apart, shaded on one side: the rest merge
        let tops: Vec<_> = lights_toward(&walled, Vec3::Y)
            .into_iter()
            .filter(|(p, _)| p.y == 6. && p.x >= 2.)
            .collect();
        assert_eq!(tops.len(), 8);
    }

    #[test]
    fn a_square_is_split_across_its_brightest_corners() {
        // `CLOCKWISE` splits across the corners 0 and 2
        assert_eq!(square_corners(&[0, 1, 2, 2, 3, 0]), Some([0, 1, 2, 3]));
        assert_eq!(square_corners(&[3, 0, 1, 1, 2, 3]), Some([3, 0, 1, 2]));
        assert_eq!(square_corners(&[0, 5, 7, 7, 5, 0]), None);

        let face = &CUBE.faces[0];
        let dark = [255, 0, 0, 255];
        let bright = FULL_LIGHT;
        let mut mesh = MeshCreator::default();
        mesh.push_face(
            face,
            0,
            None,
            Vec3::ZERO,
            &BlockDirection::Front,
            &[dark, bright, dark, bright],
        );
        // Split across the bright corners 1 and 3, which both triangles share
        let shared: Vec<_> = mesh.indices[..3]
            .iter()
            .filter(|i| mesh.indices[3..].contains(i))
            .collect();
        assert_eq!(shared, [&1, &3]);
        // Each triangle keeps the winding of the face
        let normal = |t: &[u32]| {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(mesh.vertices[t[k] as usize]));
            (b - a).cross(c - a).normalize()
        };
        assert_eq!(normal(&mesh.indices[..3]), normal(&face.indices[..3]));
        assert_eq!(normal(&mesh.indices[3..]), normal(&mesh.indices[..3]));
    }

    #[test]
    fn swaying_faces_never_merge() {
        let leaves: Vec<_> = (0..4)
            .map(|x| (IVec3::new(x + 2, 5, 5), BlockId::OakLeaves))
            .collect();
        let mesh = mesh_of(&leaves).solid_mesh.unwrap();
        assert_eq!(lights_toward(&mesh, Vec3::Y).len(), 4 * 4);
    }

    const DIRECTIONS: [BlockDirection; 4] = [
        BlockDirection::Front,
        BlockDirection::Right,
        BlockDirection::Back,
        BlockDirection::Left,
    ];

    #[test]
    fn a_turned_shape_turns_about_the_centre_of_its_cell_with_its_normals() {
        // the front face of a cube, at z = 0, looking toward -Z
        let front = &CUBE.faces[2];
        for direction in DIRECTIONS {
            let mut mesh = MeshCreator::default();
            mesh.push_face(front, 0, None, Vec3::ZERO, &direction, &[FULL_LIGHT; 4]);
            let side = direction.offset().as_vec3();
            for (vertex, normal) in mesh.vertices.iter().zip(&mesh.normals) {
                let vertex = Vec3::from(*vertex);
                assert!(
                    vertex.min_element() > -1e-6 && vertex.max_element() < 1. + 1e-6,
                    "{direction:?}: {vertex} out of the cell"
                );
                let across = (vertex - 0.5).dot(side);
                assert!((across - 0.5).abs() < 1e-6, "{direction:?}: {vertex}");
                assert_eq!(*normal, super::normal(&side.into()), "{direction:?}");
            }
        }
    }

    /// The position of each vertex of `mesh`.
    fn positions_of(mesh: &Mesh) -> Vec<Vec3> {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("a chunk mesh has positions");
        };
        positions.iter().map(|p| Vec3::from(*p)).collect()
    }

    #[test]
    fn a_vine_lies_against_the_block_that_holds_it_and_hides_none_of_its_faces() {
        let tiles = HashMap::from([("_Default".to_string(), 0), ("Vine".to_string(), 1)]);
        let stone = IVec3::splat(5);
        for direction in DIRECTIONS {
            let vine = stone - direction.offset();
            let mut chunk = Chunk::new();
            chunk.set(stone, BlockData::new(BlockId::Stone, BlockDirection::Front));
            chunk.set(vine, BlockData::new(BlockId::Vine, direction));
            let mut world = ChunkMap::default();
            world.insert_chunk(IVec3::ZERO, chunk.clone());
            let mesh = generate_chunk_mesh(&world, &chunk, &IVec3::ZERO, &tiles)
                .solid_mesh
                .unwrap();

            let side = direction.offset().as_vec3();
            let centre = vine.as_vec3() + 0.5;
            let mut stone_faces = 0;
            let vertices = positions_of(&mesh).into_iter().zip(normals_of(&mesh));
            for ((position, normal), tile) in vertices.zip(tiles_of(&mesh)) {
                if tile == 0 {
                    stone_faces += 1;
                    continue;
                }
                // in the vine's cell, 1/16 off the stone, seen from both sides
                let offset = position - centre;
                assert!(offset.abs().max_element() <= 0.5 + 1e-6, "{direction:?}");
                let gap = 0.5 - offset.dot(side);
                assert!((gap - VINE_GAP).abs() < 1e-6, "{direction:?}: {gap}");
                assert!((normal.dot(side).abs() - 1.).abs() < 0.01, "{direction:?}");
            }
            // every face of the stone, the one behind the vine too
            assert_eq!(stone_faces, 6 * 4, "{direction:?}");
            assert_eq!(mesh.count_vertices(), 6 * 4 + 8, "{direction:?}");
        }
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
