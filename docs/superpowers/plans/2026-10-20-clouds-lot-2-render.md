# Clouds Lot 2 (Fancy clouds in the sky) Implementation Plan

**Goal:** The Fancy clouds drawn in the sky of the game: their material and shader, the plugin
that spawns them, rebuilds their mesh when the camera changes cell and slides them with the
world clock, and their tint after the time of day.

**Spec:** `docs/superpowers/specs/2026-10-20-clouds-design.md`, the `material.rs` and `mod.rs`
paragraphs of "Architecture"; lot 1's `is_full`, `drift`, `cloud_mesh` and `ATTRIBUTE_SHADE`.

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD where the code can be tested without a GPU: the tint, the reach, the rebuilds.
- `cargo test -p client`, `cargo clippy -p client --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at the commit.

## Decisions (where the spec is silent, or departs from it)

1. **The drift moves the entity, not the vertices in the shader.** The spec has the vertex
   shader add a drift uniform to x. But Bevy culls a mesh by its bounding box and sorts the
   transparent meshes by the centre of that box, both taken through the entity's `Transform`:
   with the drift only in the shader, both would be off by up to a period (3 072 blocks), the
   clouds culled when the drift took their box out of the view, and sorted against the water
   by a point kilometres away. So the mesh stays in pattern space and the entity's
   `Transform` is `(drift, 0, 0)`, set every frame; the shader uses the model matrix as any
   mesh does. Same result on screen, no rebuild either.
2. **Smooth drift**: `ClientTime` counts ticks in `FixedPostUpdate`; the drift adds the
   fraction of a tick already elapsed (`Time<Fixed>::overstep_fraction`) times the drift of a
   tick, so that the clouds slide every frame.
3. **Reach** = 2 × the effective render distance (`min(RenderDistance,
   ServerRenderDistanceLimit)`) in blocks, capped at 1 024; the fade runs from 60 % of it to
   it, on the horizontal distance to the camera. The window of cells has a radius of
   `ceil(reach / CELL) + 1` cells, so that it covers the reach from anywhere in the camera's
   cell.
4. **Rebuild** only when the window changes: the camera's pattern-space cell
   `(floor((x - drift) / CELL), floor(z / CELL))` or the radius. A rebuild gives the entity a
   new mesh handle; the old mesh is dropped with its handle.
5. **Tint** after the height of the sun, computed with the angle `celestial.rs` turns the sky
   by (`sun_angle`, extracted from `update_celestial_bodies`): grey blue `(0.17, 0.2, 0.29)`
   at night, white by day, mixed with a warm orange when the sun is near the horizon. The
   colour is sRGB, handed to the shader in linear.
6. **Unlit, unfogged, untonemapped**: the fragment shader writes `tint × shade` with alpha
   `0.8 × (1 − smoothstep(fade_start, fade_end, distance))` itself, without Bevy's PBR
   functions, so neither the lights, the chunks' `DistanceFog` nor the tonemapping touch it
   (the clear colour of the sky is not tonemapped either). `NotShadowCaster`,
   `NotShadowReceiver`, and the material has neither prepass nor shadows.
7. **The camera** is the `Camera3d`: the game has one.

## Tasks

### Task 1: Tint
- [x] `celestial::sun_angle(tick)`, used by `update_celestial_bodies`.
- [x] Tests: `the_clouds_are_white_at_noon`, `the_clouds_are_grey_blue_at_midnight`,
  `the_clouds_are_warm_at_sunrise_and_sunset`. See them fail; `cloud_tint(tick)`; pass.

### Task 2: Reach
- [x] Tests: `the_reach_is_twice_the_render_distance`,
  `the_reach_follows_the_server_limit`, `the_reach_is_capped`. See them fail; `reach`; pass.
- [x] Tests: `the_cell_of_the_camera_is_taken_in_the_drifted_pattern`,
  `the_smooth_drift_moves_on_between_two_ticks`. See them fail; `camera_cell`,
  `smooth_drift`; pass.

### Task 3: Material and shader
- [x] `material.rs`: `CloudMaterial { params: CloudParams { tint, opacity, fade_start,
  fade_end } }`, `AlphaMode::Blend`, no culling, vertex layout of position and shade.
- [x] `clouds.wgsl`, loaded with `load_internal_asset!` as `chunk.wgsl`.

### Task 4: Plugin
- [x] Tests (App with `MinimalPlugins`, `StatesPlugin`, `AssetPlugin`, `Assets<Mesh>` and
  `Assets<CloudMaterial>`): `the_clouds_appear_on_entering_the_game_and_leave_with_it`,
  `the_mesh_is_kept_while_the_camera_stays_in_its_cell`,
  `the_mesh_is_rebuilt_when_the_camera_changes_cell`, `the_clouds_slide_with_the_clock`,
  `the_uniforms_follow_the_reach_and_the_hour`. See them fail.
- [x] `clouds_plugin`: spawn on `OnEnter(GameState::Game)` with `DespawnOnExit`,
  `rebuild_clouds_system`, `drift_clouds_system`, `cloud_uniforms_system`, registered from
  `world_plugin`. Remove lot 1's `#![allow(dead_code)]`.

### Task 5: A look at them
- [x] `e2e/clouds.rs`, ignored like `snapshot_the_horizon`: a new world at render distance
  12, the camera placed by hand on the ground looking up, inside the layer and above it,
  saved with `UI_SNAPSHOTS=1`. Seen: blocky slabs, faded out at the reach; from above, the
  faces behind the top show through it (lot 3).

### Task 6: Checks and commit
- [x] `cargo test -p client`, clippy, fmt; commit `feat(client): clouds in the sky`.
