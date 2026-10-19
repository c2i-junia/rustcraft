//! What a dropped item looks like: a block that is a whole cube is drawn as a small cube with
//! the textures of the chunks, anything else as a flat picture from the item atlas.

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
    ) -> Option<ItemLook> {
        if let Some(look) = self.0.get(&item) {
            return Some(look.clone());
        }
        let look =
            cube_look(item, materials, meshes).or_else(|| sprite_look(item, materials, meshes))?;
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
    let mesh = block_item_mesh(block, &materials.blocks.as_ref()?.tiles)?;
    let material = materials.chunk_materials.get(&GlobalMaterial::Blocks)?;
    Some(ItemLook::Cube(meshes.add(mesh), material.clone()))
}

fn sprite_look(
    item: ItemId,
    materials: &MaterialResource,
    meshes: &mut Assets<Mesh>,
) -> Option<ItemLook> {
    let atlas = materials.items.as_ref()?;
    let tile = atlas
        .tiles
        .get(&format!("{item:?}"))
        .or_else(|| atlas.tiles.get("_Default"))?;
    let rect = atlas.tile_rects[*tile as usize];
    let material = materials.global_materials.get(&GlobalMaterial::Items)?;
    Some(ItemLook::Sprite(
        meshes.add(sprite_mesh(rect)),
        material.clone(),
    ))
}

/// A square one block wide, centred on the origin and facing +Z, showing the part `rect` of
/// the atlas (min x, min y, max x, max y).
fn sprite_mesh(rect: Vec4) -> Mesh {
    let corners = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
    let positions: Vec<[f32; 3]> = corners.iter().map(|[x, y]| [*x, *y, 0.]).collect();
    // The top of the picture is at the top of the atlas rectangle, its smaller y
    let uvs: Vec<[f32; 2]> = corners
        .iter()
        .map(|[x, y]| {
            [
                rect.x + (rect.z - rect.x) * (x + 0.5),
                rect.w - (rect.w - rect.y) * (y + 0.5),
            ]
        })
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; 4])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
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

    #[test]
    fn a_sprite_shows_its_rectangle_upright() {
        let mesh = sprite_mesh(Vec4::new(0.25, 0.5, 0.5, 0.75));
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("no UVs");
        };
        // bottom left, then top right
        assert_eq!(uvs[0], [0.25, 0.75]);
        assert_eq!(uvs[2], [0.5, 0.5]);
    }
}
