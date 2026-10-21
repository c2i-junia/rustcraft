// The rain, the snow and the splashes, drawn by a pass of their own after the TAA
// (`precipitation/render.rs`). There is no mesh: each drop is placed and moved by the vertex
// shader from its index and the time, in a box around the camera that it wraps around, so that
// it stays put in the world as the camera moves. The occlusion map tells the height at which a
// column stops what falls (`occlusion.rs`), the climate map whether rain or snow falls on it,
// or nothing; above the snow line, whatever falls is snow.
//
// The vertices are, in order: six for each drop of rain, six for each flake of snow, twelve
// for each splash (a ring on the ground, and droplets thrown up). A drop beyond the part shown
// is folded into a point, which draws nothing.

#import bevy_render::view::View
#import bevy_render::globals::Globals
#import rustcraft::weather::Weather

// `PrecipitationParams` in `render.rs`
struct Params {
    rain: u32,
    snow: u32,
    splashes: u32,
    // The part of them shown
    level: f32,
    // How much more opaque the drops are drawn, for the qualities that draw fewer
    opacity: f32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> globals: Globals;
@group(0) @binding(2) var<storage, read> weather: Weather;
@group(0) @binding(3) var<uniform> params: Params;
// For each column around the camera, wrapped, the height at which what falls stops
@group(0) @binding(4) var occlusion: texture_2d<f32>;
// The climate map of `chunk.wgsl`: what falls on each column in blue, `PrecipitationKind`
@group(0) @binding(5) var climate_map: texture_2d<f32>;
#ifdef MULTISAMPLED
@group(0) @binding(6) var depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(6) var depth: texture_depth_2d;
#endif

// The values of `PrecipitationKind`, and what a sprite is
const RAIN: u32 = 0u;
const SNOW: u32 = 1u;
const NOTHING: u32 = 2u;
const SPLASH_RING: u32 = 3u;
const SPLASH_CROWN: u32 = 4u;

// `SNOW_LINE` of the server's generation: above it, what falls is snow, give or take
// `SNOW_BAND` blocks from a drop to the next
const SNOW_LINE: f32 = 122.0;
const SNOW_BAND: f32 = 4.0;
// The heights of `occlusion.rs` for a column not known yet, and for one with no block
const UNKNOWN: f32 = 1e5;

// A band of drops around the eye: the half width of its box, its height and how far it reaches
// under the eye; the distances from the eye over which it fades in, and out along the ground;
// how wide its drops are, in blocks, and how opaque.
struct Band {
    extent: vec3<f32>,
    fade: vec4<f32>,
    width: f32,
    alpha: f32,
}

// The band of the drop of rain picked by `pick`: a few wide, soft and faint drops near the eye,
// finer ones further, and the finest far off, out to 40 blocks.
fn rain_band(pick: f32) -> Band {
    if pick < 0.15 {
        return Band(vec3(9.0, 16.0, 8.0), vec4(1.5, 4.0, 6.5, 9.0), 0.022, 0.22);
    }
    if pick < 0.55 {
        return Band(vec3(24.0, 28.0, 14.0), vec4(6.5, 9.0, 18.0, 24.0), 0.008, 0.5);
    }
    return Band(vec3(40.0, 36.0, 18.0), vec4(18.0, 24.0, 32.0, 40.0), 0.007, 0.4);
}

// The band of the flake of snow picked by `pick`: big flakes near the eye, smaller further, and
// a haze of specks far off, out to 40 blocks.
fn snow_band(pick: f32) -> Band {
    if pick < 0.3 {
        return Band(vec3(8.0, 12.0, 6.0), vec4(0.6, 1.4, 5.5, 8.0), 0.07, 0.9);
    }
    if pick < 0.7 {
        return Band(vec3(20.0, 24.0, 12.0), vec4(5.5, 8.0, 15.0, 20.0), 0.06, 0.85);
    }
    return Band(vec3(40.0, 32.0, 18.0), vec4(15.0, 20.0, 32.0, 40.0), 0.05, 0.75);
}

// The splashes: how far from the eye, how long they live, in seconds
const SPLASH_REACH: f32 = 16.0;
const SPLASH_LIFE: f32 = 0.26;
// The thinnest a sprite is drawn, in pixels: thinner, it is drawn this wide and fainter
const MIN_PIXELS: f32 = 1.3;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // Across the sprite, and along it; for a crown, in blocks from its foot
    @location(0) uv: vec2<f32>,
    @location(1) world_y: f32,
    // The light of the sprite, and how opaque it is
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) kind: u32,
    // What shapes it: how soft it is, its age, a random number, and whether on water
    @location(4) @interpolate(flat) shape: vec4<f32>,
    // The height under which nothing of it is drawn
    @location(5) @interpolate(flat) stop: f32,
}

