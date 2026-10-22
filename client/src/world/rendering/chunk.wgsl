// The chunks are drawn with the block textures, the layers of a texture array: `uv_b.x` is the
// index of the texture of a face in `block_textures`, and `uv` the place in it. A face that spans
// several blocks has its `uv` go past 1, and the texture repeats across it. `uv_b.y` says how the
// face is coloured and how it sways (`Colouring::code` in `meshing.rs`): the colour map that
// colours it from the climate of its column, if any (`BiomeTint`), the colour it takes otherwise
// (`FaceColor`), the sway (`Sway`), and one more than the index of the texture drawn over the
// face, if any: the colour then goes to that overlay only. Both come as fractions of 65535 (`TILE`
// in `meshing.rs`).
//
// The vertex colour is the light of the vertex (`LIGHT` in `meshing.rs`): the light levels of
// the sky and of the blocks, and the ambient occlusion. The light of the sky dims what Bevy's
// sun, moon and ambient light give, so that a cave is dark, and the light of the blocks adds
// to it.

#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::alpha_discard
#import bevy_pbr::{mesh_functions, view_transformations::position_world_to_clip}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{Vertex, VertexOutput, FragmentOutput},
    pbr_bindings,
    pbr_prepass_functions,
    pbr_functions::prepare_world_normal,
    pbr_types,
}
#import bevy_render::globals::Globals
// Bound to the prepass too, where `mesh_view_bindings` has other bindings
@group(0) @binding(1) var<uniform> globals: Globals;
#else
#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    mesh_view_bindings::{
        globals, lights, view, view_transmission_texture, view_transmission_sampler,
    },
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    shadows::fetch_directional_shadow,
    view_transformations::{
        position_world_to_ndc, position_world_to_view, position_ndc_to_world, ndc_to_uv,
        uv_to_ndc, depth_ndc_to_view_z,
    },
}
#ifdef DISTANCE_FOG
#import bevy_pbr::mesh_view_bindings::fog
#endif
#import rustcraft::sky::{sky_radiance, has_sky, horizon_direction, overcast_sky, weather}
#import rustcraft::weather::FLASH_COLOR
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif
#endif

// For each texture: its first layer, its number of frames, how long a frame is shown in
// twentieths of a second, and whether a frame fades into the next in the lowest bit of `w`,
// with how its surface takes the rain over it (`WetSurface`). Keep the length in sync with
// `MAX_TILES`.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> block_textures: array<vec4<u32>, 256>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var block_layers: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var block_sampler: sampler;
// The climate of the columns around the player, wrapped around (`climate.rs`): their place on
// the colour maps from the bottom right corner, the temperature in red and the downfall times
// the temperature in green, blended across biomes, what falls on it in blue (`PrecipitationKind`)
// and an alpha of 0 where it is not known yet.
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var climate_map: texture_2d<f32>;
// The colours of the grass and of the leaves by climate, read as Minecraft reads its own.
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var grass_colors: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var foliage_colors: texture_2d<f32>;

