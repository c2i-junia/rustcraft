# Clouds Lot 1 (Field and mesh) Implementation Plan

**Goal:** The pure code of the clouds, unit-tested, with nothing on screen yet: the periodic
field of full cells, the drift of the clouds with the world clock, and the mesh of a window of
cells in the three styles.

**Spec:** `docs/superpowers/specs/2026-10-20-clouds-design.md`, "Decisions" and the `field.rs`
and `mesh.rs` paragraphs of "Architecture".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing before the code that makes it pass.
- `cargo test -p client clouds`, `cargo clippy -p client --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at every commit.
- No plugin is registered: `clouds/mod.rs` holds only the constants and the modules.

## Decisions (where the spec is silent)

1. **Value noise on a wrapped lattice** rather than SuperSimplex on a torus: random values on a
   lattice of `PERIOD / wavelength` points per axis, indexed modulo that count, smoothly
   interpolated. It tiles exactly by construction, needs no 4D noise and no new dependency
   (the client does not depend on `noise`). Lattice values come from `shared::world::splitmix64`
   of `(SEED, octave, x, z)`.
2. **Three octaves of wavelengths 8, 4 and 2 cells**, weights 1, 1/2, 1/4. A base wavelength
   of 16 cells was tried first: at 45 % coverage its blobs join into masses of hundreds of
   cells; 8 gives blobs of a few cells to a few tens. The threshold is a constant measured
   once over the whole period so that ~45 % of the cells are full.
3. **Drift in `f64`** before the modulo, so a tick count of years still gives an exact
   offset; the result is an `f32` in `[0, PERIOD * CELL)`.
4. **Quads before the mesh**: `quads(style, centre, radius, full)` returns axis-aligned
   rectangles with their outward normal; `cloud_mesh` assembles them. The predicate `full` is
   a parameter so the tests can place cells by hand; `cloud_mesh` passes `field::is_full`.
5. **The window** is the cells whose offset `d` from the centre has `d.x² + d.y² <= radius²`.
   A cell outside the window counts as empty: the edge of the window is closed.
6. **Side faces are merged too**: the ±Z faces along the cells of a run that all show them,
   the ±X faces are one per run end.
7. **The mesh** has positions, the shade attribute and `u32` indices, no normals (the
   material is unlit); `RenderAssetUsages::RENDER_WORLD` as the chunks. `Off`, or a window
   without a full cell, gives `None`.
8. **Fast** quads face up (shade 1.0) at `ALTITUDE + THICKNESS / 2`.

## Tasks

### Task 1: Module and constants
- [x] `client/src/world/clouds/{mod.rs, field.rs, mesh.rs}`, `pub mod clouds;` in
  `client/src/world/mod.rs`. Constants `CELL`, `THICKNESS`, `ALTITUDE`, `PERIOD`, `SPEED`,
  `COVERAGE`, `SEED`, the four shades.

### Task 2: Field
- [x] Tests: `the_field_is_the_same_on_every_call`, `the_field_repeats_every_period`,
  `about_the_target_share_of_cells_is_full`, `full_cells_come_in_blobs`. See them fail.
- [x] `is_full`, measure the threshold, see them pass.

### Task 3: Drift
- [x] Tests: `the_clouds_drift_half_a_block_a_second`,
  `the_drift_wraps_after_one_period`. See them fail; implement `drift`; see them pass.

### Task 4: Quads and mesh
- [x] `CloudStyle { Fancy, Fast, Off }` (Fancy default, serde, Reflect).
- [x] Tests: `an_isolated_cell_has_six_faces`, `two_neighbours_share_no_face`,
  `a_row_of_cells_has_one_top_and_one_bottom`, `fast_clouds_are_one_quad_per_run`,
  `off_draws_nothing`, `each_face_has_the_shade_of_its_orientation`,
  `every_face_is_counter_clockwise_seen_from_outside`, `only_the_cells_of_the_disk_are_drawn`,
  `the_mesh_has_a_shade_for_each_position`. See them fail.
- [x] `quads`, `cloud_mesh`; see them pass.

### Task 5: Checks and commit
- [x] `cargo test -p client clouds`, clippy, fmt; commit `feat(client): the field and the mesh
  of the clouds`.
