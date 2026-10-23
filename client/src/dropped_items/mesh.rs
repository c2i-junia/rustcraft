//! What a dropped item looks like: a block that is a whole cube is drawn as a small cube with
//! the textures of the chunks, anything else as a picture from the item atlas one pixel thick.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use shared::world::{ItemId, ItemType, DROPPED_ITEM_SIZE};
use std::collections::HashMap;

use crate::world::{meshing::block_item_mesh, ChunkMaterial, GlobalMaterial, MaterialResource};

/// Edge of the picture of an item that is not drawn as a cube, in blocks.
const SPRITE_SIZE: f32 = 0.4;

#[derive(Clone, Debug)]
pub(super) enum ItemLook {
    Cube(Handle<Mesh>, Handle<ChunkMaterial>),
    Sprite(Handle<Mesh>, Handle<StandardMaterial>),
}

impl ItemLook {
    /// The width of what is drawn, in blocks.
    pub(super) fn size(&self) -> f32 {
        match self {
            ItemLook::Cube(..) => DROPPED_ITEM_SIZE,
            ItemLook::Sprite(..) => SPRITE_SIZE,
        }
    }
}

/// The look of each item drawn so far, made the first time an item of its kind is shown.
#[derive(Resource, Default, Debug)]
pub(super) struct ItemLooks(HashMap<ItemId, ItemLook>);

impl ItemLooks {
    /// The look of `item`, `None` while the atlases are not built or when they have no
    /// texture for it.
    pub(super) fn get(
        &mut self,
        item: ItemId,
        materials: &MaterialResource,
        meshes: &mut Assets<Mesh>,
        images: &Assets<Image>,
    ) -> Option<ItemLook> {
        if let Some(look) = self.0.get(&item) {
            return Some(look.clone());
        }
        let look = cube_look(item, materials, meshes)
            .or_else(|| sprite_look(item, materials, meshes, images))?;
        self.0.insert(item, look.clone());
        Some(look)
    }
}

fn cube_look(
    item: ItemId,
    materials: &MaterialResource,
    meshes: &mut Assets<Mesh>,
) -> Option<ItemLook> {
    let ItemType::Block(block) = item.get_default_type() else {
        return None;
    };
    let mesh = block_item_mesh(block, &materials.blocks.as_ref()?.looks)?;
    let material = materials.chunk_materials.get(&GlobalMaterial::Blocks)?;
    Some(ItemLook::Cube(meshes.add(mesh), material.clone()))
}

fn sprite_look(
    item: ItemId,
    materials: &MaterialResource,
    meshes: &mut Assets<Mesh>,
    images: &Assets<Image>,
) -> Option<ItemLook> {
    let atlas = materials.items.as_ref()?;
    let name = format!("{item:?}");
    let name = if atlas.tiles.contains_key(&name) {
        name.as_str()
    } else {
        "_Default"
    };
    let rect = atlas.tile_rects[*atlas.tiles.get(name)? as usize];
    let material = materials.global_materials.get(&GlobalMaterial::Items)?;
    let pixels = atlas
        .handles
        .get(name)
        .and_then(|handle| images.get(handle))
        .map(Pixels::of)
        .unwrap_or(Pixels::SOLID);
    Some(ItemLook::Sprite(
        meshes.add(sprite_mesh(rect, &pixels)),
        material.clone(),
    ))
}

/// Which pixels of a picture are drawn, row by row from the top: those the material's alpha
/// mask keeps.
#[derive(Debug)]
struct Pixels {
    width: u32,
    height: u32,
    opaque: Vec<bool>,
}

impl Pixels {
    /// A picture of 16 by 16 pixels all drawn, for an image that cannot be read.
    const SOLID: Pixels = Pixels {
        width: 16,
        height: 16,
        opaque: Vec::new(),
    };