// `ChunkShading` in `materials.rs`
struct ChunkShading {
    // The colours of `FaceColor`, by index
    palette: array<vec4<f32>, 4>,
    // From 0 to 1, how much the dim light is brightened
    brightness: f32,
    // How bright the full light of the blocks is, as an ambient light
    block_light: f32,
    // `EFFECT_SWAY` and `EFFECT_WATER`
    effects: u32,
    // 1 for the material of the liquids
    liquid: u32,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var<uniform> shading: ChunkShading;
// At 107, the weather of the sky, `weather` of `rustcraft::sky`
// The height at which the rain stops in each column around the camera, wrapped around
// (`occlusion.rs`)
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var occlusion_map: texture_2d<f32>;

// The values of `BiomeTint::index`
const BIOME_TINT_GRASS: u32 = 1u;
const BIOME_TINT_FOLIAGE: u32 = 2u;
// The values of `Sway::index`
const SWAY_WHOLE: u32 = 1u;
const SWAY_TOP: u32 = 2u;
// The bits of `ChunkShading::effects`
const EFFECT_SWAY: u32 = 1u;
const EFFECT_WATER: u32 = 2u;
// `EFFECT_SSR` in `shaders.rs`: the water reflects the screen and refracts what lies under it
const EFFECT_SSR: u32 = 4u;
// The highest light level, `MAX_LIGHT`
const MAX_LIGHT: f32 = 15.0;
// The colour of the light of the blocks, as of a flame
const BLOCK_LIGHT_COLOR: vec3<f32> = vec3(1.0, 0.82, 0.6);
// The colour of the sky, `SKY_COLOR` in `underwater.rs`, in linear values: what the water
// reflects
const SKY_COLOR: vec3<f32> = vec3(0.178, 0.346, 0.474);
// How much of its blue the water loses under a full overcast
const WATER_GREYING: f32 = 0.6;
// The brightness of the ambient light at noon, `DAY_AMBIENT` in `graphics.rs`: when the water
// reflects the sky as bright as it is drawn
const DAY_AMBIENT: f32 = 400.0;
// The rings the rain makes on the water: the side of the cell of each, in blocks, how long one
// lives, in seconds, and how far it spreads, in blocks
const RIPPLE_CELL: f32 = 0.55;
const RIPPLE_LIFE: f32 = 0.8;
const RIPPLE_REACH: f32 = 0.3;
// `SNOW_LINE` of the server's generation: above it, the rain is snow
const SNOW_LINE: f32 = 122.0;
// The values of `WetSurface` in `materials.rs`, in the bits of `w` over the first in the
// table of the textures
const SURFACE_SOIL: u32 = 1u;
const SURFACE_POROUS: u32 = 2u;
const SURFACE_FOLIAGE: u32 = 3u;
const SURFACE_DRY: u32 = 4u;
const SURFACE_GRASS: u32 = 5u;
// How far from the camera, along x or z, the occlusion map surely holds the columns: the chunk
// columns within `REACH` - 1 of the camera's
const MAP_REACH: f32 = 48.0;
// How much of the light a wet surface and the water of a puddle reflect, seen from above
const WET_F0: f32 = 0.02;
// The width of the hollows the puddles gather in, in blocks, and the most of the ground they
// cover, at the most for the hard surfaces
const PUDDLE_SCALE: f32 = 2.8;
const PUDDLE_COVER: f32 = 0.42;
// How much darker the ground is seen through the water of a puddle
const PUDDLE_DARKEN: f32 = 0.55;
// The exposure the lights of the game were made for, `Exposure::BLENDER` (an EV100 of 9.7):
// the light of the blocks is as bright on screen whatever the exposure of the shaders
const BASE_EXPOSURE: f32 = 1.0 / (1.2 * 831.7464);

// How long the game has been drawing, in seconds, which plays the animated textures and sways
// the plants.
fn seconds() -> f32 {
    return globals.time;
}

// How a face is coloured and how it sways, from `uv_b.y`.
struct Colouring {
    biome_tint: u32,
    color: u32,
    sway: u32,
    // One more than the overlay, 0 for none
    overlay: u32,
}

// The index in `uv_b`, from a fraction of 65535. It is rounded, as interpolating it may stray
// from it, if by little.
fn index_of(fraction: f32) -> u32 {
    return u32(round(fraction * 65535.0));
}

fn colouring_of(uv_b: vec2<f32>) -> Colouring {
    let code = index_of(uv_b.y);
    return Colouring(code & 3u, (code >> 2u) & 3u, (code >> 4u) & 3u, code >> 6u);
}

// How far the point `at` of a face that sways as `sway` moves in the wind, in blocks: the waves
// cross the world in gusts, alike at a point whatever face it is a corner of, so that faces that
// meet never part. A plant sways by its top, the top of its texture, rooted below.
fn sway_offset(sway: u32, uv: vec2<f32>, at: vec3<f32>) -> vec3<f32> {
    return sway_offset_at(sway, uv, at, seconds());
}

// `sway_offset` at the time `time`: the motion vectors compare it with the frame before.
fn sway_offset_at(sway: u32, uv: vec2<f32>, at: vec3<f32>, time: f32) -> vec3<f32> {
    if sway == 0u || (shading.effects & EFFECT_SWAY) == 0u {
        return vec3(0.0);
    }
    let gust = 0.6 + 0.4 * sin(time * 0.37 + (at.x + at.z) * 0.031);
    let wave = vec2(
        sin(time * 1.7 + at.x * 0.37 + at.y * 0.21 + at.z * 0.13),
        sin(time * 1.3 + at.z * 0.37 + at.y * 0.17 + at.x * 0.19),
    );
    var reach = 0.04;
    if sway == SWAY_TOP {
        reach = select(0.0, 0.12, uv.y < 0.5);
    }
    return vec3(wave.x, 0.0, wave.y) * reach * gust;
}

// What `bevy_pbr::mesh` and `bevy_pbr::prepass` do, for meshes that are neither skinned nor
// morphed, with the plants swaying: the shadows sway with them.
@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
#ifdef VERTEX_UVS_A
#ifdef VERTEX_UVS_B
    let sway = colouring_of(vertex.uv_b).sway;
    world_position = vec4(world_position.xyz + sway_offset(sway, vertex.uv, world_position.xyz), 1.0);
#endif
#endif
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef PREPASS_PIPELINE
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(
        mesh_functions::get_previous_world_from_local(vertex.instance_index),
        vec4<f32>(vertex.position, 1.0),
    );
#ifdef VERTEX_UVS_A
#ifdef VERTEX_UVS_B
    // Where the plant swayed to in the frame before, for TAA
    let previous_sway = sway_offset_at(
        sway, vertex.uv, out.previous_world_position.xyz, seconds() - globals.delta_time,
    );
    out.previous_world_position = vec4(out.previous_world_position.xyz + previous_sway, 1.0);
#endif
#endif
#endif
#else
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index,
        world_from_local[3],
    );
#endif
    return out;
}

// The colour of the block texture `index` at `uv`, at its current frame. The layers have no
// mipmaps: sampling the first level is the same, and allowed where the control flow differs
// between fragments.
fn texture_color(index: u32, uv: vec2<f32>) -> vec4<f32> {
    let texture = block_textures[index];
    let frames = max(texture.y, 1u);
    // In frames since the start, at 20 frame times a second
    let progress = seconds() * 20.0 / f32(max(texture.z, 1u));
    let frame = u32(progress) % frames;
    let color = textureSampleLevel(block_layers, block_sampler, uv, texture.x + frame, 0.0);
    if (texture.w & 1u) == 0u || frames == 1u {
        return color;
    }
    let next = textureSampleLevel(block_layers, block_sampler, uv, texture.x + (frame + 1u) % frames, 0.0);
    return mix(color, next, fract(progress));
}

// The colour of the block texture of the face at `in`.
fn block_color(in: VertexOutput) -> vec4<f32> {
    return texture_color(index_of(in.uv_b.x), in.uv);
}

#ifdef PREPASS_PIPELINE

