// The sky of the atmosphere, the physical sky of the shaders (`shaders.rs`), read
// from its environment map, under the overcast of the weather (`overcast.rs`): the chunks fade
// into it, the water reflects it, and the horizon skirt hides the ground of the planet with it.
// A material that imports this binds `WeatherShading` at 107.

#define_import_path rustcraft::sky

#import bevy_pbr::mesh_view_bindings::{view, light_probes}
#ifdef ENVIRONMENT_MAP
#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY
#import bevy_pbr::mesh_view_bindings::{specular_environment_maps, environment_map_sampler}
#else
#import bevy_pbr::mesh_view_bindings::{specular_environment_map, environment_map_sampler}
#endif
#endif

#import rustcraft::weather::{Weather, FLASH_COLOR}

// `WeatherShading` in `overcast.rs`
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var<storage, read> weather: Weather;

// `v` turned by the quaternion `q`.
fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

// The physical sky toward `dir`, as the environment map of the atmosphere holds it, at the
// exposure of the view, however much it lights the world; `blur` from 0, sharp, to 1, the
// roughest of its mips. None without the map.
fn clear_sky_radiance(dir: vec3<f32>, blur: f32) -> vec3<f32> {
#ifdef ENVIRONMENT_MAP
    if light_probes.view_cubemap_index < 0 {
        return vec3(0.0);
    }
    var at = quat_rotate(light_probes.view_rotation, dir);
    // Cube maps are left-handed
    at.z = -at.z;
#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY
    let index = light_probes.view_cubemap_index;
    let level = blur * f32(textureNumLevels(specular_environment_maps[index]) - 1u);
    let radiance = textureSampleLevel(
        specular_environment_maps[index], environment_map_sampler, at, level,
    ).rgb;
#else
    let level = blur * f32(light_probes.smallest_specular_mip_level_for_view);
    let radiance = textureSampleLevel(specular_environment_map, environment_map_sampler, at, level).rgb;
#endif
    return radiance * view.exposure;
#else
    return vec3(0.0);
#endif
}

// How bright a flash of lightning makes the overcast sky, on screen: a little all over, and
// most toward the flash in the clouds.
const FLASH_SKY: f32 = 0.22;
const FLASH_SKY_TOWARD: f32 = 1.1;

// The overcast sky toward `dir`, as seen on screen: lighter at the horizon, darker up high,
// and at sunrise and sunset, warm low toward the sun; lit by a flash of lightning, most toward
// it.
fn overcast_sky(dir: vec3<f32>) -> vec3<f32> {
    let up = smoothstep(0.0, 0.7, dir.y);
    let toward = max(dot(dir.xz / max(length(dir.xz), 1e-4), weather.sun.xy), 0.0);
    let glow = weather.glow.rgb * toward * toward * toward * (1.0 - smoothstep(-0.05, 0.45, dir.y));
    var sky = mix(weather.horizon.rgb, weather.zenith.rgb, up) + glow;
    if weather.flash > 0.0 {
        let lobe = pow(max(dot(dir, weather.flash_dir.xyz), 0.0), 5.0);
        sky += FLASH_COLOR * weather.flash * (FLASH_SKY + FLASH_SKY_TOWARD * lobe);
    }
    return sky;
}

// The sky toward `dir` as seen: the physical sky, under the overcast of the weather.
fn sky_radiance(dir: vec3<f32>, blur: f32) -> vec3<f32> {
    return mix(clear_sky_radiance(dir, blur), overcast_sky(dir), weather.overcast);
}

// Whether the view has a sky to read: the environment map of the atmosphere.
fn has_sky() -> bool {
#ifdef ENVIRONMENT_MAP
    return light_probes.view_cubemap_index >= 0;
#else
    return false;
#endif
}

// `dir`, raised to just above the horizon if it points under it: under the horizon, past the
// chunks, is the ground of the planet, which the horizon skirt hides.
fn horizon_direction(dir: vec3<f32>) -> vec3<f32> {
    return normalize(vec3(dir.x, max(dir.y, 0.03), dir.z));
}
