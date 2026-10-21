# Weather Lot 4 (The wet world) Implementation Plan

**Goal:** In High and Ultra, the rain soaks the world. The ground open to the sky darkens and
takes a soft sheen that catches the grey of the sky at a glance; puddles gather in the hollows
of the stone, the gravel, the paths and the planks as the ground soaks, near-mirrors of the
overcast with the rain's rings in them; the sides of the blocks under the rain darken in
streaks. Nothing gets wet under a roof, under the trees, in the snow or in the desert. When the
rain stops, the world dries over two minutes, the high spots first, the puddles last. To see:
`/weather rain` on a path in High, then `/weather clear`.

**Spec:** `docs/superpowers/specs/2026-10-21-weather-design.md`, "Lot 4 — the wet world", and
its "Decisions" (12, 16, 17 and 23 above all, and those of lots 2 and 3 up to 52).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments, as around.
- TDD for what can be tested without a GPU: the wetness and how it moves, what the shaders are
  told of it in each preset, the kind of surface of each block texture and its place in the
  table, the occlusion map bound to the chunks. The look is judged on the snapshots of
  `e2e::weather`, looked at and tuned until beautiful.
- Clear weather draws exactly as before in every preset, and Off, Low and Medium draw rain as
  in lot 3: the wetness reaches the shaders as 0 there, and every wet term is skipped at 0.
- `ChunkShading` gets no per-frame value: the wetness goes through `WeatherShading`.
- `timeout 600 cargo test -p client` while working; `timeout 900 just check` at the end.

## Design

- **Wetness.** `ClientWeather::wetness`, from 0 to 1: rises by `precipitation() / 40` a second
  (40 s to soaked under a full rain), falls by `1 / 120` a second while nothing falls (2 min to
  dry). `settled` gives 1 in rain and in a storm, so a player joining mid-rain finds the world
  soaked. Pure, tested on `advance`.
- **Presets.** `ShaderFeatures::wet`, true in High and Ultra only; `WeatherShading` grows
  `wetness`, the client's wetness where the preset draws it, 0 elsewhere.
- **What gets wet.** In `chunk.wgsl`, for the solid faces (not the liquids):
  - the column must take rain (the climate map's blue channel, below the snow line, band of
    ±4 as the drops);
  - the occlusion map of lot 3 is bound to the chunks at 108: a top face is wet where its
    column's stop height is not above it (the exposed top); a side face where the column in
    front of it lets the rain fall past it at that height (so the cliffs and the sides of a
    step are wet, the walls under an overhang dry), less than a top and in streaks running
    down; a bottom face never. Beyond the map (more than 40–48 blocks away), the sky light of
    15 stands in for it, faded across.
  - each block texture has a kind of surface, in bits of the `w` of its row in the table:
    hard (stone, cobblestone, gravel, planks, logs, ores, the default for a texture of a pack),
    soil (grass, dirt, moss), porous (sand), foliage (leaves, plants, cactus, vines: darker,
    a faint sheen, no puddle) and dry (glass, ice, snow, torches, the cracks): the darkening,
    the gloss and the puddles depend on it.
- **Wet surfaces.** Darker (albedo × 0.65 at full wetness, × 0.55 for sand, less for foliage)
  and glossy: the chunks keep a reflectance of 0 for Bevy's lighting (the environment map would
  add the blue sky twice), and the sheen is drawn by hand after the light: the sky seen through
  `sky_radiance` in the reflected direction, blurred by the roughness, by a Fresnel term that
  grows at grazing angles, and a soft lobe of the sun, shadowed.
- **Puddles.** A world-space noise on the exposed top faces of hard and soil surfaces, its
  threshold falling as the wetness rises past half, held back from the edges where the ground
  drops (the occlusion map of the neighbours): inside, the ground seen through the water,
  darker, and a near-mirror of the sky by the Fresnel of water, its normal rippled by the
  rings of lot 3 (`rain_ripples`) while it rains, catching `weather.flash`. Soft edges, the
  width of a pixel at least (`fwidth`), the detail fading out with the footprint of a pixel.
  Snapping the noise to the 16 × 16 texels of the blocks or not is decided on the pictures.
- **Drying.** While it rains, the ground is wet all over; once it stops, it dries by the same
  noise, the high spots first, so the puddles and the ground around them stay wet longest.

## Tasks

### Task 1: The wetness (`weather.rs`)
- [ ] Tests: it rises to 1 in 40 s of full rain and dries in 2 min; light rain soaks slower;
  settled in rain is soaked, in clear dry; a rain caught part way keeps its wetness. See them
  fail.
- [ ] `wetness`, moved in `advance`. Commit `feat(client): the ground soaks in rain and dries
  after`.

### Task 2: The wetness reaches the shaders in High and Ultra (`shaders.rs`, `overcast.rs`)
- [ ] Tests: `ShaderFeatures::wet` in High and Ultra only; `WeatherShading::wetness` is the
  wetness in those, 0 in the others; the buffer the system writes holds it. See them fail.
- [ ] The field, in Rust and WGSL. Commit `feat(client): the shaders are told how wet the
  world is`.

### Task 3: The surfaces and the occlusion map for the chunks (`materials.rs`)
- [ ] Tests: the kind of surface of each base texture (stone hard, grass soil, sand porous,
  leaves foliage, glass dry, an unknown texture hard); its bits in the table, the fading bit
  kept; the chunk materials bind the occlusion map. See them fail.
- [ ] `WetSurface`, the table, binding 108. Commit `feat(client): the chunks know what gets
  wet`.

### Task 4: Wet surfaces (`chunk.wgsl`)
- [ ] Exposure (climate, occlusion, faces), darkening, sheen. Commit `feat(client): rain
  darkens and soaks the ground`.

### Task 5: Puddles (`chunk.wgsl`)
- [ ] Noise, mask, mirror, ripples, flash. Commit `feat(client): puddles gather in the rain`.

### Task 6: Pictures (`e2e/weather.rs`)
- [ ] A terrace of stone, cobblestone, gravel, a dirt path and planks among the grass by the
  lake: noon rain, soaked (High, Ultra, Medium), half dried under a clearing sky, under a
  roof, sunset after rain, a close-up of a puddle with rings. Commit `test(client): pictures
  of the wet world`.
- [ ] Look at every picture, boards dry against wet, tune until beautiful; commit each tuning.
  Check that the clear pictures and Medium are unchanged.

### Task 7: Closing
- [ ] Decisions appended to the spec; `just check`; commit `docs: the wet world of the weather
  (lot 4)`.
