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

// Keep in sync with `CloudParams`
struct CloudParams {
    tint: vec4<f32>,
    opacity: f32,
    fade_start: f32,
    fade_end: f32,
    threshold: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: CloudParams;

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
    return vec4(params.tint.rgb * in.shade, alpha);
}
