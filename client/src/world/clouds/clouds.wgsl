// The clouds: unlit, each face as bright as its shade, faded out with the horizontal distance
// to the camera. Neither the lights nor the fog of the chunks touch them. The same shader draws
// their depth first, then their colour where the depth is equal: both must find the same
// positions, hence `@invariant`.
//
// The mesh holds every face that shows at some threshold of the weather, with the densities of
// the two cells it lies between: a face that does not show at the threshold of now is folded
// into a point, which draws nothing.

#import bevy_pbr::{
    mesh_functions::{get_world_from_local, mesh_position_local_to_world},
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}
#import rustcraft::sky::{overcast_sky, weather}
#import rustcraft::weather::FLASH_COLOR

// Keep in sync with `CloudParams`
struct CloudParams {
    tint: vec4<f32>,
    opacity: f32,
    fade_start: f32,
    fade_end: f32,
    threshold: f32,
    overcast: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: CloudParams;

// How bright a flash of lightning makes the clouds on screen: from within, around the strike,
// over `FLASH_REACH` blocks, and a little all over.
const FLASH_GLOW: f32 = 2.6;
const FLASH_REACH: f32 = 56.0;
const FLASH_ALL_OVER: f32 = 0.25;

// The side of a cell, `CELL` in `clouds/mod.rs`
const CELL: i32 = 12;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) shade: f32,
    // The density of the cell of the face, and of the one it looks into
    @location(2) densities: vec2<f32>,
}

struct VertexOutput {
    @invariant @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) shade: f32,
    // How thick the cloud of the face is, from 0 at the threshold to 1 for the clouds of a
    // clear sky
    @location(2) @interpolate(flat) thickness: f32,
    // Where in the mesh, in which the cells are laid out
    @location(3) local_position: vec3<f32>,
}

// A random number in [0, 1) for the cell `cell`.
fn hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2(12.9898, 78.233))) * 43758.5453);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let world_from_local = get_world_from_local(vertex.instance_index);
    let world_position = mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position.xyz;
    out.shade = vertex.shade;
    // Shown when the cell of the face is full and the next one is not
    let high = vertex.densities.x;
    let low = vertex.densities.y;
    out.thickness = smoothstep(params.threshold, params.threshold + 0.3, high);
    out.local_position = vertex.position;
    if !(low <= params.threshold && params.threshold < high) {
        out.clip_position = vec4(0.0, 0.0, 0.0, 1.0);
    }
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let distance = length(in.world_position.xz - view.world_position.xz);
    let alpha = params.opacity * (1.0 - smoothstep(params.fade_start, params.fade_end, distance));
    // Where the clouds are faded out, their depth would hide what is behind them
    if alpha < 0.01 {
        discard;
    }
    // Under the overcast, the clouds are of its grey, the thicker ones darker and each cell a
    // little apart, and the clouds far off fade into the sky behind them before they fade out
    let level = params.overcast.w;
    // The cell from a point just past the edge: a face on the edge of two cells would
    // otherwise flicker between them
    let cell = hash(floor((in.local_position.xz + 0.01) / f32(CELL)));
    let thick = mix(1.0, 0.72, in.thickness) * (0.92 + 0.16 * cell);
    let grey = params.overcast.rgb * mix(1.0, thick, level);
    let color = mix(params.tint.rgb, grey, level) * in.shade;
    let haze = smoothstep(0.35 * params.fade_end, params.fade_end, distance) * level;
    let sky = overcast_sky(normalize(in.world_position - view.world_position.xyz));
    var lit = mix(color, sky, haze);
    if weather.flash > 0.0 || weather.strike.w > 0.0 {
        // Lit from within around the strike, some cells more than others, the bottoms the most
        let off = length(in.world_position.xz - weather.strike.xz) / FLASH_REACH;
        let near = weather.strike.w * FLASH_GLOW * exp(-off * off);
        let glow = (near + weather.flash * FLASH_ALL_OVER) * (0.55 + 0.9 * cell) / max(in.shade, 0.7);
        lit += FLASH_COLOR * glow;
    }
    return vec4(lit, alpha);
}
