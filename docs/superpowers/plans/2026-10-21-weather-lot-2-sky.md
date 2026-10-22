# Weather Lot 2 (The sky) Implementation Plan

**Goal:** The sky follows the weather. Rain rolls in under thicker, darker clouds that close
over the sky without a hitch; a soft grey overcast hides every bit of blue (the sky, the fog,
the water's reflection, the horizon ring, the light of the sky); the sun, the moon and the
stars fade out; the sun dims and the shadows soften while an even grey light keeps the world
readable; the exposure, the saturation and the fog follow. A thunderstorm is darker and closer
still. To see: `/weather rain`, `/weather thunder`, `/weather clear`, 20 s each way.

**Spec:** `docs/superpowers/specs/2026-10-21-weather-design.md`, "Lot 2 — the sky", and its
"Decisions" (11, 17 and 18 above all).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments, as around.
- TDD for what can be tested without a GPU: levels, curves, colours, thresholds, fog
  distances, exposure, the cloud mesh, the systems' outputs. The look is judged on the
  snapshots of `e2e::weather`, looked at and tuned until beautiful.
- Clear weather draws exactly as before: every weather term is a `mix(.., .., 0)` or a factor
  of 1 at an overcast level of 0; the existing goldens do not change.
- `ChunkShading` gets no per-frame value: the weather goes through a buffer of its own.
- `timeout 600 cargo test -p client` while working; `timeout 900 just check` at the end.

## Design

- **Levels.** `ClientWeather::overcast()` is `smoothstep(0, 0.5, rain)`, `storm()` is
  `smoothstep(0, 0.5, thunder)`: the sky leads the drops, and a storm darkens as fast.
- **`world/rendering/overcast.rs`** holds the pure curves of the weather's sky (sun factor,
  exposure shift, fog range, saturation, ambient, environment light, visibility of the sun,
  moon and stars, the overcast colours by sun height and preset), the `WeatherShading` uniform
  and the overcast dome.
- **One buffer for the sky.** `WeatherShading { horizon, zenith, overcast }` lives in a
  `ShaderBuffer` (a storage buffer asset) held by `MaterialResource`, bound at binding 107 of
  the chunk materials, the horizon skirt and the dome. Written in place when it changes: the
  GPU buffer is kept (same size, `COPY_DST`), so no material changes and no chunk is prepared
  again. `sky.wgsl` declares it once: `sky_radiance` becomes the sky as seen, the physical sky
  blended to the overcast grey (the physical one is `clear_sky_radiance`), so every reader of
  the sky (fog, water, skirt) turns grey together.
- **The dome.** A sphere around the camera on the far plane, in the transparent pass, after
  the sun, the moon and the stars and before the clouds (depth bias), blended over the sky by
  the overcast level, with `overcast_sky(dir)`: lighter at the horizon, darker at the zenith.
- **Clouds.** The mesh holds every cell denser than the storm threshold, each face with the
  densities of its two cells (`high`, `low`, a vertex attribute); the vertex shader collapses
  a face unless `low <= threshold < high`. Densities above the clear threshold count as
  "always", below the storm one as "never", so faces merge along runs as before in clear
  weather. Thresholds measured on the field: 45 % at 0.031, 80 % at -0.256, 92 % at -0.41.
- **Light.** Sun and moon illuminance × 1 → 0.25 → 0.12; EV100 − 1.3 (rain) − 0.7 more
  (storm); ambient brighter and greyer, environment light of the atmosphere turned down;
  saturation 1.12 → 0.9; god rays fade with the sun.
- **Fog.** Start/end fractions of the render distance (0.6, 1.0) → (0.3, 0.75) → (0.15, 0.6),
  end never under 48 blocks (nor over the render distance); the clouds' fade scales the same.
  In the Off preset the fog and clear colour blend to the overcast grey.

## Tasks

### Task 1: The overcast level and its curves (`weather.rs`, `overcast.rs`)
- [x] Tests: `overcast()`/`storm()` follow `smoothstep(0, 0.5, ..)`; at 0 every curve is the
  clear value (sun 1, EV shift 0, fog (0.6, 1.0), saturation 1.12, visibility 1); rain gives
  sun 0.25, EV −1.3, fog (0.3, 0.75); storm 0.12, −2, (0.15, 0.6); the fog end never under
  48 blocks nor past the render distance, the start never past the end; the overcast colours
  are grey (no channel far from the others), lighter at the horizon, brighter by day than at
  dusk, darker in a storm, continuous over the day. See them fail.
- [x] The functions. See them pass. Commit `feat(client): the overcast level and its curves`.

### Task 2: The clouds thicken (`clouds/`)
- [x] Tests: the measured thresholds give 45 %, 80 % and 92 % of the cells (±3 %); the
  threshold follows the overcast and storm levels; `quads` takes clamped densities: a face
  shows at a threshold when exactly one of its cells is above it (checked against `is_full`
  at the clear threshold: same faces, merged as before); every cell of the storm coverage has
  its faces; the mesh carries the densities; Fast style likewise; the rain tint is a
  blue-grey darker in a storm, still lit by the hour; opacity 0.8 → 0.95. See them fail.
- [x] `field::density` public with `CLEAR/RAIN/STORM_THRESHOLD`, `cloud_threshold`, the
  attribute and the shader collapse, `CloudParams { threshold, .. }`, tint and opacity from
  `ClientWeather`. See them pass. Commit `feat(client): the clouds thicken in rain`.

### Task 3: The overcast sky (`materials.rs`, `sky.wgsl`, `chunk.wgsl`, `horizon.wgsl`, dome)
- [x] Tests: the weather buffer is made with the materials and bound to both chunk
  materials; the system writes the levels and colours into it only when they change, and the
  handle stays the same; the dome follows the camera with the physical sky only, and goes in
  the Off preset. See them fail.
- [x] `WeatherShading`, the buffer, binding 107, `sky_radiance` blended, the Off water
  reflection blended, the dome material and shader. See them pass. Commit `feat(client): an
  overcast sky hides the blue`.

### Task 4: The light of a rainy day (`shaders.rs`, `celestial.rs`, `graphics.rs`)
- [x] Tests: in rain the sun's illuminance, the exposure, the ambient, the environment light
  and the saturation follow the curves (system tests, with and without the physical sky);
  the sun, the moon and the stars fade out by 0.8; clear weather leaves them as before. See
  them fail.
- [x] The systems. See them pass. Commit `feat(client): rain dims the sun and greys the
  light`.

### Task 5: The fog closes in (`underwater.rs`, clouds' fade)
- [x] Tests: the fog's start and end follow the weather, the floor at 48 blocks, the
  underwater fog unchanged; in Off the fog and clear colour turn grey; the clouds' fade
  follows. See them fail.
- [x] The systems. See them pass. Commit `feat(client): the fog closes in under rain`.

### Task 6: Pictures (`e2e/weather.rs`)
- [x] `snapshot_the_weather`: a landscape with water at noon clear / rain / thunder from the
  ground, looking ahead and up; sunset and midnight in rain; mid-transition (overcast 0.5);
  rain in the Off preset. Commit `test(client): pictures of the weather`.
- [x] Look at every picture, tune colours, levels and fog until beautiful; commit each
  tuning as `feat(client): ...` or `fix(client): ...`.

### Task 7: Closing
- [x] Spec decisions appended; `just check`; commit `docs: the sky of the weather (lot 2)`.
