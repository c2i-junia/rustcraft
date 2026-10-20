# Clouds Lot 5 (Day/night sky) Implementation Plan

**Goal:** The sky follows the time of day, so that the clouds of the night do not sit on a
light blue sky: blue by day, warm near the horizon at sunrise and sunset, dark at night.

**Spec:** `docs/superpowers/specs/2026-10-20-clouds-design.md`, the "Day/night sky" paragraph
and lot 5.

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- `cargo test -p client`, `cargo clippy -p client --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at the commit.

## Decisions

1. **`sky_color(tick)`** in `camera/underwater.rs`, next to `SKY_COLOR`, built as the tint of
   the clouds is: from the height of the sun (`sun_height`, moved to `celestial.rs` with
   `smoothstep` and shared with `cloud_tint`), night blue (sRGB 10, 13, 26) to `SKY_COLOR`,
   then towards a warm orange the nearer the sun is to the horizon. `SKY_COLOR` is the colour
   at noon, the night blue the one at midnight.
2. **One colour for the clear colour and the fog of the sky**, as before: the world still
   fades into the sky at the end of the render distance.
3. **The fog is changed in place**: it now changes every tick, so `underwater_fog_system`
   mutates the `DistanceFog` it finds, and inserts one only when the camera has none. The
   fog and the clear colour are written only when they differ, so that nothing is marked
   changed for nothing. Under water, the colour stays `UNDERWATER_COLOR`.
4. **The camera spawns with `SKY_COLOR`** for the loading screen; the first frame of the game
   gives it the colour of the hour. `bench.rs` keeps the fixed `SKY_COLOR`.

## Tasks

### Task 1: The colour of the sky
- [x] Tests: `SKY_COLOR` at noon, the night blue at midnight, warm at sunrise and sunset,
  between day and night and back without jumps.
- [x] `sun_height`, `smoothstep` in `celestial.rs`; `sky_color`.

### Task 2: The fog and the clear colour follow it
- [x] Tests: the fog and the clear colour of the sky follow the hour; under water they do
  not; the fog is mutated in place, not inserted again. The existing underwater test keeps
  its intent at noon.
- [x] `underwater_fog_system` reads `ClientTime`.

### Task 3: A look at it
- [x] The snapshots of lot 3, with the sunset and midnight above the clouds too; looked at.
