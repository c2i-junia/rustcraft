# Portals, lot 9: a seamless fog through close twins

Requirements and evidence: `.superpowers/sdd/portals/seam-diagnosis.md` (cause 1, F1 and F5).
Branch `feat/portals`, from `2dcb080c`.

## Goal

Walking through a portal is seamless at Off, Low and Medium when the twin is close, the nominal
case: the portal camera then gets the player's own fog. Out of scope: the god-ray haze (F2) and
SSAO (F3), the next lot; preloading the far side (F4); the dark horizon colour (F6).

## Steps

1. **F1, the rule (TDD).** In `portal/camera.rs`, `portal_fog` takes how far past the twin the
   chunks the client holds reach, as the camera sees through the twin (`held_reach`), instead
   of the blind `PORTAL_VIEW_RADIUS`. The chunks held are two squares of columns: the player's
   own (its render distance around its chunk) when the twin's chunk is in it, and the one the
   server sends around the twin (`PORTAL_VIEW_RADIUS`). For each square, the reach is the
   distance from the twin's centre to its nearest edge, along x and z as the player's own fog
   is, leaving out the edge behind the twin and dividing each edge's distance by how directly
   the camera's view cone can point at it. The fog ends `behind + reach` and starts at
   `FOG_START` of that, never farther than the player's: when the reach covers the player's
   fog, it is the player's fog unclamped.
   → verify: unit tests of `held_reach` and `portal_fog` (close twin unclamped; far twin ends
   where the twin's square ends; a twin near the edge of the player's square; the edge behind
   the twin left out; a view along an edge; water and a short render distance unchanged).
2. **F1, the system.** `place_portal_cameras` reads the player's chunk and render distance and
   passes them, with each camera's forward and field of view.
   → verify: the app tests of the fog in `portal/mod.rs` still describe it.
3. **F5.** Find why the holes through a far portal can show the planet's ground, and make
   every ray under the horizon that meets no chunk meet the horizon skirt, cheaply.
   → verify: the far-twin seam test's pictures, before and after.
4. **Regression tests.** `seam_close_{off,low,medium}_noon` assert that the 32–64 and 64–96
   classes stay under a threshold set from the measures, ignored like the other seam tests.
   → verify: run before (they fail) and after (they pass); numbers in the report.
5. **Cost.** `bench_portal_frame_time` before and after, on the GPU if there is one.
6. **Docs.** `docs/ARCHITECTURE.md` and the spec's as-built notes on the portal fog.
7. **Checks.** `cargo clippy --all-targets -q -- -D warnings`, `cargo fmt --all -- --check`,
   `cargo test --workspace --no-fail-fast`; screenshots to `.superpowers/sdd/portals/lot-9-screens/`.
