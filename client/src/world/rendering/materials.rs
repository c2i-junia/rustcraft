use crate::constants::{BASE_ROUGHNESS, BASE_SPECULAR_HIGHLIGHT, GRASS_COLOR};
use crate::state::PreLoadingCompletion;
use crate::state::TexturePath;
use crate::world::overcast::new_weather_buffer;
use crate::world::precipitation::occlusion::OcclusionMap;
use crate::world::voxel::FaceColor;
use crate::world::{new_climate_map, GlobalMaterial, GraphicsSettings, BLOCK_LIGHT};
use bevy::asset::{load_internal_asset, uuid_handle, RenderAssetUsages};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use serde::Deserialize;
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

/// The block textures, each in one layer of a texture array, or in several for an animated
/// one: a chunk mesh names the texture of each vertex by its index in the table of `chunk.wgsl`
/// (`UV_1.x`), and the point in it in `UV_0`, which repeats past 1.
#[derive(Debug)]
pub struct BlockTextures {
    /// The image each texture was read from, by name.
    pub handles: HashMap<String, Handle<Image>>,
    pub array: Handle<Image>,
    /// The index in the table of each texture, by name.
    pub tiles: HashMap<String, u32>,
}

/// The most block textures a texture pack may have: the length of the table in `chunk.wgsl`.
pub const MAX_TILES: usize = 256;

/// How an animated texture, a vertical strip of square frames, plays: read from the file
/// `<texture>.anim.ron` next to it, like the `.mcmeta` of a Minecraft texture.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct TextureAnimation {
    /// How long each frame is shown, in twentieths of a second as in Minecraft.
    pub frame_time: u32,
    /// Whether each frame fades into the next one instead of replacing it at once.
    pub interpolate: bool,
}

impl Default for TextureAnimation {
    fn default() -> Self {
        Self {
            frame_time: 1,
            interpolate: false,
        }
    }
}

/// How the surface of a block texture takes the rain (`chunk.wgsl`): how much it darkens, how
/// glossy it turns and whether puddles gather on it. Its value is kept in the table of the
/// textures, over the bit of the fading of the frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum WetSurface {
    /// Stone, gravel, wood and anything not known here: darker, glossy, puddles.
    Hard = 0,
    /// Dirt: darker, less glossy, fewer puddles.
    Soil = 1,
    /// Sand: much darker, hardly glossy, no puddle, as it drinks the water.
    Porous = 2,
    /// Leaves and plants: a little darker, a faint sheen, no puddle.
    Foliage = 3,
    /// Glass, ice, snow, torches, the cracks of a block being broken: as they are.
    Dry = 4,
    /// Grass: darker, a faint sheen, no puddle, as a lawn drinks the water.
    Grass = 5,
}

impl WetSurface {
    /// The surface of the block texture called `name`.
    pub fn of(name: &str) -> Self {
        match name {
            "Glass" | "Ice" | "Snow" | "Torch" | "Water" => Self::Dry,
            _ if name.starts_with("DestroyStage") => Self::Dry,
            "Sand" => Self::Porous,
            "Grass" | "GrassTop" | "GrassSideOverlay" | "Moss" => Self::Grass,
            "Dirt" => Self::Soil,
            "TallGrass" | "Fern" | "Dandelion" | "Poppy" | "Vine" | "Cocoa" | "Cactus"
            | "CactusTop" => Self::Foliage,
            _ if name.ends_with("Leaves") => Self::Foliage,
            _ => Self::Hard,
        }
    }
}

/// The row of a texture in the table of `chunk.wgsl`: its first layer, its number of frames,
/// how long a frame is shown, and whether a frame fades into the next, with its surface over
/// that bit.
fn table_row(layer: u32, frames: u32, animation: TextureAnimation, surface: WetSurface) -> UVec4 {
    UVec4::new(
        layer,
        frames,
        animation.frame_time.max(1),
        animation.interpolate as u32 | (surface as u32) << 1,
    )
}

