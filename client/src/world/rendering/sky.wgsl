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

// `WeatherShading` in `overcast.rs`
struct Weather {
    // The overcast sky at the horizon and at the zenith, as seen on screen, in linear RGB
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    // From 0 to 1, how overcast the sky is
    overcast: f32,
}
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

// The overcast sky toward `dir`, as seen on screen: lighter at the horizon, darker up high.
fn overcast_sky(dir: vec3<f32>) -> vec3<f32> {
    let up = smoothstep(0.0, 0.7, dir.y);
    return mix(weather.horizon.rgb, weather.zenith.rgb, up);
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
