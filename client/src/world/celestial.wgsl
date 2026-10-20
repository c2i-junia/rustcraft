// The sun, the moon and the stars (`CelestialMaterial` in `celestial.rs`), drawn behind
// everything else: the depth of the far plane, where only the sky is. The sun is a round disk
// with a glow, reddened as it nears the horizon; the moon a disk of 16 by 16 pixels, like the
// textures, which waxes and wanes over 8 days; the stars squares of a pixel or two that twinkle,
// fixed to the sky as it turns. All three fade out under the horizon.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::{globals, view},
    view_transformations::position_world_to_clip,
}

struct CelestialParams {
    // 0 for the sun, 1 for the moon, 2 for the stars
    kind: u32,
    // From 0 to 1: the full moon at 0, the new moon at 0.5
    phase: f32,
    // How much of the glow around the sun or the moon is drawn: the physical sky draws its own
    glow: f32,
    // How visible the body is, from 0 to 1: the stars only show at night
    visibility: f32,
    // How bright it is drawn: brighter in the 16-bit float target of the physical sky, which is
    // tonemapped, than in the 8-bit one without it
    intensity: f32,
    // The colour of the sun or of the moon, of the light that reaches the ground
    color: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: CelestialParams;

const SUN: u32 = 0u;
const MOON: u32 = 1u;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    // Where the vertex is on the quad of the sun or of the moon, or on the sphere of the stars,
    // in the frame of the sky
    @location(1) local: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.world_position = world.xyz;
    out.position = position_world_to_clip(world.xyz);
    // On the far plane, whose depth is 0 with the reversed depth: behind whatever is drawn
    out.position.z = 0.0;
    out.local = vertex.position;
    out.uv = vertex.uv;
    return out;
}

fn hash12(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var q = fract(p * vec3(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.xxy + q.yxx) * q.zyx);
}

// How much of the body shows at the height `y` of the direction it is seen in: none under the
// horizon, where the ground would hide it.
fn above_horizon(y: f32) -> f32 {
    return smoothstep(-0.04, 0.03, y);
}

// The sun, with `at` from -1 to 1 across its quad, which spans 8 times its radius.
fn sun(at: vec2<f32>) -> vec4<f32> {
    let r = length(at) * 8.0;
    // A disk a little darker at its edge, as the real one
    let disk = 1.0 - smoothstep(0.92, 1.0, r);
    let limb = mix(0.75, 1.0, sqrt(max(1.0 - r * r, 0.0)));
    let glow = params.glow * (0.5 * exp(-r * 1.4) + 0.12 * exp(-r * 0.45));
    let color = params.color.rgb;
    return vec4(color * (disk * limb * 14.0 + glow * 2.0), 1.0);
}

// Where the dark seas of the moon are, in pixels of its 16 by 16 grid from its centre.
fn maria(pixel: vec2<f32>) -> f32 {
    let seas = array(vec3(-2.5, -2.0, 3.2), vec3(2.0, -3.5, 2.2), vec3(1.5, 2.5, 2.6), vec3(-3.5, 3.0, 1.6));
    var dark = 0.0;
    for (var i = 0; i < 4; i++) {
        let sea = seas[i];
        dark = max(dark, 1.0 - smoothstep(sea.z - 1.0, sea.z, length(pixel - sea.xy)));
    }
    return dark;
}

// The moon, with `at` from -1 to 1 across its quad, which spans 4 times its radius.
fn moon(at: vec2<f32>) -> vec4<f32> {
    let disk_at = at * 4.0;
    // The pixel of the 16 by 16 grid across the disk, and its centre on the disk
    let pixel = floor(disk_at * 8.0);
    let center = (pixel + 0.5) / 8.0;
    var color = vec3(0.0);
    let r2 = dot(center, center);
    if r2 < 1.0 {
        // The sun lights the half of the sphere turned toward it, which turns away from us
        // over the days
        let normal = vec3(center, sqrt(1.0 - r2));
        let angle = params.phase * 6.2831853;
        let to_sun = vec3(sin(angle), 0.0, cos(angle));
        let lit = step(0.0, dot(normal, to_sun));
        let ground = 0.82 + 0.18 * hash12(pixel + 17.0) - 0.28 * maria(pixel + 0.5);
        // The side in the shade is lit a little by the earth
        color = params.color.rgb * ground * mix(0.05, 1.6, lit);
    }
    let r = length(disk_at);
    let glow = params.glow * 0.25 * exp(-max(r - 1.0, 0.0) * 1.8) * (1.0 - abs(params.phase * 2.0 - 1.0));
    return vec4(color + params.color.rgb * glow * step(1.0, r), 1.0);
}

// The stars toward `dir`, in the frame of the sky: in cells of a grid on each face of a cube
// around the sky, a few have a star, a square of a pixel or two.
fn stars(dir: vec3<f32>) -> vec3<f32> {
    let a = abs(dir);
    var uv: vec2<f32>;
    var face: f32;
    if a.x >= a.y && a.x >= a.z {
        uv = dir.yz / a.x;
        face = select(0.0, 1.0, dir.x > 0.0);
    } else if a.y >= a.z {
        uv = dir.xz / a.y;
        face = select(2.0, 3.0, dir.y > 0.0);
    } else {
        uv = dir.xy / a.z;
        face = select(4.0, 5.0, dir.z > 0.0);
    }
    let grid = 200.0;
    let p = (uv * 0.5 + 0.5) * grid;
    let cell = floor(p);
    let h = hash33(vec3(cell, face * 7.0 + 3.0));
    if h.x > 0.07 {
        return vec3(0.0);
    }
    // How many cells a pixel spans: a star is at least a pixel wide
    let footprint = max(fwidth(p.x), fwidth(p.y));
    let size = max(footprint * (0.6 + 1.2 * h.y * h.y), 0.08);
    let center = cell + 0.3 + 0.4 * h.yz;
    let offset = abs(p - center);
    if max(offset.x, offset.y) > size * 0.5 {
        return vec3(0.0);
    }
    let brightness = 0.25 + 1.6 * pow(h.y, 3.0);
    let twinkle = 0.7 + 0.3 * sin(globals.time * (1.5 + 3.0 * h.z) + h.x * 300.0);
    let tint = mix(vec3(0.72, 0.82, 1.0), vec3(1.0, 0.86, 0.66), h.z);
    return tint * brightness * twinkle;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_dir = normalize(in.world_position - view.world_position);
    let shown = above_horizon(world_dir.y) * params.visibility;
    let at = in.uv * 2.0 - 1.0;
    var color = vec3(0.0);
    if params.kind == SUN {
        color = sun(at).rgb;
    } else if params.kind == MOON {
        color = moon(at).rgb;
    } else {
        let dir = normalize(in.local);
        // None behind the moon, which is opposite the sun, at -Z of the sky
        let behind_moon = smoothstep(0.9994, 0.9990, dot(dir, vec3(0.0, 0.0, -1.0)));
        color = stars(dir) * behind_moon;
    }
    // Added to the sky behind
    return vec4(color * shown * params.intensity, 0.0);
}
