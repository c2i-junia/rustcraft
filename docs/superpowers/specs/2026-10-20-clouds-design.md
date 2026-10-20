# Clouds

Approved proposal: https://claude.ai/artifact/CyLm2gaRBA8wRXVTcSNFo9 (2026-10-20). Branch
`feat/clouds`, worktree `feat-clouds`.

## Goal

A layer of block clouds in the sky, like Minecraft's "Fancy" clouds: slabs of 12 × 12 blocks,
4 blocks high, at y = 192, drifting east with the world clock and tinted by the time of day.
Every player of a server sees the same clouds at the same place, with no change to the
protocol. A day/night sky colour comes with it (lot 5), so that night clouds do not sit on a
light blue sky.

## Decisions

| Topic | Choice |
|---|---|
| Style | Block clouds (Fancy); a flat layer (Fast); none (Off). Default Fancy. |
| Cells | 12 × 12 blocks, 4 high, bottom at y = 192 (the highest ridges stay under ~180). |
| Pattern | Periodic 2D field of `PERIOD = 256` cells (3 072 blocks), `noise` crate (`SuperSimplex` or value noise, 3 octaves), **fixed seed**: the same sky in every world. |
| Coverage | ~45 % of the cells full: a constant threshold. |
| Drift | 0.5 block/s towards +X, from `ClientTime` (`ticks / TICKS_PER_SECOND * 0.5`), wrapped modulo `PERIOD * 12` so `f32` keeps its precision. |
| Reach | 2 × the effective render distance (the one the server grants), capped at 1 024 blocks, faded out over its last 40 %. |
| Face shades | top 1.0, north/south (±Z) 0.9, east/west (±X) 0.8, bottom 0.7; opacity 0.8. |
| Tint | From the sun's height: white by day, warm near the horizon, grey blue at night. |
| Lighting | Unlit. No shadows cast or received. Not affected by the chunks' `DistanceFog`. |
| Network | None. |

## Architecture

New module `client/src/world/clouds/`:

- `field.rs`: pure. `is_full(cell: IVec2) -> bool`, deterministic and periodic.
- `mesh.rs`: pure. Builds the `Mesh` of the window of cells within a radius around a centre
  cell, in "pattern space" (cell `c` spans blocks `c * 12 .. c * 12 + 12`). Fancy: outer
  faces only (no face between two full neighbours), tops and bottoms merged along each row
  of +X runs. Fast: one quad per run at the slab's mid height. The face shade is a vertex
  attribute.
- `material.rs` + `clouds.wgsl`: `CloudMaterial` (`Material`, `AlphaMode::Blend`, no culling)
  with uniforms tint, drift offset, opacity, fade start/end. The vertex shader adds the drift
  to x; the fragment shader multiplies tint × shade and fades alpha with the horizontal
  distance to the camera. A depth-only variant (colour writes off, opaque pass) draws first,
  so that only the nearest cloud surface is coloured: uniform opacity from below, above and
  inside.
- `mod.rs`: the plugin. Spawns the clouds on entering `GameState::Game`
  (`DespawnOnExit(GameState::Game)`), rebuilds the mesh only when the camera's pattern-space
  cell changes (camera x minus drift) or the reach or style changes, updates the uniforms
  every frame. Day tint and sun height use the same time formula as `celestial.rs`.

Settings: `video.ron` gains `clouds: Fancy | Fast | Off`. An old file without it loads as
Fancy; saving the render distance keeps the clouds setting and the other way round. The
Video tab gets a "Clouds" row with the existing segmented widget; changing it in game
rebuilds the clouds at once; `menu_video.png` golden updated.

Day/night sky (lot 5): the clear colour and the sky fog follow the time of day (blue by day,
warm near the horizon at sunrise and sunset, dark night). Under water nothing changes.
`SKY_COLOR` stays the day colour.

## Lots

1. **Field and mesh**: pure code, unit tests (determinism, periodicity, coverage near the
   target, an isolated cell has 6 faces, two neighbours share no face, a full row has one
   top, the drift at a tick and its wrap, Fast has one quad per run).
2. **Fancy clouds in the sky**: material, shader, plugin, tint (white at noon, grey blue at
   midnight, tested like `celestial.rs`), rebuild only on cell change, fade at the reach.
3. **Depth prepass and inside view**: the depth-only material; checked by eye from below,
   inside and above with screenshots.
4. **The Fancy / Fast / Off setting**: `video.ron`, Video tab row, in-game switch, golden.
5. **Day/night sky**: clear colour and fog follow the hour; underwater unchanged.

Each lot: a plan in `docs/superpowers/plans/`, TDD, `cargo test` and `cargo clippy` clean,
review before the next lot. Nothing merged or pushed without asking the user.
