// What the shaders know of the weather: `WeatherShading` in `overcast.rs`. The materials that
// show the sky bind it at 107 (`rustcraft::sky`); the pass of the rain and the snow has a
// binding of its own (`precipitation.wgsl`).

#define_import_path rustcraft::weather

struct Weather {
    // The overcast sky at the horizon and at the zenith, as seen on screen, in linear RGB
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    // The glow of a low sun at the horizon toward it, and in `xy` the direction of the sun on
    // the ground plane
    glow: vec4<f32>,
    sun: vec4<f32>,
    // The wind along the ground plane in `xy`, in blocks per second, and in `z` how gusty it is
    wind: vec4<f32>,
    // The strike flashing: the top of the block struck in `xyz`, and in `w` how bright its
    // channel is now, from 0 to 1, however far off it is
    strike: vec4<f32>,
    // The direction from the eye to the flash in the clouds over the strike
    flash_dir: vec4<f32>,
    // From 0 to 1, how overcast the sky is
    overcast: f32,
    // From 0 to 1, how much rain or snow falls
    rain: f32,
    // From 0 to 1, how stormy it is
    storm: f32,
    // How bright a flash of lightning lights the world, from 0 to 1: less the farther it is
    flash: f32,
    // From 0 to 1, how wet the world open to the rain is: 0 in the qualities that draw it dry
    wetness: f32,
}

// The colour of the light of a flash of lightning: a cold white.
const FLASH_COLOR: vec3<f32> = vec3(0.86, 0.88, 1.0);
