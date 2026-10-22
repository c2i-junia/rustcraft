// The bolts of lightning, drawn by a pass of their own after the TAA (`lightning/render.rs`).
// There is no mesh: the points of the channels lie in a storage buffer, and each piece between
// two points is a ribbon facing the camera, six vertices, whose corners at a point are shared
// with the next piece through a miter, so that the glow runs on without a seam. Its middle is
// a hot white core, around it a violet glow, wider the farther the bolt, as the rain scatters
// its light; they are added to the frame. A piece across two channels is folded into a point.

#import bevy_render::view::View

// `LightningParams` in `render.rs`
struct Params {
    core: f32,
    glow: f32,
}

// `BoltVertex` in `render.rs`
struct BoltVertex {
    position: vec3<f32>,
    width: f32,
    brightness: f32,
    ends: u32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read> points: array<BoltVertex>;
#ifdef MULTISAMPLED
@group(0) @binding(3) var depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(3) var depth: texture_depth_2d;
#endif

const FIRST: u32 = 1u;
const LAST: u32 = 2u;

// How many times wider than the core the glow reaches, near and far
const GLOW_NEAR: f32 = 11.0;
const GLOW_FAR: f32 = 18.0;
// The narrowest half widths drawn, in pixels: a thinner core is drawn fainter instead
const MIN_CORE: f32 = 0.75;
const MIN_GLOW: f32 = 6.0;

const CORE_COLOR: vec3<f32> = vec3(1.0, 0.96, 1.0);
const GLOW_COLOR: vec3<f32> = vec3(0.56, 0.46, 1.0);

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // From -1 to 1 across the ribbon
    @location(0) across: f32,
    // The half width of the core, as a part of the half width of the ribbon
    @location(1) core: f32,
    @location(2) brightness: f32,
}

// Where `clip` is on screen, in pixels from the middle.
fn on_screen(clip: vec4<f32>) -> vec2<f32> {
    return clip.xy / clip.w * 0.5 * view.viewport.zw;
}

fn clip_of(index: u32) -> vec4<f32> {
    // Unjittered: the TAA is done with
    return view.unjittered_clip_from_world * vec4(points[index].position, 1.0);
}

fn direction(start: vec2<f32>, end: vec2<f32>) -> vec2<f32> {
    let d = end - start;
    let length = length(d);
    return select(vec2(0.0), d / length, length > 1e-4);
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    var out: VertexOutput;
    let segment = index / 6u;
    let corner = index % 6u;
    // The corners: at the start or the end of the piece, and on which side
    var ends = array<u32, 6>(0u, 0u, 1u, 1u, 0u, 1u);
    var sides = array<f32, 6>(-1.0, 1.0, -1.0, -1.0, 1.0, 1.0);
    let folded = vec4(0.0, 0.0, 0.0, 1.0);
    if (points[segment].ends & LAST) != 0u {
        out.position = folded;
        return out;
    }
    let at = segment + ends[corner];
    let point = points[at];
    let prev = select(at - 1u, at, (point.ends & FIRST) != 0u);
    let next = select(at + 1u, at, (point.ends & LAST) != 0u);
    let clip = clip_of(at);
    if clip.w < 0.1 || clip_of(segment).w < 0.1 || clip_of(segment + 1u).w < 0.1 {
        out.position = folded;
        return out;
    }
    let here = on_screen(clip);
    let tangent = normalize(
        direction(on_screen(clip_of(prev)), here) + direction(here, on_screen(clip_of(next))) + vec2(1e-6, 0.0)
    );
    let piece = direction(on_screen(clip_of(segment)), on_screen(clip_of(segment + 1u)));
    let normal = vec2(-tangent.y, tangent.x);
    let miter = 1.0 / max(abs(dot(normal, vec2(-piece.y, piece.x))), 0.5);

    // Widths in pixels
    let pixels_per_block = view.clip_from_view[1][1] * 0.5 * view.viewport.w / clip.w;
    let core = point.width * 0.5 * pixels_per_block;
    let distance = clip.w;
    let glow = max(max(core, MIN_CORE) * mix(GLOW_NEAR, GLOW_FAR, smoothstep(30.0, 160.0, distance)), MIN_GLOW);
    let side = sides[corner];
    let offset = normal * side * glow * miter;
    out.position = clip + vec4(offset / (0.5 * view.viewport.zw) * clip.w, 0.0, 0.0);
    out.across = side;
    out.core = max(core, MIN_CORE) / glow;
    // A core thinner than drawn is fainter, and the rain dims a bolt far off
    let thin = sqrt(clamp(core / MIN_CORE, 0.0, 1.0));
    out.brightness = point.brightness * mix(0.35, 1.0, thin) * mix(1.0, 0.6, smoothstep(40.0, 200.0, distance));
    return out;
}

// The glow at `a` across the ribbon: a bright halo around the core, and a faint one wide.
fn glow_profile(a: f32) -> f32 {
    return exp(-a * 9.0) * 0.6 + exp(-a * 3.0) * 0.3 + exp(-a * 1.2) * 0.1;
}

// Whether the world drawn hides the fragment at `position`.
fn behind_world(position: vec4<f32>) -> bool {
    let world = textureLoad(depth, vec2<i32>(position.xy), 0);
    // The depth is reversed: nearer is greater
    return position.z < world;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.brightness <= 0.0 || behind_world(in.position) {
        discard;
    }
    let a = abs(in.across);
    let core = 1.0 - smoothstep(in.core * 0.4, in.core * 1.1, a);
    // Steep near the core, then a long soft tail to the edge
    let glow = (glow_profile(a) - glow_profile(1.0)) / (glow_profile(0.0) - glow_profile(1.0));
    let color = CORE_COLOR * core * params.core + GLOW_COLOR * max(glow, 0.0) * params.glow;
    return vec4(color * in.brightness, 0.0);
}