// A hash of `n`, PCG's.
fn pcg(n: u32) -> u32 {
    let state = n * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Four random numbers in [0, 1) for `seed`.
fn random4(seed: u32) -> vec4<f32> {
    let a = pcg(seed);
    let b = pcg(a);
    let c = pcg(b);
    let d = pcg(c);
    return vec4(f32(a >> 8u), f32(b >> 8u), f32(c >> 8u), f32(d >> 8u)) / 16777216.0;
}

// The corner of a quad of the vertex `vertex` of its six, from (-1, -1) to (1, 1).
fn corner_of(vertex: u32) -> vec2<f32> {
    var corners = array(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
        vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
    );
    return corners[vertex];
}

// A vertex that draws nothing.
fn hidden() -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4(0.0, 0.0, 0.0, 1.0);
    out.color = vec4(0.0);
    out.kind = NOTHING;
    return out;
}

// `at`, moved by whole sizes of the box `size` from `low` into it.
fn wrap(at: vec3<f32>, low: vec3<f32>, size: vec3<f32>) -> vec3<f32> {
    let from_low = at - low;
    return low + from_low - size * floor(from_low / size);
}

// The texel of the column `column` on a map of `size` texels, wrapped around.
fn texel_of(column: vec2<f32>, size: vec2<u32>) -> vec2<i32> {
    let side = vec2<f32>(size);
    return vec2<i32>(column - side * floor(column / side));
}

// The height at which the column `column` stops what falls.
fn stop_height(column: vec2<f32>) -> f32 {
    return textureLoad(occlusion, texel_of(column, textureDimensions(occlusion)), 0).r;
}

// What falls on the column `column` out of the snow line: `RAIN`, `SNOW`, or `NOTHING`, also
// where the climate is not known yet.
fn column_kind(column: vec2<f32>) -> u32 {
    let climate = textureLoad(climate_map, texel_of(column, textureDimensions(climate_map)), 0);
    if climate.a == 0.0 {
        return NOTHING;
    }
    return u32(round(climate.b * 255.0));
}

// How much of a drop at `height`, which turns to snow at `line`, falls as snow.
fn snowiness(kind: u32, height: f32, line: f32) -> f32 {
    if kind == SNOW {
        return 1.0;
    }
    if kind == RAIN {
        return smoothstep(line - 1.5, line, height);
    }
    return 0.0;
}

// How far the wind has carried what falls since the start, along the ground plane, and how
// fast it blows now, for what started at `at`: the wind of the weather, in gusts that sweep
// the land in a storm. The gusts are the derivative of the carriage, so that nothing jumps.
fn wind_drift(at: vec2<f32>, time: f32) -> vec4<f32> {
    let gusty = weather.wind.z;
    let phase = dot(at, vec2(0.031, 0.017));
    let slow = 0.53;
    let fast = 1.37;
    let gust = 1.0 + gusty * (0.45 * sin(slow * time + phase) + 0.25 * sin(fast * time + at.x * 0.07));
    let carried = time - gusty * (0.45 * cos(slow * time + phase) / slow
        + 0.25 * cos(fast * time + at.x * 0.07) / fast);
    return vec4(weather.wind.xy * carried, weather.wind.xy * gust);
}

// The light of what falls: the grey of the overcast, a little darker than the sky, as a drop
// shows the sky above it and the darker world below, a glimmer even at night, and the flashes
// of the lightning.
fn sky_light() -> vec3<f32> {
    let sky = mix(weather.horizon.rgb, weather.zenith.rgb, 0.4);
    return sky * 0.85 + vec3(0.012, 0.013, 0.016) + weather.flash * vec3(0.8, 0.82, 1.0);
}

// The width of a pixel at `dist` from the eye, in blocks.
fn pixel_size(dist: f32) -> f32 {
    return 2.0 * dist / (view.clip_from_view[1][1] * view.viewport.w);
}

fn clip(at: vec3<f32>) -> vec4<f32> {
    // Unjittered: the TAA is done with
    return view.unjittered_clip_from_world * vec4(at, 1.0);
}