// The cutoff of `bevy_pbr::pbr_prepass_functions` for the blended alpha modes.
const PREMULTIPLIED_ALPHA_CUTOFF: f32 = 0.05;

// What `prepass_alpha_discard` does, with the colour of the block texture: the standard
// material has no texture of its own.
fn block_alpha_discard(in: VertexOutput) {
#ifdef MAY_DISCARD
    let alpha = pbr_bindings::material.base_color.a * block_color(in).a;
    let alpha_mode = pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    // Alpha to coverage falls back to the mask without MSAA, as in `alpha_discard`
    if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK
        || alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ALPHA_TO_COVERAGE {
        if alpha < pbr_bindings::material.alpha_cutoff {
            discard;
        }
    } else if alpha < PREMULTIPLIED_ALPHA_CUTOFF {
        discard;
    }
#endif
}

// What `bevy_pbr::pbr_prepass` does, with the block textures
#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    block_alpha_discard(in);

    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
    out.normal = vec4(prepare_world_normal(in.world_normal, false, is_front) * 0.5 + vec3(0.5), 1.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(
        in.world_position,
        in.previous_world_position,
    );
#endif
    return out;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    block_alpha_discard(in);
}
#endif

#else

// The colour at `at` of a colour map, from (0, 0) at its top left to (1, 1) at its bottom right.
fn colormap_color(colormap: texture_2d<f32>, at: vec2<f32>) -> vec3<f32> {
    let last = vec2<f32>(textureDimensions(colormap) - 1u);
    return textureLoad(colormap, vec2<i32>(round(at * last)), 0).rgb;
}

// The colour of the grass or of the leaves at `in`, from the climate of its column with the
// colour map `tint`, or `color` where the face has none or the climate is not known yet.
fn biome_color(in: VertexOutput, tint: u32, color: vec3<f32>) -> vec3<f32> {
    if tint != BIOME_TINT_GRASS && tint != BIOME_TINT_FOLIAGE {
        return color;
    }
    // A point of a face on the side of a block is on the edge of its column: a little back
    // from the face, it is in the block
    let column = floor(in.world_position.xz - in.world_normal.xz * 0.01);
    let size = vec2<f32>(textureDimensions(climate_map));
    // Wrapped with `floor`: `%` on negative integers reads the wrong texels on some GPUs
    let texel = vec2<i32>(column - size * floor(column / size));
    let climate = textureLoad(climate_map, texel, 0);
    if climate.a == 0.0 {
        return color;
    }
    // As Minecraft: the hotter to the left, and up as the downfall grows, the more so the
    // hotter the climate
    let at = vec2(1.0 - climate.r, 1.0 - climate.g);
    if tint == BIOME_TINT_GRASS {
        return colormap_color(grass_colors, at);
    }
    return colormap_color(foliage_colors, at);
}

// How bright a light level looks, from 0 to 1, as Minecraft's: it falls slowly from the full
// level, then fast, brightened by `shading.brightness`, and never quite black.
fn level_brightness(level: f32) -> f32 {
    let fraction = clamp(level / MAX_LIGHT, 0.0, 1.0);
    let dim = fraction / (4.0 - 3.0 * fraction);
    let bright = 1.0 - pow(1.0 - dim, 4.0);
    return 0.03 + 0.97 * mix(dim, bright, shading.brightness);
}

// The normal of the surface of the water at `at`: small waves that cross it in three
// directions, the sum of their slopes, and the rings of the rain, as much as `rain` says.
// `footprint` is how many blocks a pixel spans there: the waves too short to be drawn fade out,
// instead of making stripes far off.
fn water_normal(at: vec2<f32>, footprint: f32, rain: f32) -> vec3<f32> {
    let time = seconds();
    var slope = rain_ripples(at, rain, footprint);
    let waves = array(
        vec4(0.8, 0.6, 1.9, 1.1),
        vec4(-0.5, 0.87, 2.7, 1.6),
        vec4(0.2, -0.98, 4.3, 2.3),
    );
    for (var i = 0; i < 3; i++) {
        let wave = waves[i];
        let phase = dot(at, wave.xy) * wave.z + time * wave.w;
        // Gone once the phase turns by a radian from a pixel to the next
        let fade = 1.0 - smoothstep(0.25, 1.0, footprint * wave.z);
        slope += wave.xy * wave.z * cos(phase) * 0.012 * fade;
    }
    return normalize(vec3(-slope.x, 1.0, -slope.y));
}

// Four numbers from 0 to 1 for `cell`, all different from those of the cells around it.
fn hash4(cell: vec2<f32>) -> vec4<f32> {
    var v = vec4<u32>(vec2<u32>(vec2<i32>(cell)), 0x9e3779b9u, 0x7f4a7c15u);
    v = v * 1664525u + 1013904223u;
    v.x += v.y * v.w;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v.w += v.y * v.z;
    v ^= v >> vec4(16u);
    v.x += v.y * v.w;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v.w += v.y * v.z;
    return vec4<f32>(v >> vec4(8u)) / 16777216.0;
}

