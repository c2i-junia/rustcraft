// The chunks are drawn with the block textures, the layers of a texture array: `uv_b.x` is the
// index of the texture of a face in `block_textures`, and `uv` the place in it. A face that spans
// several blocks has its `uv` go past 1, and the texture repeats across it. `uv_b.y` names the
// colour map that colours the face from the climate of its column, if any (`BiomeTint`).

#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::alpha_discard

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_bindings,
    pbr_prepass_functions,
    pbr_functions::prepare_world_normal,
    pbr_types,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#endif

// For each texture: its first layer, its number of frames, how long a frame is shown in
// twentieths of a second, and whether a frame fades into the next. Keep the length in sync with
// `MAX_TILES`.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> block_textures: array<vec4<u32>, 256>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var block_layers: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var block_sampler: sampler;
// The climate of the columns around the player, wrapped around (`climate.rs`): the temperature
// in red, the humidity in green, and an alpha of 0 where it is not known yet.
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var climate_map: texture_2d<f32>;
// The colours of the grass and of the leaves by climate, read as Minecraft reads its own.
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var grass_colors: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var foliage_colors: texture_2d<f32>;

// The values of `BiomeTint::index`
const BIOME_TINT_GRASS: u32 = 1u;
const BIOME_TINT_FOLIAGE: u32 = 2u;

// How long the game has been drawing, in seconds, which plays the animated textures. The
// prepass has no such binding: it draws their first frame, which only matters for the pixels
// it discards.
fn seconds() -> f32 {
#ifdef PREPASS_PIPELINE
    return 0.0;
#else
    return globals.time;
#endif
}

// The colour of the block texture at `in`, at its current frame. The layers have no mipmaps:
// sampling the first level is the same, and allowed where the control flow differs between
// fragments.
fn block_color(in: VertexOutput) -> vec4<f32> {
    // Rounded, as interpolating the index of the face may stray from it, if by little
    // The index comes as a fraction of 65535 (`TILE` in `meshing.rs`)
    let texture = block_textures[u32(round(in.uv_b.x * 65535.0))];
    let frames = max(texture.y, 1u);
    // In frames since the start, at 20 frame times a second
    let progress = seconds() * 20.0 / f32(max(texture.z, 1u));
    let frame = u32(progress) % frames;
    let color = textureSampleLevel(block_layers, block_sampler, in.uv, texture.x + frame, 0.0);
    if texture.w == 0u || frames == 1u {
        return color;
    }
    let next = textureSampleLevel(block_layers, block_sampler, in.uv, texture.x + (frame + 1u) % frames, 0.0);
    return mix(color, next, fract(progress));
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
    if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK {
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

// The colour of the grass or of the leaves at `in`, from the climate of its column, or `color`
// where the face has no colour map or the climate is not known yet.
fn biome_color(in: VertexOutput, color: vec3<f32>) -> vec3<f32> {
    // A fraction of 65535, as the index of the texture
    let tint = u32(round(in.uv_b.y * 65535.0));
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
    // As Minecraft: the hotter to the left, and up as the humidity grows, the more so the
    // hotter the climate
    let temperature = climate.r;
    let at = vec2(1.0 - temperature, 1.0 - climate.g * temperature);
    if tint == BIOME_TINT_GRASS {
        return colormap_color(grass_colors, at);
    }
    return colormap_color(foliage_colors, at);
}

// What `bevy_pbr::pbr` does in forward rendering, with the block textures
@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;
#ifdef VERTEX_COLORS
    in.color = vec4(biome_color(in, in.color.rgb), in.color.a);
#endif
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color *= block_color(in);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}

#endif
