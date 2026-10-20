# Portals Lot 1 (Model and rendering) Implementation Plan

**Goal:** The portals of the world as data shared by both sides, and the client drawing them:
an open portal shows, in real time, the view from its twin; a lone one shows its colour. No
placement yet: tests insert portals into the client's map.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Architecture" (`shared`,
`client`) and "Lots", lot 1. Reference technique: the three.js demo of the approved proposal
(virtual camera `through · camera`, oblique near plane, screen-space sampling, hollow box drawn
by its inner faces, glowing rim, visibility test).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- No network change: `Portal` gets no serde yet, `PROTOCOL_ID` stays 18.
- TDD where the code runs without a GPU: the model, the transform, the projection, the
  spawning of the surfaces and cameras, the visibility, the meshing of the wall.
- `cargo test` (workspace), `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean at each commit.

## Decisions (where the spec is silent, or departs from it)

1. **The frame of a portal.** `n` is the outward normal of `face`, `r = Y × n` the right of a
   viewer facing the wall, the anchor the lower-left wall cell, so the wall cells are
   `anchor + i·r + j·Y`, `i < 2`, `j < 3`. The frame (`Portal::frame`) has its origin at the
   centre of the opening on the wall's face and its axes `(r, Y, n)`; the plane is
   `n · p = n · origin`.
2. **No custom `CameraProjection`.** Bevy 0.19's `PerspectiveProjection` already has an oblique
   near plane, `near_clip_plane`, applied by Lengyel's method rewritten for its infinite
   reverse-Z matrix: the third row becomes `M₄ − a·C`, which is the spec's `z_clip = w_clip −
   s · (plane · p)`. The portal camera keeps `Projection::Perspective`, and `projection.rs`
   only computes that plane from the twin (`portal_projection`), or the plain projection
   within 0.12 block of it. Bevy skips the adjustment when the plane's normal is exactly
   `(0, 0, −1)`, a portal seen straight on; `near` is then set to the distance to the plane,
   which is the same plane (with the oblique matrix `near` does not count).
3. **The wall's face under a portal is not meshed.** The box lies behind the plane, in the six
   wall cells, which are solid blocks: their front faces, on the plane, would hide it. The
   mesher skips the faces that a portal covers (`Portals::covers`), and the client meshes the
   chunks of the wall cells again when the portals change.
4. **Layers.** The main camera sees layers 0 and 1, the portal cameras 0 and 2. A portal's
   live surface (sampling its camera's image) is on layer 1; its stand-in, the same box in its
   colour, on layer 2. So no portal camera samples an image (its own or the other's): this is
   recursion 1, the deepest level showing the colour. The rim is on layer 0.
5. **One material, three looks.** `PortalMaterial` (`portal.wgsl`) is unlit and unfogged, with
   Bevy's mesh vertex shader: `VIEW` samples the image at `(position.xy − viewport.xy) /
   viewport.zw`, `COLOR` shows the portal's colour (darker towards the bottom), `RIM` fades
   from white at the opening to the colour, blended. The box is drawn by its inner faces (front
   faces culled).
6. **Colour pipeline.** The portal camera has `Tonemapping::None` and renders into an
   `Rgba8UnormSrgb` image: the image holds the scene in linear light, before tonemapping, and
   the main camera, which tonemaps everything it draws, tonemaps it once. The portal camera
   copies the main camera's fog and clear colour (its own fog comes with lot 4).
7. **The camera of a portal** exists while the portal is open, is placed in `PostUpdate`
   before transform propagation (`through(portal) · camera`), and is active only when the
   player's camera is in front of the portal's plane and its frustum meets the box. Its image
   is the size of the window; on a resize a new image replaces it in the camera and the
   material, as Bevy's `mirror` example does.
8. **Other cameras.** Systems that took the one `With<Camera>` or `With<Camera3d>` as the
   player's now filter on `CameraController`. In the e2e harness, only cameras that render to
   the window become the UI camera or are switched to the screenshot image.
9. **Test hook.** `ClientWorldMap::blocks.portals_mut()` is the way in: an e2e test builds two
   walls in the client's map (the server does not know them: they are décor), inserts a blue
   and an orange portal, and aims at the blue one.

## Tasks

### Task 1: The model (`shared/src/world/portal.rs`)
- [ ] Tests: `the_wall_cells_go_right_and_up_from_the_anchor`,
  `the_frame_is_at_the_centre_of_the_opening_facing_out`,
  `the_twin_is_the_other_colour_of_the_same_owner`, `a_lone_portal_has_no_twin`,
  `through_takes_the_front_of_a_portal_to_the_back_of_its_twin`,
  `through_turns_a_direction_into_the_portal_into_one_out_of_the_twin`,
  `through_and_back_is_the_identity`, `placing_a_colour_again_moves_it`,
  `a_portal_covers_the_front_faces_of_its_wall_cells_only`. See them fail.
- [ ] `PortalColor`, `Portal { owner, color, anchor, face }`, `Portal::{normal, right, cells,
  frame, plane_distance}`, `Portals::{insert, remove, get, iter, twin, through, covers,
  is_empty}`.
- [ ] `ChunkMap::{portals, portals_mut}`. Commit `feat(shared): ...`.

### Task 2: The wall under a portal is not meshed
- [ ] Test (meshing): `the_face_a_portal_covers_is_not_meshed`. See it fail.
- [ ] `chunk_faces` skips the covered faces; `spawn_meshing_tasks` copies the portals into the
  map it hands the tasks.

### Task 3: Projection (`client/src/portal/projection.rs`)
- [ ] Tests: `a_point_behind_the_twins_plane_is_clipped`, `a_point_in_front_of_it_is_not`,
  `depth_stays_reversed_beyond_the_plane`, `a_portal_seen_straight_on_is_clipped_too`,
  `within_0_12_block_of_the_plane_the_projection_is_plain`. See them fail;
  `portal_projection`; pass.

### Task 4: Material and shader
- [ ] `PortalMaterial { params: PortalParams { color, look }, image }`, `portal.wgsl`, loaded
  with `load_internal_asset!`; front faces culled for the box, none for the rim; `RIM` blended.

### Task 5: Surfaces, rim, cameras
- [ ] Tests (App with `MinimalPlugins`, `StatesPlugin`, `AssetPlugin`, the assets):
  `an_open_pair_gets_two_surfaces_and_two_cameras`,
  `a_lone_portal_shows_its_colour_and_has_no_camera`,
  `removing_the_twin_closes_the_portal`, `the_portal_camera_follows_through_the_portal`,
  `a_portal_off_screen_or_seen_from_behind_has_its_camera_inactive`,
  `the_wall_cells_are_meshed_again_when_the_portals_change`. See them fail.
- [ ] `portal_plugin`: `sync_portals` (spawn/despawn per portal and camera, remesh requests),
  `place_portal_cameras` (transform, projection, activity, fog, clear colour) in `PostUpdate`
  before `TransformSystems::Propagate`, `resize_portal_images`. Main camera on layers 0 and 1.
- [ ] Camera queries filtered on `CameraController`; harness changes of decision 8.

### Task 6: Seen through
- [ ] `e2e/portals.rs`: `the_view_through_a_portal_is_the_other_side` — two walls, a pair,
  a marker pillar in front of the orange portal and one behind its wall; asserts the marker in
  front shows inside the blue portal on screen and the one behind does not; review snapshot.
  Look at the image.
- [ ] `bench_portal_frame_time` (ignored): frame time with the portal in view and turned away,
  lavapipe and `BENCH_GPU`.

### Task 7: Checks, docs, report
- [ ] `cargo test`, clippy, fmt; `docs/ARCHITECTURE.md` (shared, client frame, a portals
  paragraph); report in `.superpowers/sdd/portals/lot-1-report.md`.