// The slope of the rings the drops of rain make on the water at `at`. In each cell of a grid
// a ring grows from a point and fades, at a pace of its own, then starts again elsewhere;
// `amount`, from 0 to 1, is the share of the cells that hold one. The rings fade out where a
// pixel spans too much of them to draw them, as the waves.
fn rain_ripples(at: vec2<f32>, amount: f32, footprint: f32) -> vec2<f32> {
    let fade = 1.0 - smoothstep(0.02, 0.06, footprint);
    if amount <= 0.0 || fade <= 0.0 {
        return vec2(0.0);
    }
    let time = seconds();
    let base = floor(at / RIPPLE_CELL);
    var slope = vec2(0.0);
    for (var dx = -1; dx <= 1; dx++) {
        for (var dz = -1; dz <= 1; dz++) {
            let cell = base + vec2(f32(dx), f32(dz));
            let pace = hash4(cell);
            let cycle = time / (RIPPLE_LIFE * mix(0.8, 1.25, pace.x)) + pace.y;
            let ring = hash4(cell * 7.0 + floor(cycle) * vec2(13.0, 101.0));
            if ring.w > amount {
                continue;
            }
            let age = fract(cycle);
            let centre = (cell + 0.15 + 0.7 * ring.xy) * RIPPLE_CELL;
            let off = at - centre;
            let dist = max(length(off), 1e-4);
            // A crest and a trough round the ring, which widens and flattens as it spreads
            let x = dist - RIPPLE_REACH * sqrt(age);
            let width = mix(0.03, 0.07, age);
            let k = 6.2832 / (2.5 * width);
            let envelope = exp(-x * x / (width * width));
            let rise = (k * cos(k * x) - 2.0 * x / (width * width) * sin(k * x)) * envelope;
            let strength = 0.005 * (1.0 - age) * (1.0 - age) * mix(0.6, 1.0, ring.z);
            slope += off / dist * rise * strength;
        }
    }
    return slope * fade;
}

// How much it rains on the water at `at`, open to the sky with `sky_level` from 0 to 1: as
// much as falls, where the climate of the column gives rain and below the snow line.
fn rain_on_water(at: vec3<f32>, sky_level: f32) -> f32 {
    let column = floor(at.xz);
    let size = vec2<f32>(textureDimensions(climate_map));
    let climate = textureLoad(climate_map, vec2<i32>(column - size * floor(column / size)), 0);
    // The blue channel holds the kind of what falls: rain is 0
    if climate.a == 0.0 || climate.b > 0.5 / 255.0 || at.y > SNOW_LINE {
        return 0.0;
    }
    return weather.rain * smoothstep(14.0 / MAX_LIGHT, 1.0, sky_level);
}

// How a surface takes the rain once soaked (`WetSurface`): what its albedo is multiplied by,
// its perceptual roughness, how strong its sheen is, and how much of the most the puddles
// cover they may cover on it.
fn wet_response(surface: u32) -> vec4<f32> {
    switch surface {
        case SURFACE_SOIL: {
            return vec4(0.68, 0.45, 0.2, 0.6);
        }
        case SURFACE_POROUS: {
            return vec4(0.55, 0.6, 0.08, 0.0);
        }
        case SURFACE_GRASS: {
            return vec4(0.72, 0.45, 0.15, 0.0);
        }
        case SURFACE_FOLIAGE: {
            return vec4(0.8, 0.4, 0.25, 0.0);
        }
        default: {
            return vec4(0.62, 0.3, 0.4, 1.0);
        }
    }
}

// The height at which the rain stops in the column `column` (`occlusion.rs`).
fn stop_height(column: vec2<f32>) -> f32 {
    let size = vec2<f32>(textureDimensions(occlusion_map));
    return textureLoad(occlusion_map, vec2<i32>(column - size * floor(column / size)), 0).r;
}

// How much of what falls in the column `column` at the height `y` is rain, as its climate
// says: none in the snow, the deserts and the columns whose climate is not known yet, and less
// across the band of the snow line, as for the drops.
fn rain_climate(column: vec2<f32>, y: f32) -> f32 {
    let size = vec2<f32>(textureDimensions(climate_map));
    let climate = textureLoad(climate_map, vec2<i32>(column - size * floor(column / size)), 0);
    // The blue channel holds the kind of what falls: rain is 0
    if climate.a == 0.0 || climate.b > 0.5 / 255.0 {
        return 0.0;
    }
    return 1.0 - smoothstep(SNOW_LINE - 4.0, SNOW_LINE + 4.0, y);
}

// How much rain falls on the face at `at` of normal `normal`, from 0 to 1, with `sky_level`
// the light of the sky there, from 0 to 1. A top face gets it where nothing stops the rain
// above it, a side face where the rain falls past it, in the column in front of it, at its
// height; a bottom face never. Past the columns the occlusion map surely holds, the full light
// of the sky stands for it.
fn rain_exposure(at: vec3<f32>, normal: vec3<f32>, sky_level: f32) -> f32 {
    if normal.y < -0.5 {
        return 0.0;
    }
    let climate = rain_climate(floor(at.xz - normal.xz * 0.01), at.y);
    if climate <= 0.0 {
        return 0.0;
    }
    let open = select(0.0, 1.0, at.y >= stop_height(floor(at.xz + normal.xz * 0.5)) - 0.02);
    let reach = max(abs(at.x - view.world_position.x), abs(at.z - view.world_position.z));
    let open_sky = smoothstep(14.0 / MAX_LIGHT, 1.0, sky_level);
    return climate * mix(open, open_sky, smoothstep(MAP_REACH - 8.0, MAP_REACH, reach));
}

