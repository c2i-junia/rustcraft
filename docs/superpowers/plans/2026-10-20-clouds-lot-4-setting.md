# Clouds Lot 4 (The Fancy / Fast / Off setting) Implementation Plan

**Goal:** The player chooses how the clouds are drawn, Fancy, Fast or Off, in the Video tab of
the settings. The choice is saved in `video.ron` with the render distance and applies at once
in game.

**Spec:** `docs/superpowers/specs/2026-10-20-clouds-design.md`, the "Settings" paragraph and
lot 4; lot 1's `CloudStyle` and `cloud_mesh`.

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- `cargo test -p client`, `cargo clippy -p client --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at the commit.

## Decisions

1. **`CloudStyle` is a resource**, next to `RenderDistance`, both loaded from `video.ron` at
   start. `RenderDistance` keeps its API (`distance`, `set`) but is no longer what the file
   holds.
2. **`VideoSettings` is what `video.ron` holds**: `render_distance` and `clouds`, each with its
   default when missing. `load_video_settings` returns both resources, the distance brought
   within its bounds; `write_video_settings` writes both from the resources. Every writer
   (the render distance keys, its slider, the clouds row) passes both, so that writing one
   never loses the other. An old file, `(render_distance: 8)`, loads with Fancy clouds.
3. **The rebuild key holds the style**: the clouds are rebuilt when the camera changes cell,
   the reach changes, or the style changes. Fancy gives the mesh to the colour and the depth
   drawings; Fast gives the flat mesh to the colour alone (one layer of quads that never
   overlap: no depth needed); Off gives no mesh to either, so nothing is drawn.
4. **The Video tab row** is laid out as the rows of the Interface tab: a label as wide as the
   labels of the sliders, the segmented control, and a note under it in the muted small font,
   aligned with the control, as the note of the render distance: "Clouds made of blocks",
   "A flat layer of clouds", "No clouds".

## Tasks

### Task 1: `video.ron`
- [x] Tests: an old file without `clouds` loads as Fancy; the clouds and the render distance
  are saved and loaded back together; garbage and out-of-bounds distances as before.
- [x] `VideoSettings`, `load_video_settings`, `write_video_settings`; `CloudStyle: Resource`;
  `add_client` inserts both resources.

### Task 2: The clouds follow the style
- [x] Tests: Fast gives the colour a mesh and the depth none; Off gives neither; back to
  Fancy gives both again, at once.
- [x] `rebuild_clouds_system` reads `CloudStyle`.

### Task 3: The Video tab
- [x] Tests: the note of each style; e2e: the row is shown with Fancy checked, choosing Fast
  saves it and keeps the render distance; in game, choosing Off removes the mesh at once.
- [x] The row, its observer and its note; `menu_video.png` rewritten and looked at.
