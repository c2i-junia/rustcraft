use crate::constants::{BASE_ROUGHNESS, BASE_SPECULAR_HIGHLIGHT};
use crate::state::PreLoadingCompletion;
use crate::state::TexturePath;
use crate::world::GlobalMaterial;
use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::image::ImageSampler;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Face};
use bevy::shader::ShaderRef;
use shared::world::{BlockId, GameElementId, ItemId};
use shared::GameFolderPaths;
use std::collections::HashMap;
use std::fs;
use std::marker::PhantomData;
use std::path::Path;

#[derive(Debug)]
pub struct AtlasWrapper {
    pub handles: HashMap<String, Handle<Image>>,
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub sources: TextureAtlasSources,
    /// The index in `tile_rects` of each texture, by name.
    pub tiles: HashMap<String, u32>,
    /// Where each texture lies in the atlas: min x, min y, max x, max y, in atlas units.
    pub tile_rects: Vec<Vec4>,
}

impl AtlasWrapper {
    /// The place in the atlas of the texture called `name`, or of the `_Default` texture when
    /// the texture pack has no such texture.
    pub fn sprite(&self, name: &str) -> Option<TextureAtlas> {
        let image = self
            .handles
            .get(name)
            .or_else(|| self.handles.get("_Default"))?;
        self.sources.handle(self.layout.clone(), image.id())
    }
}

/// The most block textures a texture pack may have: the length of the array of tiles in
/// `chunk.wgsl`.
pub const MAX_TILES: usize = 256;

const CHUNK_SHADER: Handle<Shader> = uuid_handle!("8d2b8c4e-6a51-4f7e-9b0c-3e5f1a7d9c21");

/// What the chunks are drawn with: the material of the block atlas, whose tiles repeat over the
/// faces that span several blocks.
pub type ChunkMaterial = ExtendedMaterial<StandardMaterial, AtlasTiles>;

/// Where each texture lies in the atlas, for `chunk.wgsl`. A chunk mesh names the tile of each
/// vertex by its index (`UV_1.x`), and the point in the tile in `UV_0`, which repeats past 1.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct AtlasTiles {
    /// min x, min y, max x, max y of each tile, in atlas units
    #[uniform(100)]
    pub rects: [Vec4; MAX_TILES],
}

impl AtlasTiles {
    fn new(rects: &[Vec4]) -> Self {
        if rects.len() > MAX_TILES {
            error!(
                "{} block textures, but no more than {MAX_TILES} can be drawn",
                rects.len()
            );
        }
        let mut tiles = Self {
            rects: [Vec4::ZERO; MAX_TILES],
        };
        for (tile, rect) in tiles.rects.iter_mut().zip(rects) {
            *tile = *rect;
        }
        tiles
    }
}

impl MaterialExtension for AtlasTiles {
    fn fragment_shader() -> ShaderRef {
        CHUNK_SHADER.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        CHUNK_SHADER.into()
    }
}

/// Makes `ChunkMaterial` drawable.
pub fn chunk_material_plugin(app: &mut App) {
    load_internal_asset!(app, CHUNK_SHADER, "chunk.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<ChunkMaterial>::default());
}

#[derive(Resource, Default, Debug)]
pub struct MaterialResource {
    pub global_materials: HashMap<GlobalMaterial, Handle<StandardMaterial>>,
    /// The materials of the blocks and of the liquids.
    pub chunk_materials: HashMap<GlobalMaterial, Handle<ChunkMaterial>>,
    pub items: Option<AtlasWrapper>,
    pub blocks: Option<AtlasWrapper>,
}

#[derive(Resource)]
pub struct AtlasHandles<T> {
    pub handles: Vec<(Handle<Image>, String)>,
    /// Phantom to allow multiple instances of the struct
    _d: PhantomData<T>,
}

impl<T> Default for AtlasHandles<T> {
    fn default() -> Self {
        Self {
            handles: Vec::new(),
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
) {
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1., 0.95, 0.1),
        emissive: LinearRgba::new(1., 0.95, 0.1, 0.5),
        emissive_exposure_weight: 0.5,
        cull_mode: Some(Face::Front),
        ..default()
    });

    let moon_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::WHITE,
        emissive_exposure_weight: 0.5,
        cull_mode: Some(Face::Front),
        ..default()
    });

    material_resource
        .global_materials
        .insert(GlobalMaterial::Sun, sun_material);
    material_resource
        .global_materials
        .insert(GlobalMaterial::Moon, moon_material);

    let textures_path = paths.assets_folder_path.join(&texture_path.path);
    block_atlas_handles.handles = load_textures(&asset_server, &textures_path.join("blocks/"));
    let mut items = load_textures(&asset_server, &textures_path.join("items/"));
    // An item with no texture of its own, like most blocks, shows the texture of its block
    let block_textures: Vec<_> = block_atlas_handles
        .handles
        .iter()
        .filter(|(_, block)| items.iter().all(|(_, item)| item != block))
        .cloned()
        .collect();
    items.extend(block_textures);
    item_atlas_handles.handles = items;
}

