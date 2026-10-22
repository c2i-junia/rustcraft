# Weather Lot 3 (Precipitation) Implementation Plan

**Goal:** Rain and snow fall around the player. Fine, soft, translucent streaks, big and faint
near the eye and fine far off, slanted by a wind that follows the clouds and gusts in a storm;
quiet flakes that drift and sway in the cold biomes and above the snow line; nothing in the
deserts; nothing under a roof, a tree or the water's surface. Tiny crowns of droplets splash
on the ground near the player, and rings spread on the water. To see: `/weather rain`,
`/weather thunder`, in a forest, on a lake, in the snow, under a roof.

**Spec:** `docs/superpowers/specs/2026-10-21-weather-design.md`, "Lot 3 — precipitation", and
its "Decisions" (8, 9, 10, 15, and 23 to 37 of lot 2).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments, as around.
- TDD for what can be tested without a GPU: the level of the drops, the kind of a column, the
  height that stops the precipitation, the occlusion map (its texels, its slots, its updates as
  chunks come and blocks change and the camera moves), the counts by preset, what is handed to
  the GPU. The look is judged on the snapshots of `e2e::weather`, looked at and tuned until
  beautiful.
- Clear weather draws exactly as before: no drop is drawn at a level of 0, the ripples are a
  factor of the level; the existing goldens do not change.
- No work per drop on the CPU: the CPU keeps the occlusion map (a few columns when a block
  changes, a row of chunk columns when the camera crosses a chunk) and a handful of values.
- `ChunkShading` gets no per-frame value: the drops' level reaches the water through
  `WeatherShading` (decision 23).
- `timeout 600 cargo test -p client` while working; `timeout 900 just check` at the end.

## Design

- **Levels.** `ClientWeather::precipitation()` is `smoothstep(0.25, 1, rain)`: the drops follow
  the sky. `WeatherShading` grows `rain` (that level), `storm`, `flash` (0 until lot 5's
  lightning) and `wind` (the direction of the clouds' drift, +X, and its speed, gustier in a
  storm), so that the drops, the splashes and the ripples read one buffer.
- **The kind of a column** (rain, snow, none) is written by the climate map's task in the blue
  channel of its texel, from the column's own biome: snow in `IcePlain`, none in `Desert` and
  `Savanna`, rain elsewhere. The snow line is a height, so it is applied to each drop by its
  own height: a drop above 122 (give or take a few blocks, drawn per drop) falls as a flake. A
  column whose surface is above the line gets only flakes; from a summit, the snow turns to
  rain lower down the valley.
- **The occlusion map** (`world/precipitation/occlusion.rs`): a 144 × 144 `R32Float` texture,
  9 × 9 chunk columns around the camera's, wrapped (column `(x, z)` at `(x, z)` modulo 144):
  for each column, the height at which the precipitation stops, the top of its highest block
  but air and the decorations (leaves, glass, cactus stop it; water stops it at its surface);
  an unknown column holds a height that stops everything. The columns of a chunk column are
  scanned again when one of its chunks comes or goes and when it enters the window; one
  column when a block of it changes. The image changes only then.
- **Drawing.** No mesh: a pass of its own (`world/precipitation/render.rs`), in Bevy's
  `Core3d` schedule after the TAA and before the bloom and the tonemapping, draws the drops,
  the flakes and the splashes from the vertex index alone. The TAA keeps 85 to 98 % of the
  frame before for a pixel without motion vectors: thin blended streaks drawn before it would
  fade into a haze. The pass reads the view's depth (multisampled in Off) to hide what is
  behind the world, and softens the splashes against it. It is drawn in every preset.
- **Rain.** Two bands, wrapped around the camera in world space: near (up to 10 blocks, wider,
  softer, fainter) and far (8 to 40 blocks, fine). Each drop has its speed (12–16 blocks/s),
  length (0.6–1 block), place and phase from a hash of its index; it is billboarded around its
  fall axis, slanted by the wind; at least 1.2 pixels wide, its alpha scaled down by what it
  lacks. Lit by the overcast grey, brighter on the darker ground than on the sky, and by
  `flash`. Hidden under the stop height of its column, past 40 blocks, near the edges of its
  box, and where the column is not rain.
