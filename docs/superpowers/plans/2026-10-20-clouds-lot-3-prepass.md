# Clouds Lot 3 (Depth prepass and inside view) Implementation Plan

**Goal:** One opacity for the clouds wherever they are seen from: below, above, between them
and inside one. Blended faces behind one another add up, so a cloud seen through its own top,
sides and bottom looked thicker, and the rows of its faces showed through its top.

**Spec:** `docs/superpowers/specs/2026-10-20-clouds-design.md`, the `material.rs` paragraph of
"Architecture" and lot 3.

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- `cargo test -p client`, `cargo clippy -p client --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at the commit.

## How Bevy 0.19 draws them

- The camera has no prepass (`DepthPrepass` is not used in the game), MSAA is the default.
- Depth is reversed: every mesh pipeline compares with `GreaterEqual`, the transparent ones
  included (`bevy_pbr/src/render/mesh.rs`), and the transparent ones do not write depth.
- `Opaque3d` is binned, in no set order; `Transparent3d` is sorted back to front by the view
  depth of the centre of the mesh's bounding box plus the material's `depth_bias`, which
  orders and does nothing else for a custom `Material`.

## Decisions

1. **Two drawings of the same mesh, both among the transparent meshes.** A child of the
   clouds' entity has the same mesh (so the same drift) and a `CloudMaterial` with
   `depth_only`: its pipeline writes no colour (`write_mask` empty, no blending) and writes
   depth. Its `depth_bias` of −1 sorts it just before the colour. The colour then passes the
   default `GreaterEqual` only where its depth is the one written: the nearest face.
2. **Not in the opaque phase**, as the spec first said: a depth-only mesh among the opaque
   ones, drawn in no set order with the terrain, hides the terrain behind the clouds (it
   fails the depth test, or is drawn first and covered). Tried: from above, the ground under
   a cloud is gone and the cloud is blended over the clear colour
   (`clouds_over_the_ground`). Among the transparent meshes, the depth is written after the
   terrain, which shows through.
3. **The same depth for both drawings**: the same shader, the same mesh and transform, the
   position `@invariant`. The material key (`CloudKey`) only changes the pipeline state.
4. **Faded out, no depth**: both drawings discard where the alpha is under 0.01, so the far
   edge of the clouds hides nothing behind it.
5. **The water**: the clouds are sorted by the centre of their window, around the camera at
   the height of the clouds. From below, that point is above the camera, nearer along the
   view than the water ahead: the clouds are drawn after the water, which they do not cover
   on screen. From above, it is nearer than the water below: the water is drawn first and
   seen through the clouds. Water drawn after the clouds' depth and behind them would be
   hidden by it; with this order, that takes water above the camera and behind a cloud.

## Tasks

### Task 1: Depth-only drawing
- [x] Test: `the_depth_of_the_clouds_is_drawn_just_before_their_colour` (a child with the
  same mesh, after a rebuild too, the same uniforms, a smaller depth bias, no shadows). See
  it fail.
- [x] `CloudMaterial::depth_only`, `CloudKey`, `specialize`, `depth_bias`; the child spawned
  with the clouds; the rebuild gives both the mesh; the uniforms go to every cloud material.
  See it pass.
- [x] `clouds.wgsl`: `@invariant` position, discard under 0.01.

### Task 2: A look at them
- [x] `e2e::clouds::snapshot_the_clouds`: also in a cloud (the nearest full cell with an
  empty one west of it), over its west edge 12 blocks above its top looking down, two days
  later, at sunset and at midnight. Seen: one opacity from below, above and inside, no rows
  through the tops, no z-fighting, the ground through the clouds from above, the drift, the
  fade, the warm and the grey blue tints.

### Task 3: Checks and commit
- [x] `cargo test -p client`, clippy, fmt; commit `feat(client): clouds keep one opacity seen
  from anywhere`.
