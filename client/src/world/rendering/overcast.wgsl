// The overcast dome (`OvercastMaterial` in `overcast.rs`): the grey sky of the weather, drawn
// on the far plane over the physical sky, as opaque as the sky is overcast.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}
#import rustcraft::sky::{overcast_sky, weather}

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
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.world_position = world.xyz;
    out.position = position_world_to_clip(world.xyz);
    // On the far plane, whose depth is 0 with the reversed depth: over the sky only
    out.position.z = 0.0;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position - view.world_position);
    return vec4(overcast_sky(dir), weather.overcast);
}
