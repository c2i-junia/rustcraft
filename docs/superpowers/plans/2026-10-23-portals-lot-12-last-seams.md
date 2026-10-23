# Portals, lot 12: the last seams of a crossing

Requirements and evidence: `.superpowers/sdd/portals/seam-diagnosis.md` (F6, "not measured"),
`.superpowers/sdd/portals/lot-10-report.md` (concerns 1 and 3, m5), `.superpowers/sdd/portals/rebase-5-report.md`
(open items 1–6), `.superpowers/sdd/portals/lot-11-report.md`. Branch `feat/portals`, from
`0b81b4d9`.

## Goal

What is left of the seam of a crossing, by visual impact: the colour the world fades into, the
shadows, the haze seen from outside its layer, the weather through a portal, and the cost of the
haze with recursion. Each item is measured first; fixed when it can be, gated when that is cheap;
otherwise shown not fixable, with the evidence.

## 1. F6: the colour of the horizon fog

- Measure: a new e2e, `horizon_colour`, from 30 blocks over the plains, compares the horizon
  skirt (drawn with `sky_radiance(horizon_direction)`, the colour every fogged edge fades into)
  with the sky at the height it is read at (0.03), at noon and dusk, at Medium, High and Ultra,
  with no portals, portals opened before the shaders, and after. Several runs each.
- Root cause: run red with the portal cameras given the sky as before lot 11 (Bevy's
  `prepare_probe_textures` takes the tables of the first camera with an atmosphere; with a
  portal camera first, the player's sky map is drawn from the portal camera's tables).
- Gate: skirt and sky within `HORIZON_APART`.
- Main: decide from the code which part of the fix main needs (no portal cameras there), and
  leave a patch for main that applies, builds and passes.

## 2. Shadow edges at Medium

- Measure with the portal camera's own eye as the picture after (no 0.1-block parallax), under
  each filter (as built, the portal camera's Gaussian as before, Gaussian, Hardware2x2 and
  temporal on every camera), in turn, three rounds, the first measure of a run left out.
- Fix what differs: the portal camera filters its shadows as the player's camera does
  (temporal with TAA); unit test first.

## 3. The haze of a far twin, seen from outside its layer

- Measure: the portal seen from 45° above the top of the layer of haze, and from within it,
  against the view straight from the portal camera's pose (`seam_close_high_*_from_above`).
- If the oblique near plane clips the layer's front faces, the portal camera's layer is
  stretched to hold it (Bevy then draws it from inside) and the patched shader marches only the
  slab of the real layer.

## 4. The weather through portals

- (a) An occlusion map around each twin of a drawn portal, bound to the portal cameras' pass.
- (b) Whether drops fall is decided per view: the player's eye, and for a portal camera the
  medium at the opening of its twin. Lightning likewise.
- (c) An e2e strike near the twin, held at its brightest stroke, seen through the portal and
  straight; a bolt struck near an approached twin is drawn.
- (d) Measure the cost of the drops in the portal views (GPU bench in rain); cap if significant.
- (e) TAA and drops through a moving portal view: measure the drops' contrast through the portal
  against straight while the eye moves.

## 5. The haze with recursion

- Measure `bench_portal_recursion_frame_time` at High and Ultra with the haze, and with fewer
  steps for deeper levels; cap if it is cheap and invisible.

## Checks

`cargo clippy --all-targets -q -- -D warnings && cargo fmt --all -- --check && cargo test
--workspace --no-fail-fast`; the seam gates two at a time; screens as JPEG in
`.superpowers/sdd/portals/lot-12-screens/`; ARCHITECTURE and the spec's as-built notes; report
`.superpowers/sdd/portals/lot-12-report.md`.