const CHUNK_SHADER: Handle<Shader> = uuid_handle!("8d2b8c4e-6a51-4f7e-9b0c-3e5f1a7d9c21");
/// `rustcraft::sky`, which `chunk.wgsl` imports.
const SKY_SHADER: Handle<Shader> = uuid_handle!("3f0c9a61-27d4-4b8e-a5c2-9e41d7b06f18");
/// `rustcraft::weather`, which `rustcraft::sky` and the precipitation import.
const WEATHER_SHADER: Handle<Shader> = uuid_handle!("6b1e8f20-3c7d-4a95-b2e4-0d9f5a7c1e63");

/// What the chunks are drawn with: the material of the block textures, which repeat over the
/// faces that span several blocks.
pub type ChunkMaterial = ExtendedMaterial<StandardMaterial, BlockLayers>;

/// The block textures for `chunk.wgsl`: the array of their layers, and for each texture of the
/// table its first layer, its number of frames, the time of a frame and whether frames fade.
/// With them, what the grass and the leaves are coloured with: the climate around the player
/// and the colour maps of the texture pack (see `climate.rs`), how the faces are shaded, and
/// the weather of the sky the chunks fade into and the water reflects (`overcast.rs`), and
/// where the rain stops, which says what gets wet (`occlusion.rs`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct BlockLayers {
    #[uniform(100)]
    pub table: [UVec4; MAX_TILES],
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub layers: Handle<Image>,
    #[texture(103)]
    pub climate: Handle<Image>,
    #[texture(104)]
    pub grass_colors: Handle<Image>,
    #[texture(105)]
    pub foliage_colors: Handle<Image>,
    #[uniform(106)]
    pub shading: ChunkShading,
    /// `WeatherShading`, written in place as the weather changes: a buffer of its own, as
    /// changing `shading` would prepare every chunk drawn again.
    #[storage(107, read_only)]
    pub weather: Handle<ShaderBuffer>,
    /// Where the rain stops around the camera (`OcclusionMap`): what is under it stays dry.
    #[texture(108, sample_type = "float", filterable = false)]
    pub occlusion: Handle<Image>,
}

/// The bits of `ChunkShading::effects`: the plants sway in the wind, and the water ripples and
/// reflects the light.
pub const EFFECT_SWAY: u32 = 1;
pub const EFFECT_WATER: u32 = 2;

/// How `chunk.wgsl` shades the faces, from the graphics settings (`GraphicsSettings`). Changing
/// it makes Bevy prepare every chunk drawn again: it only changes with a setting.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub struct ChunkShading {
    /// The colours of `FaceColor`, by index.
    pub palette: [Vec4; 4],
    /// `GraphicsSettings::brightness`.
    pub brightness: f32,
    /// How bright the full light of the blocks is, as the brightness of an ambient light.
    pub block_light: f32,
    /// `EFFECT_SWAY` and `EFFECT_WATER`, as the settings turn them on.
    pub effects: u32,
    /// 1 for the material of the liquids, whose surface the water effect draws.
    pub liquid: u32,
}

impl ChunkShading {
    /// The shading of the solid faces, or of the liquids if `liquid`, with `settings`.
    pub fn new(settings: &GraphicsSettings, liquid: bool) -> Self {
        let mut palette = [Vec4::ONE; 4];
        for color in FaceColor::ALL {
            palette[color.index() as usize] = Vec4::from(color.rgba());
        }
        let effects = if settings.effects {
            EFFECT_SWAY | EFFECT_WATER
        } else {
            0
        };
        Self {
            palette,
            brightness: settings.brightness,
            block_light: BLOCK_LIGHT,
            effects,
            liquid: liquid as u32,
        }
    }
}

impl MaterialExtension for BlockLayers {
    fn vertex_shader() -> ShaderRef {
        CHUNK_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        CHUNK_SHADER.into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        CHUNK_SHADER.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        CHUNK_SHADER.into()
    }
}