/// Starts loading every texture of `folder`, each with the name of its file without the
/// extension, which is the name of the block or the item it shows.
fn load_textures(asset_server: &AssetServer, folder: &Path) -> Vec<(Handle<Image>, String)> {
    let files = match fs::read_dir(folder) {
        Ok(files) => files,
        Err(err) => {
            error!("Could not read the textures of {}: {err}", folder.display());
            return Vec::new();
        }
    };

    let textures: Vec<_> = files
        .filter_map(|file| {
            let path = file.ok()?.path();
            // A file whose name is not valid UTF-8 cannot name a block or an item
            let name = path.file_stem()?.to_str()?.to_owned();
            let texture = folder.join(&name).with_extension("png");
            Some((
                asset_server.load(texture.to_string_lossy().into_owned()),
                name,
            ))
        })
        .collect();
    info!(
        "Loading {} textures from {}",
        textures.len(),
        folder.display()
    );
    textures
}

/// Builds the block and item atlases once all their textures are loaded, and reports in
/// `PreLoadingCompletion` whether both are ready.
pub fn create_all_atlases(
    mut atlases: (ResMut<AtlasHandles<BlockId>>, ResMut<AtlasHandles<ItemId>>),
    mut images: ResMut<Assets<Image>>,
    mut material_resource: ResMut<MaterialResource>,
    mut loading: ResMut<PreLoadingCompletion>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
) {
    let atlas_material = |atlas: &AtlasWrapper, alpha_mode| StandardMaterial {
        base_color_texture: Some(atlas.texture.clone()),
        perceptual_roughness: BASE_ROUGHNESS,
        reflectance: BASE_SPECULAR_HIGHLIGHT,
        alpha_mode,
        ..default()
    };

    if material_resource.blocks.is_none() {
        if let Some(blocks) = build_texture_atlas(&mut atlases.0, &mut images, &mut texture_atlases)
        {
            let chunk_material = |base| ChunkMaterial {
                base,
                extension: AtlasTiles::new(&blocks.tile_rects),
            };
            material_resource.chunk_materials.insert(
                GlobalMaterial::Blocks,
                chunk_materials.add(chunk_material(atlas_material(
                    &blocks,
                    AlphaMode::AlphaToCoverage,
                ))),
            );
            // A liquid is seen through, and from below its surface too
            material_resource.chunk_materials.insert(
                GlobalMaterial::Liquids,
                chunk_materials.add(chunk_material(StandardMaterial {
                    cull_mode: None,
                    ..atlas_material(&blocks, AlphaMode::Blend)
                })),
            );
            material_resource.blocks = Some(blocks);
        }
    }

    if material_resource.items.is_none() {
        if let Some(items) = build_texture_atlas(&mut atlases.1, &mut images, &mut texture_atlases)
        {
            // The pictures of the items lying in the world turn round: both sides are drawn
            material_resource.global_materials.insert(
                GlobalMaterial::Items,
                materials.add(StandardMaterial {
                    cull_mode: None,
                    double_sided: true,
                    ..atlas_material(&items, AlphaMode::Mask(0.5))
                }),
            );
            material_resource.items = Some(items);
        }
    }

    loading.textures_loaded =
        material_resource.blocks.is_some() && material_resource.items.is_some();
}

/// One image holding every texture of `atlas_handles`, or `None` while some of them are still
/// loading. Pixel art is sampled without smoothing.
fn build_texture_atlas<T: GameElementId>(
    atlas_handles: &mut AtlasHandles<T>,
    images: &mut Assets<Image>,
    texture_atlases: &mut Assets<TextureAtlasLayout>,
) -> Option<AtlasWrapper> {
    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    for (handle, _) in atlas_handles.handles.iter() {
        texture_atlas_builder.add_texture(Some(handle.id()), images.get(handle.id())?);
    }

    let (texture_atlas_layout, texture_atlas_sources, mut texture) =
        match texture_atlas_builder.build() {
            Ok(atlas) => atlas,
            Err(err) => {
                error_once!("Could not build a texture atlas: {err}");
                return None;
            }
        };
    texture.sampler = ImageSampler::nearest();
    let size = texture.size_f32();
    let texture = images.add(texture);

    // Create UV references
    let mut handles = HashMap::new();
    let mut tiles = HashMap::new();
    let mut tile_rects = Vec::new();
    for (handle, name) in atlas_handles.handles.iter() {
        handles.insert(name.clone(), handle.clone());
        let rect = texture_atlas_sources
            .texture_rect(&texture_atlas_layout, handle.id())
            .unwrap_or_default();

        tiles.insert(name.clone(), tile_rects.len() as u32);
        tile_rects.push(Vec4::new(
            rect.min.x as f32 / size.x,
            rect.min.y as f32 / size.y,
            rect.max.x as f32 / size.x,
            rect.max.y as f32 / size.y,
        ));
    }

    Some(AtlasWrapper {
        texture,
        layout: texture_atlases.add(texture_atlas_layout),
        sources: texture_atlas_sources,
        handles,
        tiles,
        tile_rects,
    })
}
