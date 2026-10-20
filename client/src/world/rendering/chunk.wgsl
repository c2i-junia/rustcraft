// The chunks are drawn with the block textures, the layers of a texture array: `uv_b.x` is the
// index of the texture of a face in `block_textures`, and `uv` the place in it. A face that spans
// several blocks has its `uv` go past 1, and the texture repeats across it.

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

// The colour of the block texture at `in`. The layers have no mipmaps: sampling the first level
// is the same, and allowed where the control flow differs between fragments.
fn block_color(in: VertexOutput) -> vec4<f32> {
    // Rounded, as interpolating the index of the face may stray from it, if by little
    // The index comes as a fraction of 65535 (`TILE` in `meshing.rs`)
    let texture = block_textures[u32(round(in.uv_b.x * 65535.0))];
    return textureSampleLevel(block_layers, block_sampler, in.uv, texture.x, 0.0);
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

// What `bevy_pbr::pbr` does in forward rendering, with the block textures
@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
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