/// Makes `ChunkMaterial` drawable.
pub fn chunk_material_plugin(app: &mut App) {
    load_internal_asset!(app, WEATHER_SHADER, "weather.wgsl", Shader::from_wgsl);
    load_internal_asset!(app, SKY_SHADER, "sky.wgsl", Shader::from_wgsl);
    load_internal_asset!(app, CHUNK_SHADER, "chunk.wgsl", Shader::from_wgsl);
    app.add_plugins(MaterialPlugin::<ChunkMaterial>::default());
}

#[derive(Resource, Default, Debug)]
pub struct MaterialResource {
    pub global_materials: HashMap<GlobalMaterial, Handle<StandardMaterial>>,
    /// The materials of the blocks and of the liquids.
    pub chunk_materials: HashMap<GlobalMaterial, Handle<ChunkMaterial>>,
    pub items: Option<AtlasWrapper>,
    pub blocks: Option<BlockTextures>,
    /// The climate of the columns around the player: see `climate.rs`.
    pub climate_map: Handle<Image>,
    /// The colours of the grass and of the leaves by climate, from the texture pack.
    pub grass_colors: Handle<Image>,
    pub foliage_colors: Handle<Image>,
    /// What the shaders of the sky know of the weather (`WeatherShading`).
    pub weather: Handle<ShaderBuffer>,
}

#[derive(Resource)]
pub struct AtlasHandles<T> {
    pub handles: Vec<(Handle<Image>, String)>,
    /// How each animated texture plays, by name: an animated texture without a file of its
    /// own plays with the default animation.
    pub animations: HashMap<String, TextureAnimation>,
    /// Phantom to allow multiple instances of the struct
    _d: PhantomData<T>,
}

impl<T> Default for AtlasHandles<T> {
    fn default() -> Self {
        Self {
            handles: Vec::new(),
            animations: HashMap::new(),
            _d: PhantomData {},
        }
    }
}

pub fn setup_materials(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut material_resource: ResMut<MaterialResource>,
    mut block_atlas_handles: ResMut<AtlasHandles<BlockId>>,
    mut item_atlas_handles: ResMut<AtlasHandles<ItemId>>,
    texture_path: Res<TexturePath>,
    paths: Res<GameFolderPaths>,
) {
    let textures_path = paths.assets_folder_path.join(&texture_path.path);
    let blocks = textures_path.join("blocks/");
    block_atlas_handles.handles = load_textures(&asset_server, &blocks);
    block_atlas_handles.animations = read_animations(&blocks);
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

    material_resource.climate_map = images.add(new_climate_map());
    // Made once: the materials of the chunks, made once, keep it
    if !buffers.contains(&material_resource.weather) {
        material_resource.weather = buffers.add(new_weather_buffer());
    }
    let colormaps = textures_path.join("colormap");
    material_resource.grass_colors =
        load_colormap(&asset_server, &mut images, &colormaps.join("grass.png"));
    material_resource.foliage_colors =
        load_colormap(&asset_server, &mut images, &colormaps.join("foliage.png"));
}

