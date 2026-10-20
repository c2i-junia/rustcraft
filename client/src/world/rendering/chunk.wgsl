// The chunks are drawn with the block atlas, each face with one of its tiles: `uv_b.x` is the
// index of the tile and `uv` the place in it. A face that spans several blocks has its `uv` go
// past 1, and the tile repeats across it.

#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::alpha_discard

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_prepass_functions,
    pbr_functions::prepare_world_normal,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#endif

// Keep in sync with `MAX_TILES`
struct AtlasTiles {
    // min x, min y, max x, max y of each tile, in atlas units
    rects: array<vec4<f32>, 256>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> atlas_tiles: AtlasTiles;

// How far inside its tile a face stops, in atlas units, so that the neighbouring tile never
// bleeds along the edges of a block. !!! DO NOT REMOVE THE INSET !!! It removes seams between
// blocks in chunk meshes.
const UV_INSET: f32 = 0.001;

// Where `in` samples the atlas
fn atlas_uv(in: VertexOutput) -> vec2<f32> {
    // Rounded, as interpolating the index of the face may stray from it, if by little
    // The index comes as a fraction of 65535 (`TILE` in `meshing.rs`)
    let rect = atlas_tiles.rects[u32(round(in.uv_b.x * 65535.0))];
    let low = rect.xy + UV_INSET;
    let high = rect.zw - UV_INSET;
    return low + fract(in.uv) * (high - low);
}

#ifdef PREPASS_PIPELINE

// What `bevy_pbr::pbr_prepass` does, with the UVs in the atlas
#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;
    in.uv = atlas_uv(in);
    pbr_prepass_functions::prepass_alpha_discard(in);

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
fn fragment(vertex_output: VertexOutput) {
    var in = vertex_output;
    in.uv = atlas_uv(in);
    pbr_prepass_functions::prepass_alpha_discard(in);
}
#endif

#else

// What `bevy_pbr::pbr` does in forward rendering, with the UVs in the atlas
@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var in = vertex_output;
    in.uv = atlas_uv(in);
    var pbr_input = pbr_input_from_standard_material(in, is_front);
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