    fn of(image: &Image) -> Pixels {
        let (width, height) = (image.width(), image.height());
        let opaque = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| image.get_color_at(x, y).is_ok_and(|c| c.alpha() >= 0.5))
            .collect();
        Pixels {
            width,
            height,
            opaque,
        }
    }

    /// Whether the pixel at column `x` and row `y` is drawn: none outside the picture.
    fn is_opaque(&self, x: i64, y: i64) -> bool {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return false;
        }
        self.opaque.is_empty() || self.opaque[(y * self.width as i64 + x) as usize]
    }
}

/// A picture one block wide and one of its pixels thick, centred on the origin and facing +Z,
/// showing the part `rect` of the atlas (min x, min y, max x, max y): a square on each side,
/// and around each pixel drawn, the edges that border none, in its colour, as Minecraft draws
/// the items held and dropped.
fn sprite_mesh(rect: Vec4, pixels: &Pixels) -> Mesh {
    let (width, height) = (pixels.width as f32, pixels.height as f32);
    let half_depth = 0.5 / width;
    // The point of the atlas at `(x, y)` of the square, the top of the picture being at the top
    // of the atlas rectangle, its smaller y
    let uv = |x: f32, y: f32| {
        [
            rect.x + (rect.z - rect.x) * (x + 0.5),
            rect.w - (rect.w - rect.y) * (y + 0.5),
        ]
    };
    let mut quads = SpriteQuads::default();
    let corners = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
    for side in [1.0, -1.0] {
        quads.push(
            corners.map(|[x, y]| Vec3::new(x, y, side * half_depth)),
            Vec3::Z * side,
            corners.map(|[x, y]| uv(x, y)),
        );
    }
    for row in 0..pixels.height as i64 {
        for column in 0..pixels.width as i64 {
            if !pixels.is_opaque(column, row) {
                continue;
            }
            let x0 = column as f32 / width - 0.5;
            let x1 = x0 + 1.0 / width;
            let y1 = 0.5 - row as f32 / height;
            let y0 = y1 - 1.0 / height;
            // A side shows the colour of its pixel, sampled at its centre
            let colour = [uv((x0 + x1) / 2.0, (y0 + y1) / 2.0); 4];
            let edges = [
                (-1, 0, Vec3::NEG_X, [x0, y0], [x0, y1]),
                (1, 0, Vec3::X, [x1, y0], [x1, y1]),
                (0, -1, Vec3::Y, [x0, y1], [x1, y1]),
                (0, 1, Vec3::NEG_Y, [x0, y0], [x1, y0]),
            ];
            for (dx, dy, normal, [ax, ay], [bx, by]) in edges {
                if pixels.is_opaque(column + dx, row + dy) {
                    continue;
                }
                let face = [
                    Vec3::new(ax, ay, -half_depth),
                    Vec3::new(bx, by, -half_depth),
                    Vec3::new(bx, by, half_depth),
                    Vec3::new(ax, ay, half_depth),
                ];
                quads.push(face, normal, colour);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, quads.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, quads.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, quads.uvs)
    .with_inserted_indices(Indices::U32(quads.indices))
}

#[derive(Default)]
struct SpriteQuads {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl SpriteQuads {
    /// A quad of the corners in order round it, turned to face `normal`.
    fn push(&mut self, mut corners: [Vec3; 4], normal: Vec3, mut uvs: [[f32; 2]; 4]) {
        if (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .dot(normal)
            < 0.0
        {
            corners.reverse();
            uvs.reverse();
        }
        let first = self.positions.len() as u32;
        self.positions.extend(corners.map(|c| c.to_array()));
        self.normals.extend([normal.to_array(); 4]);
        self.uvs.extend(uvs);
        self.indices
            .extend([0, 1, 2, 0, 2, 3].map(|index| first + index));
    }
}

/// How many copies of an item are drawn for a stack of `nb`: more for a bigger stack.
pub(super) fn copies(nb: u32) -> usize {
    match nb {
        0..=1 => 1,
        2..=16 => 2,
        17..=32 => 3,
        33..=48 => 4,
        _ => 5,
    }
}

/// Where the copy `index` of a stack is drawn, in widths of the item: a little apart, so
/// that they can be counted.
pub(super) fn copy_offset(index: usize) -> Vec3 {
    const OFFSETS: [Vec3; 5] = [
        Vec3::ZERO,
        Vec3::new(0.25, 0.2, 0.2),
        Vec3::new(-0.2, 0.4, -0.25),
        Vec3::new(0.2, 0.6, -0.15),
        Vec3::new(-0.25, 0.8, 0.2),
    ];
    OFFSETS[index % OFFSETS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bigger_stack_shows_more_copies_up_to_five() {
        let shown: Vec<_> = [1, 2, 16, 17, 32, 33, 48, 49, 64].map(copies).into();
        assert_eq!(shown, [1, 2, 2, 3, 3, 4, 4, 5, 5]);
    }

    use bevy::mesh::VertexAttributeValues;

    /// A picture `width` pixels wide, drawn where `rows` has a `#`.
    fn pixels(rows: &[&str]) -> Pixels {
        Pixels {
            width: rows[0].len() as u32,
            height: rows.len() as u32,
            opaque: rows.concat().chars().map(|c| c == '#').collect(),
        }
    }

    fn positions(mesh: &Mesh) -> Vec<Vec3> {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        positions.iter().map(|p| Vec3::from_array(*p)).collect()
    }

    #[test]
    fn a_sprite_shows_its_rectangle_upright() {
        let mesh = sprite_mesh(Vec4::new(0.25, 0.5, 0.5, 0.75), &Pixels::SOLID);
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("no UVs");
        };
        // bottom left, then top right
        assert_eq!(uvs[0], [0.25, 0.75]);
        assert_eq!(uvs[2], [0.5, 0.5]);
    }

    #[test]
    fn a_sprite_is_one_of_its_pixels_thick() {
        for (picture, thickness) in [(Pixels::SOLID, 1.0 / 16.0), (pixels(&["##"]), 0.5)] {
            let positions = positions(&sprite_mesh(Vec4::new(0., 0., 1., 1.), &picture));
            let (min, max) = positions
                .iter()
                .fold((f32::MAX, f32::MIN), |(min, max), p| {
                    (min.min(p.z), max.max(p.z))
                });
            assert_eq!(
                (min, max),
                (-thickness / 2.0, thickness / 2.0),
                "{picture:?}"
            );
        }
    }

    #[test]
    fn only_the_edges_of_the_drawn_pixels_that_border_none_have_a_side() {
        // Both sides, then the sides: the outline of the shape, 4 + 4 + 2 for an L of three
        let quads = |rows: &[&str]| {
            let mesh = sprite_mesh(Vec4::new(0., 0., 1., 1.), &pixels(rows));
            positions(&mesh).len() / 4
        };
        assert_eq!(quads(&["..", ".."]), 2);
        assert_eq!(quads(&["#.", ".."]), 2 + 4);
        assert_eq!(quads(&["##", "##"]), 2 + 8);
        assert_eq!(quads(&["#.", "##"]), 2 + 8);
        assert_eq!(quads(&["#.", ".#"]), 2 + 8);
    }

    #[test]
    fn every_face_turns_to_its_normal() {
        let mesh = sprite_mesh(Vec4::new(0., 0., 1., 1.), &pixels(&["##", "##"]));
        let positions = positions(&mesh);
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("no normals");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("no indices");
        };
        for triangle in indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| positions[triangle[i] as usize]);
            let normal = Vec3::from_array(normals[triangle[0] as usize]);
            assert!((b - a).cross(c - a).dot(normal) > 0.0, "{triangle:?}");
            // Outward: away from the centre of the picture
            assert!(((a + b + c) / 3.0).dot(normal) > 0.0, "{triangle:?}");
        }
    }
}