/// Starts loading the colour map at `path`, or, for a texture pack without it, makes one of a
/// single texel of `GRASS_COLOR`, which colours every climate alike.
fn load_colormap(
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
    path: &Path,
) -> Handle<Image> {
    if path.is_file() {
        return asset_server.load(path.to_string_lossy().into_owned());
    }
    info!(
        "No colour map {}: the grass is of one colour",
        path.display()
    );
    let [red, green, blue, alpha] = GRASS_COLOR;
    images.add(Image::new_fill(
        Extent3d::default(),
        TextureDimension::D2,
        &Color::linear_rgba(red, green, blue, alpha)
            .to_srgba()
            .to_u8_array(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

/// Starts loading every PNG texture of `folder`, each with the name of its file without the
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
            if path.extension()? != "png" {
                return None;
            }
            // A file whose name is not valid UTF-8 cannot name a block or an item
            let name = path.file_stem()?.to_str()?.to_owned();
            Some((asset_server.load(path.to_string_lossy().into_owned()), name))
        })
        .collect();
    info!(
        "Loading {} textures from {}",
        textures.len(),
        folder.display()
    );
    textures
}

/// The animations of the textures of `folder`, from its `<texture>.anim.ron` files. A file that
/// cannot be read is left out, and its texture plays with the default animation.
fn read_animations(folder: &Path) -> HashMap<String, TextureAnimation> {
    let Ok(files) = fs::read_dir(folder) else {
        return HashMap::new();
    };
    files
        .filter_map(|file| {
            let path = file.ok()?.path();
            let name = path.file_name()?.to_str()?.strip_suffix(".anim.ron")?;
            let animation = fs::read_to_string(&path)
                .map_err(|err| err.to_string())
                .and_then(|text| ron::from_str(&text).map_err(|err| err.to_string()));
            match animation {
                Ok(animation) => Some((name.to_owned(), animation)),
                Err(err) => {
                    warn!("Could not read the animation {}: {err}", path.display());
                    None
                }
            }
        })
        .collect()
}

/// Builds the block textures and the item atlas once all their textures are loaded, and
/// reports in `PreLoadingCompletion` whether both are ready.
pub fn create_all_atlases(
    mut atlases: (ResMut<AtlasHandles<BlockId>>, ResMut<AtlasHandles<ItemId>>),
    mut images: ResMut<Assets<Image>>,
    mut material_resource: ResMut<MaterialResource>,
    mut loading: ResMut<PreLoadingCompletion>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
    graphics: Option<Res<GraphicsSettings>>,
    occlusion: Option<Res<OcclusionMap>>,
) {
    let graphics = graphics.map(|graphics| *graphics).unwrap_or_default();
    let textured = |texture: Option<Handle<Image>>, alpha_mode| StandardMaterial {
        base_color_texture: texture,
        perceptual_roughness: BASE_ROUGHNESS,
        reflectance: BASE_SPECULAR_HIGHLIGHT,
        alpha_mode,
        ..default()
    };

    if material_resource.blocks.is_none() {
        if let Some((blocks, table)) = build_block_textures(&atlases.0, &mut images) {
            // The shader samples the layers itself: the standard material has no texture
            let extension = BlockLayers {
                table,
                layers: blocks.array.clone(),
                climate: material_resource.climate_map.clone(),
                grass_colors: material_resource.grass_colors.clone(),
                foliage_colors: material_resource.foliage_colors.clone(),
                shading: ChunkShading::new(&graphics, false),
                weather: material_resource.weather.clone(),
                occlusion: occlusion
                    .as_ref()
                    .map(|occlusion| occlusion.image.clone())
                    .unwrap_or_default(),
            };
            let chunk_material = |base, liquid| ChunkMaterial {
                base,
                extension: BlockLayers {
                    shading: ChunkShading::new(&graphics, liquid),
                    ..extension.clone()
                },
            };
            material_resource.chunk_materials.insert(
                GlobalMaterial::Blocks,
                chunk_materials.add(chunk_material(
                    textured(None, AlphaMode::AlphaToCoverage),
                    false,
                )),
            );
            // A liquid is seen through, and from below its surface too
            material_resource.chunk_materials.insert(
                GlobalMaterial::Liquids,
                chunk_materials.add(chunk_material(
                    StandardMaterial {
                        cull_mode: None,
                        ..textured(None, AlphaMode::Blend)
                    },
                    true,
                )),
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
                    ..textured(Some(items.texture.clone()), AlphaMode::Mask(0.5))
                }),
            );
            material_resource.items = Some(items);
        }
    }

    loading.textures_loaded =
        material_resource.blocks.is_some() && material_resource.items.is_some();
}