- **Snow.** Flakes out to 48 blocks, 1–2 blocks/s, a lazy sway on two axes, a slow spin of a
  soft round sprite with faint lobes, varied in size; carried by the wind.
- **Splashes.** Within 16 blocks, on rain columns: each splash takes a new place every cycle,
  a thin ring spreading on the ground and four to six droplets thrown up and falling back in
  a quarter of a second. Smaller on the water.
- **Counts.** Rain: Off 2 500, Low 3 500, Medium 5 000, High 6 500, Ultra 8 000. Snow and
  splashes scale alike. The drops drawn are the first `count × precipitation()` of them, the
  last fading in, so that the rain starts with a few drops.
- **Ripples.** In `water_normal`, rings spreading from points hashed in a grid of cells, each
  with its own phase and only a share of the cells live, the share following the drops'
  level; on rain columns (the climate map) open to the sky (sky light 15); faded out where a
  pixel spans too much, as the waves.
- **Underwater**, and outside of the game, nothing is drawn.

## Tasks

### Task 1: The level of the drops and the wind (`weather.rs`, `overcast.rs`, `sky.wgsl`)
- [x] Tests: `precipitation()` is 0 up to a rain of 0.25, 1 at 1, rising gently; the wind
  blows along +X like the clouds, stronger in a storm; `WeatherShading` holds the drops'
  level, the storm, a flash of 0 and the wind, and is the same as before in clear weather but
  for them. See them fail.
- [x] The functions and fields, `Weather` in WGSL in a module of its own that both `sky.wgsl`
  and the precipitation import. Commit `feat(client): the level of the drops and the wind`.

### Task 2: The kind of each column (`climate.rs`)
- [x] Tests: snow in the ice plains, none in deserts and savannas, rain elsewhere (oceans,
  forests, mountains); the climate texels carry the kind of their own column, unblended. See
  them fail.
- [x] `PrecipitationKind`, `precipitation_kind(biome)`, the blue channel. Commit
  `feat(client): the climate map tells rain from snow`.

### Task 3: The occlusion map (`precipitation/occlusion.rs`)
- [x] Tests: the stop height of a column (air below a roof, leaves, glass, water at its
  surface, grass and flowers let it through, an empty column, a column not loaded); the texel
  of a column, wrapped, west and north of the origin too; the window of chunk columns around
  the camera; a chunk that comes, a block placed or broken, a camera that crosses a chunk
  rewrite only what they change, and the image only then. See them fail.
- [x] `OcclusionMap`, `stop_height`, the system. Commit `feat(client): the height that stops
  the rain around the camera`.

### Task 4: The pass of the drops (`precipitation/mod.rs`, `render.rs`, `precipitation.wgsl`)
- [x] Tests: the counts by preset, Off 2 500 to Ultra 8 000, more for each quality; what is
  extracted for the GPU: the handles, the counts, the drops' level, hidden under water and
  with no rain. See them fail.
- [x] The extraction, the pipeline, the bind group and the draw after the TAA; the camera's
  depth readable. Rain first. Commit `feat(client): rain falls around the camera`.
- [x] Snow. Commit `feat(client): snow drifts down in the cold`.
- [x] Splashes. Commit `feat(client): the rain splashes on the ground`.

### Task 5: Ripples (`chunk.wgsl`)
- [x] Rings in `water_normal` as above. Commit `feat(client): the rain rings the water`.

### Task 6: Pictures (`e2e/weather.rs`)
- [x] Noon rain looking at the lake, noon thunder, a close-up of the splashes, the snow plain
  of seed 147 while it snows, under a roof looking out, midnight rain, rain in Off, rain in
  Ultra. Commit `test(client): pictures of the rain and the snow`.
- [x] Look at every picture (several frames for the TAA), boards side by side, tune until
  beautiful; commit each tuning. Measure the frame on lavapipe with and without rain.

### Task 7: Closing
- [x] Decisions appended to the spec; `just check`; commit `docs: the precipitation of the
  weather (lot 3)`.