// The drop of rain `drop`, at its corner `corner`: a streak along its fall, turned to face the
// eye around it.
fn rain(drop: u32, corner: vec2<f32>) -> VertexOutput {
    let shown = f32(params.rain) * params.level;
    if f32(drop) >= shown {
        return hidden();
    }
    let r = random4(drop * 2u);
    let s = random4(drop * 2u + 1u);
    let band = rain_band(r.w);
    let near = r.w < 0.15;
    let extent = band.extent;
    let size = vec3(2.0 * extent.x, extent.y, 2.0 * extent.x);
    let time = globals.time;
    let speed = mix(12.0, 16.0, s.x);
    let start = r.xyz * size;
    let wind = wind_drift(start.xz, time);
    let eye = view.world_position;
    let low = eye - vec3(extent.x, extent.z, extent.x);
    let head = wrap(start + vec3(wind.x, -speed * time, wind.y), low, size);
    let axis = normalize(vec3(wind.z, -speed, wind.w));
    let streak_length = mix(0.6, 1.0, s.y);
    let centre = head - axis * streak_length * 0.5;

    let column = floor(head.xz);
    let kind = column_kind(column);
    let line = SNOW_LINE + (s.z - 0.5) * 2.0 * SNOW_BAND;
    var alpha = band.alpha * params.opacity * clamp(shown - f32(drop), 0.0, 1.0);
    alpha *= 1.0 - snowiness(kind, head.y, line);
    if kind != RAIN {
        alpha = 0.0;
    }
    let stop = stop_height(column);
    if head.y - axis.y * streak_length < stop {
        // Under the roof of its column, the drop has stopped
        alpha = 0.0;
    }
    let to_eye = eye - centre;
    let dist = max(length(to_eye), 0.01);
    let across = length(to_eye.xz);
    alpha *= smoothstep(band.fade.x, band.fade.y, dist)
        * (1.0 - smoothstep(band.fade.z, band.fade.w, across));
    // Faded in at the top of the box and out at its bottom
    let height = head.y - low.y;
    alpha *= smoothstep(0.0, 2.0, height) * (1.0 - smoothstep(extent.y - 3.0, extent.y, height));
    if alpha < 0.002 {
        return hidden();
    }

    var width = band.width;
    let thinnest = MIN_PIXELS * pixel_size(dist);
    if width < thinnest {
        // Fainter, but not as much as it is thinner: far off, the drops blur into each other
        alpha *= sqrt(width / thinnest);
        width = thinnest;
    }
    var side = cross(axis, to_eye / dist);
    if dot(side, side) < 1e-6 {
        side = vec3(1.0, 0.0, 0.0);
    }
    side = normalize(side);
    // The soft edge of a drop lies outside of its width
    let at = centre + side * corner.x * width + axis * corner.y * streak_length * 0.5;
    var out: VertexOutput;
    out.position = clip(at);
    out.uv = corner;
    out.world_y = at.y;
    out.color = vec4(sky_light() * mix(0.7, 1.1, s.w), alpha);
    out.kind = RAIN;
    out.shape = vec4(select(0.0, 1.0, near), 0.0, s.w, 0.0);
    out.stop = stop;
    return out;
}

// The flake of snow `flake`, at its corner `corner`: a soft round sprite facing the eye,
// drifting down with a lazy sway and a slow spin.
fn snow(flake: u32, corner: vec2<f32>) -> VertexOutput {
    let shown = f32(params.snow) * params.level;
    if f32(flake) >= shown {
        return hidden();
    }
    let r = random4(flake * 2u + 0x9e3779b9u);
    let s = random4(flake * 2u + 0x7f4a7c15u);
    let band = snow_band(r.w);
    let near = r.w < 0.3;
    let extent = band.extent;
    let size = vec3(2.0 * extent.x, extent.y, 2.0 * extent.x);
    let time = globals.time;
    let speed = mix(1.0, 2.0, s.x);
    let start = r.xyz * size;
    let wind = wind_drift(start.xz, time) * 0.8;
    let sway = mix(0.2, 0.55, s.y);
    let pace = mix(0.6, 1.3, s.z);
    let phase = s.w * 6.2832;
    let swaying = sway * vec2(sin(pace * time + phase), cos(0.83 * pace * time + 1.7 * phase));
    let eye = view.world_position;
    let low = eye - vec3(extent.x, extent.z, extent.x);
    let at = wrap(
        start + vec3(wind.x + swaying.x, -speed * time, wind.y + swaying.y),
        low,
        size,
    );

    let column = floor(at.xz);
    let kind = column_kind(column);
    let line = SNOW_LINE + (s.x - 0.5) * 2.0 * SNOW_BAND;
    var alpha = min(band.alpha * params.opacity, 1.0) * clamp(shown - f32(flake), 0.0, 1.0);
    alpha *= snowiness(kind, at.y, line);
    let stop = stop_height(column);
    if at.y < stop {
        alpha = 0.0;
    }
    let to_eye = eye - at;
    let dist = max(length(to_eye), 0.01);
    let across = length(to_eye.xz);
    alpha *= smoothstep(band.fade.x, band.fade.y, dist)
        * (1.0 - smoothstep(band.fade.z, band.fade.w, across));
    let height = at.y - low.y;
    alpha *= smoothstep(0.0, 2.0, height) * (1.0 - smoothstep(extent.y - 3.0, extent.y, height));
    if alpha < 0.002 {
        return hidden();
    }

    var radius = band.width * mix(0.45, 1.0, r.x * r.y);
    let smallest = 0.5 * MIN_PIXELS * pixel_size(dist);
    if radius < smallest {
        alpha *= (radius * radius) / (smallest * smallest);
        radius = smallest;
    }
    let spin = (s.y - 0.5) * 1.6 * time + phase;
    let turned = vec2(
        corner.x * cos(spin) - corner.y * sin(spin),
        corner.x * sin(spin) + corner.y * cos(spin),
    );
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let position = at + (right * turned.x + up * turned.y) * radius;
    var out: VertexOutput;
    out.position = clip(position);
    out.uv = corner;
    out.world_y = position.y;
    out.color = vec4(sky_light() * 1.35, alpha);
    out.kind = SNOW;
    out.shape = vec4(select(0.35, 0.6, near), 0.0, s.w, 0.0);
    out.stop = stop;
    return out;
}