// Smooth noise from 0 to 1 over the plane, of features about a unit wide.
fn value_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = p - cell;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash4(cell).x;
    let b = hash4(cell + vec2(1.0, 0.0)).x;
    let c = hash4(cell + vec2(0.0, 1.0)).x;
    let d = hash4(cell + vec2(1.0, 1.0)).x;
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// How deep a hollow the ground at `at` lies in, from 0 to 1: the puddles gather in the
// deepest first, and the ground there dries last. The finer part fades out where a pixel spans
// too much of it, `footprint` blocks.
fn hollow(at: vec2<f32>, footprint: f32) -> f32 {
    let broad = value_noise(at / PUDDLE_SCALE);
    let turned = vec2(at.x * 0.8 - at.y * 0.6, at.x * 0.6 + at.y * 0.8);
    let fine = value_noise(turned / (PUDDLE_SCALE * 0.37) + 17.3);
    let sum = broad * 0.65 + mix(fine, 0.5, smoothstep(0.1, 0.5, footprint)) * 0.35;
    // Spread out about evenly from 0 to 1, so that a share of the ground is that much of it
    return smoothstep(0.22, 0.78, sum);
}

// How low the texel of the face at `in` lies in its block texture, from -1 to 1: the darker
// texels of the pixel art, the cracks between the stones, are taken as lower than those around
// them, so that the water fills them first and the edges of the puddles follow the pixels.
fn texel_relief(in: VertexOutput) -> f32 {
    let index = index_of(in.uv_b.x);
    let luma = vec3(0.3, 0.59, 0.11);
    let here = dot(texture_color(index, in.uv).rgb, luma);
    var around = 0.0;
    for (var i = 0; i < 4; i++) {
        let angle = f32(i) * 1.5708 + 0.4;
        around += dot(texture_color(index, in.uv + vec2(cos(angle), sin(angle)) * 0.19).rgb, luma);
    }
    let mean = max(around * 0.25, 1e-3);
    return clamp((mean - here) / mean, -1.0, 1.0);
}

// How far into the top face at `at` a puddle may reach: not up to the edges where the ground
// drops, as the water would run off there.
fn held_water(at: vec3<f32>) -> f32 {
    let column = floor(at.xz);
    let inner = at.xz - column;
    let sides = array(vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0));
    var held = 1.0;
    for (var i = 0; i < 4; i++) {
        let side = sides[i];
        if stop_height(column + side) < at.y - 0.5 {
            let toward = select(inner, 1.0 - inner, side > vec2(0.0));
            held = min(held, smoothstep(0.04, 0.3, dot(toward, abs(side))));
        }
    }
    return held;
}

// The streaks the rain runs down the side of a block in, from 0 to 1, at `at` on the face of
// normal `normal`: columns of texels, some wetter than others, wettest under the top.
fn rain_streaks(at: vec3<f32>, normal: vec3<f32>) -> f32 {
    let along = dot(at.xz, vec2(-normal.z, normal.x));
    let texel = floor(along * 16.0) / 16.0;
    let streak = value_noise(vec2(texel * 7.0, at.y * 0.9));
    let under_top = stop_height(floor(at.xz - normal.xz * 0.01)) - at.y;
    let near_top = 1.0 - smoothstep(0.0, 2.0, under_top);
    return mix(0.35, 0.75, streak) + 0.25 * near_top;
}

// How the rain wets a face.
struct Wet {
    // From 0 to 1, how wet the surface is
    amount: f32,
    // From 0 to 1, how much of a puddle covers it, and how deep it is there, from its edge
    puddle: f32,
    deep: f32,
    // `texel_relief` where the texels are drawn larger than a pixel, 0 elsewhere
    relief: f32,
    // `wet_response` of its surface
    response: vec4<f32>,
    // The normal of the water of a puddle, rippled by the rain
    normal: vec3<f32>,
}

// How the rain wets the face at `in` of normal `normal`, its surface `surface` and the light
// of the sky there `sky_level`, from 0 to 1. While it rains, the ground is wet all over; as it
// dries, it dries from the high spots, so the hollows stay wet longest, and puddles fill them
// as the wetness passes half.
fn wet_surface(in: VertexOutput, normal: vec3<f32>, sky_level: f32, surface: u32) -> Wet {
    // In uniform control flow, before anything is left out
    let footprint = length(fwidth(in.world_position.xz));
    var wet = Wet(0.0, 0.0, 0.0, 0.0, vec4(1.0, 1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0));
    if surface == SURFACE_DRY {
        return wet;
    }
    let exposure = rain_exposure(in.world_position.xyz, normal, sky_level);
    if exposure <= 0.0 {
        return wet;
    }
    wet.response = wet_response(surface);
    let wetness = weather.wetness;
    let falling = weather.rain;
    let top = normal.y > 0.5;
    var depth = 0.5;
    if top {
        depth = hollow(in.world_position.xz, footprint);
        // The texels count where they are drawn larger than a pixel
        let texels = 1.0 - smoothstep(0.03, 0.1, footprint);
        if texels > 0.0 {
            wet.relief = texel_relief(in) * texels;
            depth += wet.relief * 0.2;
        }
    }
    let drying = (1.0 - depth) * 0.6 * (1.0 - falling);
    wet.amount = smoothstep(0.0, 1.0, saturate((wetness - drying) / 0.4)) * exposure;
    if !top {
        wet.amount *= rain_streaks(in.world_position.xyz, normal);
        return wet;
    }
    if wet.response.w <= 0.0 {
        return wet;
    }
    // The puddles fill as the wetness passes half, and linger as the ground dries
    let filling = mix(smoothstep(0.1, 0.7, wetness), smoothstep(0.5, 1.0, wetness), falling);
    let cover = filling * wet.response.w * PUDDLE_COVER;
    let edge = 0.008 + footprint * 0.25;
    let reach = max(
        abs(in.world_position.x - view.world_position.x),
        abs(in.world_position.z - view.world_position.z),
    );
    var held = 1.0;
    if reach < MAP_REACH {
        held = held_water(in.world_position.xyz);
    }
    wet.puddle = smoothstep(1.0 - cover - edge, 1.0 - cover + edge, depth)
        * smoothstep(0.0, 0.05, cover) * held * exposure;
    wet.deep = smoothstep(1.0 - cover, 1.0 - cover + 0.15, depth);
    if (shading.effects & EFFECT_WATER) != 0u {
        let slope = rain_ripples(in.world_position.xz, falling * exposure, footprint);
        wet.normal = normalize(vec3(-slope.x, 1.0, -slope.y));
    }
    return wet;
}

