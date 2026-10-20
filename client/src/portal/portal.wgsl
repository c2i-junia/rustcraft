// The portals: unlit and unfogged, as what an open portal shows was lit and fogged by its
// camera. Keep the looks in sync with `PortalLook`.

#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

struct PortalParams {
    // linear RGB
    color: vec4<f32>,
    // (normal, -normal · centre): positive in front of the portal
    plane: vec4<f32>,
    look: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: PortalParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var portal_image: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var portal_sampler: sampler;

const VIEW: u32 = 0u;
const RIM: u32 = 2u;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // The camera of the portal sees what lies behind it from where this camera would be: its
    // image is read at the place of the fragment on the screen. It holds colours as they are
    // shown, tonemapped by the materials of its camera as by those of this one: they are copied
    // as they are, as tonemapping them again would wash them out.
    if params.look == VIEW {
        let uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
        return vec4(textureSample(portal_image, portal_sampler, uv).rgb, 1.0);
    }
    // The front of a portal, its colour and its rim, is only seen from in front of it. A portal
    // camera stands behind its twin, inside its box when it is within 0.12 block of its plane,
    // where it has no oblique near plane: it must see neither that box nor that rim
    if dot(params.plane.xyz, view.world_position) + params.plane.w < 0.0 {
        discard;
    }
    if params.look == RIM {
        // 0 at the edge of the opening, 1 at the outer edge of the rim
        let edge = in.uv.x;
        let color = mix(vec3(1.0), params.color.rgb, smoothstep(0.0, 0.25, edge));
        return vec4(color, pow(1.0 - edge, 1.8));
    }
    // Waves of the colour across the box
    let p = in.world_position.xyz;
    let wave = 0.5 + 0.5 * sin(p.y * 5.0 + sin((p.x + p.z) * 4.0));
    return vec4(mix(params.color.rgb * 0.25, params.color.rgb * 0.55, wave), 1.0);
}