// The splash `index`, at its vertex `vertex` of twelve: a ring on the ground (the first six)
// and droplets thrown up (the next six), on a quad that faces the eye. Each splash pops up at a
// new place each time round.
fn splash(index: u32, vertex: u32) -> VertexOutput {
    let shown = f32(params.splashes) * params.level;
    if f32(index) >= shown {
        return hidden();
    }
    let r = random4(index * 2u + 0x2545f491u);
    let period = mix(0.3, 0.6, r.x);
    let round_trip = globals.time / period + r.y;
    let age = fract(round_trip) * period / SPLASH_LIFE;
    if age >= 1.0 {
        return hidden();
    }
    let s = random4(index * 7919u + u32(floor(round_trip)) * 104729u);
    let eye = view.world_position;
    let low = eye.xz - SPLASH_REACH;
    let side = 2.0 * SPLASH_REACH;
    let place = low + fract((s.xy * side - low) / side) * side;
    let column = floor(place);
    let stop = stop_height(column);
    if column_kind(column) != RAIN || abs(stop) > UNKNOWN
        || stop > SNOW_LINE - SNOW_BAND {
        return hidden();
    }
    let on_water = fract(stop) > 0.01;
    let foot = vec3(place.x, stop, place.y);
    let to_eye = eye - foot;
    let dist = max(length(to_eye), 0.01);
    var alpha = clamp(shown - f32(index), 0.0, 1.0)
        * (1.0 - smoothstep(0.6 * SPLASH_REACH, SPLASH_REACH, length(to_eye.xz)))
        * smoothstep(0.6, 1.5, dist);

    let corner = corner_of(vertex % 6u);
    var out: VertexOutput;
    out.shape = vec4(0.0, age, s.z, select(0.0, 1.0, on_water));
    out.stop = -UNKNOWN;
    if vertex < 6u {
        if on_water {
            return hidden();
        }
        // The ring, flat on the ground, a little above it
        let reach = 0.13;
        let at = foot + vec3(corner.x * reach, 0.015, corner.y * reach);
        out.position = clip(at);
        out.uv = corner;
        out.world_y = at.y;
        out.color = vec4(sky_light() * 1.3, 0.22 * alpha);
        out.kind = SPLASH_RING;
        return out;
    }
    // The droplets, on a quad standing on the ground, facing the eye
    let across = normalize(vec3(-to_eye.z, 0.0, to_eye.x) + vec3(1e-5, 0.0, 0.0));
    let half_width = select(0.2, 0.14, on_water);
    let height = select(0.24, 0.16, on_water);
    let uv = vec2(corner.x * half_width, (corner.y * 0.5 + 0.5) * height);
    let at = foot + across * uv.x + vec3(0.0, uv.y, 0.0);
    out.position = clip(at);
    out.uv = uv;
    out.world_y = at.y;
    out.color = vec4(sky_light() * 1.4, 0.9 * alpha);
    out.kind = SPLASH_CROWN;
    return out;
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let rain_vertices = params.rain * 6u;
    if index < rain_vertices {
        return rain(index / 6u, corner_of(index % 6u));
    }
    let snow_index = index - rain_vertices;
    let snow_vertices = params.snow * 6u;
    if snow_index < snow_vertices {
        return snow(snow_index / 6u, corner_of(snow_index % 6u));
    }
    let splash_index = snow_index - snow_vertices;
    return splash(splash_index / 12u, splash_index % 12u);
}