// The light of the sun, or of the moon, that a surface of normal `N` at `in` reflects toward
// the eye along `V`, as a lobe of `shininess`, shadowed, on screen.
fn sun_lobe(in: VertexOutput, N: vec3<f32>, V: vec3<f32>, shininess: f32) -> vec3<f32> {
    if lights.n_directional_lights == 0u {
        return vec3(0.0);
    }
    let light = lights.directional_lights[0];
    let L = light.direction_to_light;
    let H = normalize(L + V);
    let lobe = (shininess + 8.0) / 25.13 * pow(max(dot(N, H), 0.0), shininess) * max(dot(N, L), 0.0);
    var shadow = 1.0;
    if (light.flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
        let view_z = position_world_to_view(in.world_position.xyz).z;
        shadow = fetch_directional_shadow(0u, in.world_position, N, view_z, in.position.xy);
    }
    return light.color.rgb * view.exposure * lobe * shadow;
}

// How bright the light of a flash of lightning is on screen: from the flash in the clouds, on
// the faces turned toward it, and from the bolt itself, very bright on what is near it and
// falling off over `STRIKE_REACH` blocks.
const FLASH_TOWARD: f32 = 0.45;
const STRIKE_LIGHT: f32 = 3.0;
const STRIKE_REACH: f32 = 9.0;

// The light of a flash of lightning on the face at `at` of normal `N`, beyond the even light
// of the flash, which the ambient light gives. The bolt is taken as a line straight up from the
// block it strikes.
fn flash_light(at: vec3<f32>, N: vec3<f32>) -> vec3<f32> {
    let channel = weather.strike.w;
    if weather.flash <= 0.0 && channel <= 0.0 {
        return vec3(0.0);
    }
    let foot = weather.strike.xyz;
    let to = vec3(foot.x, clamp(at.y, foot.y, foot.y + 120.0), foot.z) - at;
    let near = dot(to, to);
    let L = to * inverseSqrt(max(near, 1e-4));
    let bolt = channel * STRIKE_LIGHT / (1.0 + near / (STRIKE_REACH * STRIKE_REACH));
    let toward = weather.flash * FLASH_TOWARD * max(dot(N, weather.flash_dir.xyz), 0.0);
    return FLASH_COLOR * (bolt * (0.25 + 0.75 * max(dot(N, L), 0.0)) + toward);
}

// The face at `in` of normal `N`, lit as `lit`, as wet as `wet` says, seen along `V`, with
// `sky_level` the light of the sky there: a sheen of the sky, blurred by the roughness, the
// stronger the more the face is seen from the side, and of the sun; in a puddle, the ground
// seen through the water and a near mirror of the sky, rippled by the rain. The sky
// reflected brightens with a flash of lightning (`overcast_sky`).
fn wet_color(
    in: VertexOutput,
    N: vec3<f32>,
    V: vec3<f32>,
    wet: Wet,
    sky_level: f32,
    lit: vec3<f32>,
) -> vec3<f32> {
    let roughness = wet.response.y;
    let facing = max(dot(N, V), 0.0);
    let gloss = 1.0 - roughness;
    let fresnel = WET_F0 + (max(gloss, WET_F0) - WET_F0) * pow(1.0 - facing, 5.0);
    let R = reflect(-V, N);
    // What lies under the horizon is the ground, not the sky
    let above = smoothstep(-0.05, 0.15, R.y);
    let sky = sky_radiance(R, roughness) * sky_level * above;
    let alpha = roughness * roughness;
    let shininess = clamp(2.0 / (alpha * alpha) - 2.0, 8.0, 400.0);
    let sun = sun_lobe(in, N, V, shininess) * sky_level;
    // The water lies in the cracks between the stones, and the tops of the stones dry first:
    // the sheen follows the pixels
    let sheen = fresnel * wet.response.z * wet.amount * clamp(1.0 + 1.5 * wet.relief, 0.2, 2.0);
    var color = mix(lit, sky, sheen) + sun * sheen;
    if wet.puddle > 0.0 {
        let water_facing = max(dot(wet.normal, V), 0.0);
        let water_fresnel = WET_F0 + (1.0 - WET_F0) * pow(1.0 - water_facing, 5.0);
        let mirror = sky_radiance(reflect(-V, wet.normal), 0.0) * sky_level;
        let glint = sun_lobe(in, normalize(mix(N, wet.normal, 0.35)), V, 900.0) * sky_level;
        // The shallow edge shows more of the ground than the middle
        let reflected = water_fresnel * mix(0.65, 1.0, wet.deep);
        let water = mix(lit, mirror, reflected) + glint * reflected;
        color = mix(color, water, wet.puddle);
    }
    return color;
}

