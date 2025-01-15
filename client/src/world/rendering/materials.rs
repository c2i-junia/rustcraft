use crate::constants::{BASE_ROUGHNESS, BASE_SPECULAR_HIGHLIGHT};
use crate::game::PreLoadingCompletion;
use crate::world::GlobalMaterial;
use crate::TexturePath;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use shared::world::{get_game_folder, BlockId, GameElementId, ItemId};
use shared::{GameFolderPaths, SpecialFlag};
use std::collections::HashMap;
use std::fs;
use std::marker::PhantomData;

use super::meshing::UvCoords;

#[derive(Resource, Debug)]
pub struct AtlasWrapper {
    pub handles: HashMap<String, Handle<Image>>,
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub sources: TextureAtlasSources,
    pub uvs: HashMap<String, UvCoords>,
}

#[derive(Resource, Default)]
pub struct MaterialResource {
    pub global_materials: HashMap<GlobalMaterial, Handle<StandardMaterial>>,
    pub items: Option<AtlasWrapper>,
    pub blocks: Option<AtlasWrapper>,
}

#[derive(Resource)]
pub struct AtlasHandles<T> {
    pub handles: Vec<(Handle<Image>, String)>,
    pub loaded: bool,
    /// Phantom to allow multiple instances of the struct
    _d: PhantomData<T>,
}

impl<T> Default for AtlasHandles<T> {
    fn default() -> Self {
        Self {
            handles: Vec::new(),
            loaded: false,
            _d: PhantomData {},
        }
    }
}