/// The frames of an image, as RGBA bytes each `size` wide and high: a square image is one
/// frame, an image as many times higher as it is wide a vertical strip of frames, and any other
/// image the square at its top. A frame of another size is scaled to `size`, keeping the
/// pixels sharp.
fn frames(image: &Image, size: u32) -> Option<Vec<Vec<u8>>> {
    let converted;
    let image = if image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb {
        image
    } else {
        converted = image.convert(TextureFormat::Rgba8UnormSrgb)?;
        &converted
    };
    let (width, height) = (image.width(), image.height());
    let pixels = image.data.as_ref()?;
    let count = if height % width == 0 {
        height / width
    } else {
        1
    };
    let frames = (0..count)
        .map(|frame| {
            let mut bytes = Vec::with_capacity((size * size * 4) as usize);
            for y in 0..size {
                let source_y = frame * width + y * width / size;
                for x in 0..size {
                    let at = ((source_y * width + x * width / size) * 4) as usize;
                    bytes.extend_from_slice(&pixels[at..at + 4]);
                }
            }
            bytes
        })
        .collect();
    Some(frames)
}

/// The texture array of the blocks and the table of `BlockLayers`, or `None` while some of
/// the textures are still loading. Every layer is as large as the widest texture, and pixel
/// art is sampled without smoothing.
fn build_block_textures(
    handles: &AtlasHandles<BlockId>,
    images: &mut Assets<Image>,
) -> Option<(BlockTextures, [UVec4; MAX_TILES])> {
    let loaded: Vec<(&Image, &String)> = handles
        .handles
        .iter()
        .map(|(handle, name)| Some((images.get(handle.id())?, name)))
        .collect::<Option<_>>()?;
    if loaded.len() > MAX_TILES {
        error!(
            "{} block textures, but no more than {MAX_TILES} can be drawn",
            loaded.len()
        );
    }
    let size = loaded
        .iter()
        .map(|(image, _)| image.width())
        .max()
        .unwrap_or(1);

    let mut bytes = Vec::new();
    let mut layers = 0;
    let mut tiles = HashMap::new();
    let mut table = [UVec4::ZERO; MAX_TILES];
    for (image, name) in loaded.into_iter().take(MAX_TILES) {
        let Some(frames) = frames(image, size) else {
            error!("Could not read the texture {name}");
            continue;
        };
        let animation = handles.animations.get(name).copied().unwrap_or_default();
        table[tiles.len()] =
            table_row(layers, frames.len() as u32, animation, WetSurface::of(name));
        tiles.insert(name.clone(), tiles.len() as u32);
        layers += frames.len() as u32;
        bytes.extend(frames.into_iter().flatten());
    }
    if layers == 0 {
        error_once!("No block texture could be read");
        return None;
    }

    let mut array = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // An array of one layer would otherwise be seen as a plain texture
    array.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    // A face that spans several blocks repeats its texture
    array.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::nearest()
    });
    info!(
        "{} block textures in {layers} layers of {size}x{size}",
        tiles.len()
    );
    Some((
        BlockTextures {
            handles: handles
                .handles
                .iter()
                .map(|(handle, name)| (name.clone(), handle.clone()))
                .collect(),
            array: images.add(array),
            tiles,
        },
        table,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A `width` by `height` image whose pixel at `(x, y)` is `[x, y, 0, 255]`.
    fn numbered(width: u32, height: u32) -> Image {
        let bytes = (0..height)
            .flat_map(|y| (0..width).flat_map(move |x| [x as u8, y as u8, 0, 255]))
            .collect();
        Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            bytes,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        )
    }

    fn pixel(frame: &[u8], size: u32, x: u32, y: u32) -> [u8; 2] {
        let at = ((y * size + x) * 4) as usize;
        [frame[at], frame[at + 1]]
    }

    #[test]
    fn every_item_has_an_icon_among_the_base_textures() {
        // Its own, or the texture of the block of the same name
        let textures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/graphics/base_textures");
        for item in ItemId::ALL {
            let has_icon = ["items", "blocks"]
                .iter()
                .any(|folder| textures.join(format!("{folder}/{item:?}.png")).is_file());
            assert!(has_icon, "{item:?}");
        }
    }

    #[test]
    fn each_block_texture_gets_wet_as_its_surface_does() {
        for (name, surface) in [
            ("Stone", WetSurface::Hard),
            ("Cobblestone", WetSurface::Hard),
            ("Gravel", WetSurface::Hard),
            ("OakPlanks", WetSurface::Hard),
            ("OakLogTop", WetSurface::Hard),
            ("GrassTop", WetSurface::Grass),
            ("Grass", WetSurface::Grass),
            ("Dirt", WetSurface::Soil),
            ("Moss", WetSurface::Grass),
            ("Sand", WetSurface::Porous),
            ("OakLeaves", WetSurface::Foliage),
            ("TallGrass", WetSurface::Foliage),
            ("Poppy", WetSurface::Foliage),
            ("Cactus", WetSurface::Foliage),
            ("Glass", WetSurface::Dry),
            ("Ice", WetSurface::Dry),
            ("Snow", WetSurface::Dry),
            ("Torch", WetSurface::Dry),
            ("DestroyStage4", WetSurface::Dry),
            ("Water", WetSurface::Dry),
            // a texture of a pack unknown here, as stone
            ("Marble", WetSurface::Hard),
        ] {
            assert_eq!(WetSurface::of(name), surface, "{name}");
        }
    }

    #[test]
    fn a_row_of_the_table_holds_the_surface_over_the_fading_of_the_frames() {
        let fading = TextureAnimation {
            frame_time: 10,
            interpolate: true,
        };
        let row = table_row(7, 3, fading, WetSurface::Porous);
        assert_eq!(row.truncate(), UVec3::new(7, 3, 10));
        assert_eq!(row.w & 1, 1);
        assert_eq!(row.w >> 1, WetSurface::Porous as u32);
        let row = table_row(0, 1, TextureAnimation::default(), WetSurface::Dry);
        assert_eq!(row.w & 1, 0);
        assert_eq!(row.w >> 1, WetSurface::Dry as u32);
    }

    #[test]
    fn a_strip_of_square_frames_is_cut_into_its_frames() {
        let frames = frames(&numbered(4, 12), 4).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(pixel(&frames[0], 4, 1, 2), [1, 2]);
        assert_eq!(pixel(&frames[2], 4, 3, 1), [3, 9]);
    }

    #[test]
    fn a_smaller_texture_is_scaled_up_without_smoothing() {
        let frames = frames(&numbered(2, 2), 4).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(pixel(&frames[0], 4, 1, 1), [0, 0]);
        assert_eq!(pixel(&frames[0], 4, 2, 3), [1, 1]);
    }

    #[test]
    fn an_image_neither_square_nor_a_strip_shows_its_top() {
        let frames = frames(&numbered(4, 6), 4).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(pixel(&frames[0], 4, 0, 3), [0, 3]);
    }

    #[test]
    fn an_animation_without_a_setting_takes_the_default_one() {
        let animation: TextureAnimation = ron::from_str("(interpolate: true)").unwrap();
        assert_eq!(
            animation,
            TextureAnimation {
                frame_time: 1,
                interpolate: true
            }
        );
    }

    #[test]
    fn the_water_of_the_base_textures_fades_from_frame_to_frame() {
        let blocks = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../data")
            .join(crate::constants::TEXTURE_PATH_BASE)
            .join("blocks");
        assert_eq!(
            read_animations(&blocks).get("Water"),
            Some(&TextureAnimation {
                frame_time: 10,
                interpolate: true
            })
        );
    }

    #[test]
    fn the_animations_are_read_from_their_files_and_a_broken_one_is_left_out() {
        let folder = std::env::temp_dir().join(format!("animations-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("Water.anim.ron"), "(frame_time: 10)").unwrap();
        fs::write(folder.join("Lava.anim.ron"), "(frame_time: \"slow\")").unwrap();
        fs::write(folder.join("Water.png"), "").unwrap();
        let animations = read_animations(&folder);
        fs::remove_dir_all(&folder).unwrap();
        assert_eq!(
            animations,
            HashMap::from([(
                "Water".to_string(),
                TextureAnimation {
                    frame_time: 10,
                    interpolate: false
                }
            )])
        );
    }
}
