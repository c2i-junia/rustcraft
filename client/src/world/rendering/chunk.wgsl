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
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif
#endif

// For each texture: its first layer, its number of frames, how long a frame is shown in
// twentieths of a second, and whether a frame fades into the next. Keep the length in sync with
// `MAX_TILES`.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> block_textures: array<vec4<u32>, 256>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var block_layers: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var block_sampler: sampler;
// The climate of the columns around the player, wrapped around (`climate.rs`): their place on
// the colour maps from the bottom right corner, the temperature in red and the downfall times
// the temperature in green, blended across biomes, and an alpha of 0 where it is not known yet.
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
// The brightness of the ambient light at noon, `DAY_AMBIENT` in `graphics.rs`: when the water
// reflects the sky as bright as it is drawn
const DAY_AMBIENT: f32 = 400.0;
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
    if texture.w == 0u || frames == 1u {
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
// directions, the sum of their slopes. `footprint` is how many blocks a pixel spans there: the
// waves too short to be drawn fade out, instead of making stripes far off.
fn water_normal(at: vec2<f32>, footprint: f32) -> vec3<f32> {
    let time = seconds();
    var slope = vec2(0.0);
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
            pbr_input.N = water_normal(in.world_position.xz, footprint);
        }
        pbr_input.material.perceptual_roughness = 0.12;
        pbr_input.material.reflectance = vec3(0.5);
        let facing = abs(dot(pbr_input.N, pbr_input.V));
        fresnel = 0.02 + 0.98 * pow(1.0 - facing, 5.0);
        pbr_input.material.base_color.a = mix(pbr_input.material.base_color.a, 1.0, fresnel);
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