pub fn setup_materials(
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_resource: ResMut<MaterialResource>,
    mut block_atlas_handles: ResMut<AtlasHandles<BlockId>>,
    mut item_atlas_handles: ResMut<AtlasHandles<ItemId>>,
    texture_path: Res<TexturePath>,
    paths: Res<GameFolderPaths>,
    special_flag: Res<SpecialFlag>,
) {
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1., 0.95, 0.1),
        emissive: LinearRgba::new(1., 0.95, 0.1, 0.5),
        emissive_exposure_weight: 0.5,
        cull_mode: Some(Face::Front),
        ..Default::default()
    });

    let moon_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::WHITE,
        emissive_exposure_weight: 0.5,
        cull_mode: Some(Face::Front),
        ..Default::default()
    });

    material_resource
        .global_materials
        .insert(GlobalMaterial::Sun, sun_material);
    material_resource
        .global_materials
        .insert(GlobalMaterial::Moon, moon_material);

    let (blocks_path, items_path) = if !special_flag.special_flag {
        (
            get_game_folder(Some(&paths))
                .join("data/")
                .join(&texture_path.path)
                .join("blocks/"),
            get_game_folder(Some(&paths))
                .join("data/")
                .join(&texture_path.path)
                .join("items/"),
        )
    } else {
        (
            get_game_folder(Some(&paths))
                .join(paths.assets_folder_path.clone())
                .join(&texture_path.path)
                .join("blocks/"),
            get_game_folder(Some(&paths))
                .join(paths.assets_folder_path.clone())
                .join(&texture_path.path)
                .join("items/"),
        )
    };

    if let Ok(dir) = fs::read_dir(blocks_path.clone()) {
        block_atlas_handles.handles = dir
            .map(|file| {
                let binding = file.unwrap().path();
                let filename = binding.file_stem().unwrap().to_str().unwrap();
                (
                    asset_server.load(
                        blocks_path
                            .join(filename)
                            .with_extension("png")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    filename.to_owned(),
                )
            })
            .collect();
        info!("Block textures loaded");
    } else {
        warn!(
            "Block textures could not be loaded. This could crash the game : {:?}",
            blocks_path.display()
        );
    }

    if let Ok(dir) = fs::read_dir(items_path.clone()) {
        item_atlas_handles.handles = dir
            .map(|file| {
                let binding = file.unwrap().path();
                let filename = binding.file_stem().unwrap().to_str().unwrap();
                (
                    asset_server.load(
                        items_path
                            .join(filename)
                            .with_extension("png")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    filename.to_owned(),
                )
            })
            .collect();
        info!("Block textures loaded");
    } else {
        warn!(
            "Block textures could not be loaded. This could crash the game : {:?}",
            items_path.display()
        );
    }
}

pub fn create_all_atlases(
    mut atlases: (ResMut<AtlasHandles<BlockId>>, ResMut<AtlasHandles<ItemId>>),
    mut images: ResMut<Assets<Image>>,
    mut material_resource: ResMut<MaterialResource>,
    mut loading: ResMut<PreLoadingCompletion>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    loading.textures_loaded = true;

    if material_resource.blocks.is_none() {
        if let Some(blocks) = build_texture_atlas(
            &mut atlases.0,
            &mut images,
            &mut texture_atlases,
            None,
            Some(ImageSampler::nearest()),
        ) {
            material_resource.global_materials.insert(
                GlobalMaterial::Blocks,
                materials.add(StandardMaterial {
                    base_color_texture: Some(blocks.texture.clone_weak()),
                    perceptual_roughness: BASE_ROUGHNESS,
                    reflectance: BASE_SPECULAR_HIGHLIGHT,
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                }),
            );
            material_resource.blocks = Some(blocks);
            debug!(
                "Uv sources : {:?}",
                material_resource.blocks.as_ref().unwrap()
            );
        } else {
            loading.textures_loaded = false;
        }
    }

    if material_resource.items.is_none() {
        if let Some(items) = build_texture_atlas(
            &mut atlases.1,
            &mut images,
            &mut texture_atlases,
            None,
            Some(ImageSampler::linear()),
        ) {
            material_resource.items = Some(items);
        } else {
            loading.textures_loaded = false;
        }
    }
}

fn build_texture_atlas<T: GameElementId>(
    atlas_handles: &mut AtlasHandles<T>,
    images: &mut ResMut<Assets<Image>>,
    texture_atlases: &mut ResMut<Assets<TextureAtlasLayout>>,
    padding: Option<UVec2>,
    sampling: Option<ImageSampler>,
) -> Option<AtlasWrapper> {
    if atlas_handles.loaded {
        // Blocks if this atlas is loaded but game setup phase is not done yet
        return None;
    }

    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    texture_atlas_builder.padding(padding.unwrap_or_default());

    for handle in atlas_handles.handles.iter() {
        let id = handle.0.id();
        let Some(texture) = images.get(id) else {
            // Not all images are loaded yet
            return None;
        };

        texture_atlas_builder.add_texture(Some(id), texture);
    }

    let (texture_atlas_layout, texture_atlas_sources, texture) =
        texture_atlas_builder.build().unwrap();

    let size = texture.size_f32();
    let texture = images.add(texture);
    // Update the sampling settings of the texture atlas
    let image = images.get_mut(&texture).unwrap();
    image.sampler = sampling.unwrap_or_default();

    // Create UV references
    let mut handles = HashMap::new();
    let mut uvs = HashMap::new();
    for i in atlas_handles.handles.iter() {
        handles.insert(i.1.clone(), i.0.clone_weak());
        let rect = texture_atlas_sources
            .texture_rect(&texture_atlas_layout, i.0.id())
            .unwrap_or_default();

        let uv_coords = UvCoords::new(
            rect.min.x as f32 / size.x,
            rect.max.x as f32 / size.x,
            rect.min.y as f32 / size.y,
            rect.max.y as f32 / size.y,
        );

        uvs.insert(i.1.clone(), uv_coords);
    }

    debug!("Total atlas size : {size:?}");

    // Create the atlas
    Some(AtlasWrapper {
        texture,
        layout: texture_atlases.add(texture_atlas_layout),
        sources: texture_atlas_sources,
        handles,
        uvs,
    })
}