#ifdef DEPTH_PREPASS
// The depth of the opaque faces at `uv` on screen, 0 where there are none.
fn opaque_depth(uv: vec2<f32>) -> f32 {
    let pixel = uv * view.viewport.zw + view.viewport.xy;
    return prepass_depth(vec4(pixel, 0.0, 0.0), 0u);
}

// What the screen shows of the opaque faces at `uv`, before the water.
fn opaque_color(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(view_transmission_texture, view_transmission_sampler, uv, 0.0).rgb;
}

// What the ray from `origin` toward `dir` meets on screen first, and how much to trust it, as
// the screen-space reflections of shader packs: steps that grow with the distance, then a
// binary search between the last two. Nothing where the ray leaves the screen or passes
// behind a face too thick to be the one it hits.
fn screen_reflection(origin: vec3<f32>, dir: vec3<f32>) -> vec4<f32> {
    var before = 0.0;
    var t = 0.4;
    for (var i = 0; i < 40; i++) {
        let ndc = position_world_to_ndc(origin + dir * t);
        if any(abs(ndc.xy) > vec2(1.0)) || ndc.z <= 0.0 {
            return vec4(0.0);
        }
        let depth = opaque_depth(ndc_to_uv(ndc.xy));
        // The depth is reversed: the ray is behind the face if its own is smaller
        if depth > ndc.z {
            let gap = depth_ndc_to_view_z(depth) - depth_ndc_to_view_z(ndc.z);
            if gap > max(1.5, t * 0.1) {
                // Behind something in front of what it meets: on with the march
                before = t;
                t = t * 1.12 + 0.35;
                continue;
            }
            var near = before;
            var far = t;
            for (var j = 0; j < 5; j++) {
                let mid = (near + far) * 0.5;
                let m = position_world_to_ndc(origin + dir * mid);
                if opaque_depth(ndc_to_uv(m.xy)) > m.z {
                    far = mid;
                } else {
                    near = mid;
                }
            }
            let hit = position_world_to_ndc(origin + dir * far);
            let edge = max(abs(hit.x), abs(hit.y));
            let trust = (1.0 - smoothstep(0.8, 1.0, edge)) * (1.0 - smoothstep(0.7, 1.0, f32(i) / 40.0));
            return vec4(opaque_color(ndc_to_uv(hit.xy)), trust);
        }
        before = t;
        t = t * 1.12 + 0.35;
    }
    return vec4(0.0);
}

// The surface of the water at `in`, of normal `N` seen along `V`: what lies under it, bent
// by the waves and absorbed by the depth of water (red first), then what it reflects, the
// screen where it can, the sky elsewhere, with a glint of the sun. `body` is the colour the
// water scatters, `sky` how much of the sky reaches it.
fn water_surface(in: VertexOutput, N: vec3<f32>, V: vec3<f32>, body: vec3<f32>, sky: f32) -> vec3<f32> {
    let uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
    let surface_z = position_world_to_view(in.world_position.xyz).z;
    // The waves bend what lies under them, less from far
    let bend = (N.xz - vec2(0.0)) * 0.6 / max(-surface_z, 1.0);
    var under_uv = uv + bend;
    // What is bent from in front of the water is not under it
    if depth_ndc_to_view_z(opaque_depth(under_uv)) > surface_z {
        under_uv = uv;
    }
    let floor_depth = opaque_depth(under_uv);
    var thickness = 64.0;
    if floor_depth > 0.0 {
        let floor = position_ndc_to_world(vec3(uv_to_ndc(under_uv), floor_depth));
        thickness = distance(floor, in.world_position.xyz);
    }
    let absorbed = exp(-thickness * vec3(0.42, 0.11, 0.07));
    let scattered = 1.0 - exp(-thickness * 0.35);
    let under = opaque_color(under_uv) * absorbed + body * scattered;

    // The waves bend the reflection less than they would a mirror: too bent, it breaks up
    let R = reflect(-V, normalize(mix(vec3(0.0, 1.0, 0.0), N, 0.35)));
    var reflected = sky_radiance(R, 0.0) * sky;
    let screen = screen_reflection(in.world_position.xyz, R);
    reflected = mix(reflected, screen.rgb, screen.a);
    let facing = max(dot(N, V), 0.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - facing, 5.0);
    var color = mix(under, reflected, fresnel);

    // The sun, sharp on the waves, of the colour of the sky around it
    if lights.n_directional_lights > 0u {
        let light = lights.directional_lights[0];
        let L = light.direction_to_light;
        let H = normalize(L + V);
        let shininess = 900.0;
        let lobe = (shininess + 8.0) / 25.13 * pow(max(dot(N, H), 0.0), shininess) * max(dot(N, L), 0.0);
        let halo = sky_radiance(L, 0.6);
        let tint = select(vec3(1.0), halo / max(max(halo.r, max(halo.g, halo.b)), 1e-4), has_sky());
        let view_z = position_world_to_view(in.world_position.xyz).z;
        let shadow = fetch_directional_shadow(0u, in.world_position, N, view_z, in.position.xy);
        color += light.color.rgb * view.exposure * tint * lobe * fresnel * shadow * sky;
    }
    return color;
}
#endif

// What `bevy_pbr::pbr` does in forward rendering, with the block textures, lit by the light
// levels of the vertices.
@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;
    let colouring = colouring_of(in.uv_b);
    var texture = block_color(in);
    let color = biome_color(in, colouring.biome_tint, shading.palette[colouring.color].rgb);
    if colouring.overlay == 0u {
        texture = vec4(texture.rgb * color, texture.a);
    } else {
        // Only the overlay is coloured, drawn over the texture of the face
        let overlay = texture_color(colouring.overlay - 1u, in.uv);
        texture = vec4(mix(texture.rgb, overlay.rgb * color, overlay.a), texture.a);
    }