// How much of the streak of a drop covers `uv`: soft across, its head round and its tail
// fading, as the eye smears a falling drop.
fn streak(uv: vec2<f32>, near: f32) -> f32 {
    let spread = mix(4.0, 2.5, near);
    let across = exp(-uv.x * uv.x * spread);
    let along = smoothstep(-1.0, 0.4, uv.y) * (1.0 - smoothstep(0.8, 1.0, uv.y));
    return across * along;
}

// How much of a flake covers `uv`: a soft disc with faint lobes, softer for the near ones,
// which are out of focus.
fn flake(uv: vec2<f32>, softness: f32, seed: f32) -> f32 {
    let dist = length(uv);
    let angle = atan2(uv.y, uv.x);
    let edge = 1.0 - 0.12 * (0.5 + 0.5 * cos(6.0 * angle + seed * 6.2832));
    return 1.0 - smoothstep(edge * (1.0 - softness), edge, dist);
}

// How much of the ring of a splash covers `uv` at `age`, from 0 to 1 of its life: a soft ring
// of water thrown out, gone by half of it.
fn ring(uv: vec2<f32>, age: f32) -> f32 {
    let spread = min(age * 2.0, 1.0);
    let radius = mix(0.2, 0.9, sqrt(spread));
    let width = mix(0.12, 0.25, spread);
    let off = (length(uv) - radius) / width;
    return exp(-off * off) * (1.0 - spread) * (1.0 - spread);
}

// How much of the droplets of a splash cover `uv` (in blocks from its foot) at `age`: five or
// six droplets thrown up and out, falling back.
fn droplets(uv: vec2<f32>, age: f32, seed: f32, on_water: bool) -> f32 {
    let time = age * SPLASH_LIFE;
    let scale = select(1.0, 0.7, on_water);
    let pixel = length(fwidth(uv));
    var cover = 0.0;
    let count = 5u + u32(seed * 2.0);
    for (var k = 0u; k < count; k++) {
        let r = random4(u32(seed * 65536.0) + k * 31u);
        let angle = (f32(k) + r.x * 0.6) * 6.2832 / f32(count);
        let out_speed = mix(0.3, 0.6, r.y) * scale;
        let up_speed = mix(1.3, 2.3, r.z) * scale;
        let at = vec2(cos(angle) * out_speed * time, up_speed * time - 9.0 * time * time);
        if at.y < -0.005 {
            continue;
        }
        let radius = mix(0.011, 0.018, r.w) * scale * (1.0 - 0.4 * age);
        // At least a pixel wide, and fainter then
        let wide = max(radius, pixel);
        let dist = length(uv - at);
        cover += (1.0 - smoothstep(wide * 0.3, wide, dist)) * (radius * radius) / (wide * wide);
    }
    // At first, a crown of water rises around the drop
    let rise = (uv.y - 0.02 * scale) / (0.012 * scale);
    let crown = exp(-rise * rise)
        * (1.0 - smoothstep(0.03, 0.09 * scale, abs(uv.x)))
        * (1.0 - smoothstep(0.0, 0.25, age));
    return min(cover + 0.6 * crown, 1.0);
}

// Whether the world drawn hides the fragment at `position`.
fn behind_world(position: vec4<f32>) -> bool {
    let world = textureLoad(depth, vec2<i32>(position.xy), 0);
    // The depth is reversed: nearer is greater
    return position.z < world;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.world_y < in.stop || behind_world(in.position) {
        discard;
    }
    var cover = 0.0;
    switch in.kind {
        case RAIN: {
            cover = streak(in.uv, in.shape.x);
        }
        case SNOW: {
            cover = flake(in.uv, in.shape.x, in.shape.z);
        }
        case SPLASH_RING: {
            cover = ring(in.uv, in.shape.y);
        }
        case SPLASH_CROWN: {
            cover = droplets(in.uv, in.shape.y, in.shape.z, in.shape.w > 0.5);
        }
        default: {
            discard;
        }
    }
    let alpha = in.color.a * cover;
    if alpha < 0.001 {
        discard;
    }
    return vec4(in.color.rgb * alpha, alpha);
}
