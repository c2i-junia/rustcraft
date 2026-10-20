// The horizon skirt of the proof of concept (`shader_poc.rs`): a ring around the camera, past
// the chunks and under the horizon, of the colour of the sky at the horizon, which hides the
// ground of the planet of the atmosphere.

#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}
#import rustcraft::sky::{sky_radiance, horizon_direction}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position.xyz - view.world_position);
    // Opaque under the horizon, faded out into the sky over it
    let alpha = 1.0 - smoothstep(0.0, 0.12, dir.y);
    return vec4(sky_radiance(horizon_direction(dir), 0.0), alpha);
}