#ifdef VERTEX_COLORS
    let light = in.color;
    // The standard material would colour the face with it
    in.color = vec4(1.0);
#else
    let light = vec4(1.0, 0.0, 1.0, 1.0);
#endif
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color *= texture;

    // The rain darkens what it wets, the more so in a puddle
    var wet = Wet(0.0, 0.0, 0.0, 0.0, vec4(1.0, 1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0));
    if shading.liquid == 0u && weather.wetness > 0.0 {
        let surface = block_textures[index_of(in.uv_b.x)].w >> 1u;
        wet = wet_surface(in, pbr_input.world_normal, light.r, surface);
        let under_water = wet.puddle * mix(0.6, 1.0, wet.deep);
        let darken = mix(1.0, wet.response.x, wet.amount) * mix(1.0, PUDDLE_DARKEN, under_water);
        pbr_input.material.base_color = vec4(
            pbr_input.material.base_color.rgb * darken,
            pbr_input.material.base_color.a,
        );
    }

    let water = shading.liquid != 0u && (shading.effects & EFFECT_WATER) != 0u;
    let screen_water = water && (shading.effects & EFFECT_SSR) != 0u;
    // The water shades itself (`water_surface`), not as seen through
    pbr_input.material.specular_transmission = 0.0;
    var fresnel = 0.0;
    if water {
        // A smooth surface, rippled on top, which reflects the sky more and shows less of what
        // lies under it the more it is seen from the side, as the Fresnel term says
        let footprint = length(fwidth(in.world_position.xz));
        if pbr_input.world_normal.y > 0.5 {
            let rain = rain_on_water(in.world_position.xyz, light.r);
            pbr_input.N = water_normal(in.world_position.xz, footprint, rain);
        }
        pbr_input.material.perceptual_roughness = 0.12;
        pbr_input.material.reflectance = vec3(0.5);
        let facing = abs(dot(pbr_input.N, pbr_input.V));
        fresnel = 0.02 + 0.98 * pow(1.0 - facing, 5.0);
        pbr_input.material.base_color.a = mix(pbr_input.material.base_color.a, 1.0, fresnel);
        // Under the overcast, the water loses some of its blue to the grey of the sky
        let water_color = pbr_input.material.base_color.rgb;
        let grey = vec3(dot(water_color, vec3(0.3, 0.59, 0.11)));
        pbr_input.material.base_color = vec4(
            mix(water_color, grey, WATER_GREYING * weather.overcast),
            pbr_input.material.base_color.a,
        );
    }
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    // The light of the sky dims the sun, the moon and the ambient light, and the light of the
    // blocks, an ambient light of its own, shines where it is brighter. Each corner is shaded
    // by the blocks around it.
    let sky = level_brightness(light.r * MAX_LIGHT);
    let block = level_brightness(light.g * MAX_LIGHT) * select(1.0, 0.0, light.g == 0.0);
    let occlusion = 0.55 + 0.15 * light.b * 3.0;
    let lit_by_blocks = pbr_input.material.base_color.rgb * block * BLOCK_LIGHT_COLOR
        * shading.block_light * BASE_EXPOSURE;
    out.color = vec4(max(out.color.rgb * sky, lit_by_blocks) * occlusion, out.color.a);
    // A flash of lightning lights what is open to the sky, as its light comes from it
    let flash = flash_light(in.world_position.xyz, pbr_input.world_normal);
    out.color = vec4(
        out.color.rgb + pbr_input.material.base_color.rgb * flash * sky * occlusion,
        out.color.a,
    );
    if wet.amount > 0.0 {
        // The corners reflect less of the sky, if not as little as they are lit
        let open = sky * mix(1.0, occlusion, 0.5);
        out.color = vec4(
            wet_color(in, pbr_input.world_normal, pbr_input.V, wet, open, out.color.rgb),
            out.color.a,
        );
    }
    if water && !screen_water {
        // The sky, as bright as the day is, where the water is under it
        let daylight = lights.ambient_color.r / DAY_AMBIENT;
        let R = reflect(-pbr_input.V, pbr_input.N);
        var reflected = mix(SKY_COLOR * daylight, overcast_sky(R), weather.overcast) * sky;
        if has_sky() {
            reflected = sky_radiance(R, 0.0) * sky;
        }
        out.color = vec4(mix(out.color.rgb, reflected, fresnel), out.color.a);
    }
#ifdef DEPTH_PREPASS
    if screen_water {
        if is_front {
            out.color = vec4(water_surface(in, pbr_input.N, pbr_input.V, out.color.rgb * 0.5, sky), 1.0);
        } else {
            // From under the water, the surface shows what is above it, dimmed
            let uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
            out.color = vec4(opaque_color(uv) * vec3(0.55, 0.75, 0.85), 1.0);
        }
    }
#endif
#ifdef DISTANCE_FOG
    // Where the fog is transparent, the chunks fade into the sky itself (`shaders.rs`)
    if fog.base_color.a == 0.0 && has_sky() {
        let to = in.world_position.xyz - view.world_position;
        let distance = length(to);
        let amount = smoothstep(fog.be.x, fog.be.y, distance);
        let sky_color = sky_radiance(horizon_direction(to / distance), 0.0);
        out.color = vec4(mix(out.color.rgb, sky_color, amount), out.color.a);
    }
#endif
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}

#endif
