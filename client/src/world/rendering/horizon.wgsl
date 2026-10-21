// The horizon skirt of the physical sky (`shaders.rs`): a ring around the camera, past
// the chunks and under the horizon, of the colour of the sky at the horizon, which hides the
// ground of the planet of the atmosphere. The mesh follows the player's camera; each camera
// draws it around itself, so that a portal camera far from the player has its own.

#import bevy_pbr::{
    mesh_functions::{get_world_from_local, mesh_position_local_to_world},
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}
#import rustcraft::sky::{sky_radiance, horizon_direction}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let world_from_local = get_world_from_local(vertex.instance_index);
    let world = mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0)).xyz;
    // Moved from around the player's camera to around this one
    let around = world - world_from_local[3].xyz + view.world_position;
    var out: VertexOutput;
    out.position = position_world_to_clip(around);
    out.world_position = around;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position - view.world_position);
    // Opaque under the horizon, faded out into the sky over it
    let alpha = 1.0 - smoothstep(0.0, 0.12, dir.y);
    return vec4(sky_radiance(horizon_direction(dir), 0.0), alpha);
}
