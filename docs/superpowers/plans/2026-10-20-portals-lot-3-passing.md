# Portals Lot 3 (Passing through) Implementation Plan

**Goal:** A player walks into an open portal and comes out of its twin without a cut: the wall
lets it in inside the frame, its position, velocity and camera go through the portal, and the
client turns its camera when it predicts the crossing, never when it replays inputs.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Architecture" (the "Passing"
bullet of `shared`) and "Lots", lot 3. Out of scope: the chunks around far twins (lot 4),
recursion, sounds and the body (lot 5).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: a failing test first, then the code. `cargo test` (workspace), `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean at the end.
- No network change: `PROTOCOL_ID` stays 19, `MovementState` keeps its fields.

## Decisions (where the spec is silent, or departs from it)

1. **What the simulation reports.** `simulate_player_movement` returns a `MovementOutcome {
   fallen, turned }` instead of the fall alone; `turned` is the yaw, in radians about the
   vertical, the player was turned by going through a portal in this input. Portals are
   vertical, so `through` turns about the vertical only and keeps the pitch. The server needs
   nothing more: the shared simulation turns `player.camera_transform` itself, and the next
   input brings the client's turned camera anyway. `ActionsOutcome` keeps `fallen` only.
2. **Crossing** (`Portals::crossed`). Over one input, from where the centre of the player was
   before it to where it is after: the first open portal whose plane the segment goes through,
   from in front of it to behind it, at a point inside its frame (2 × 3). Then position and
   velocity go through `through(portal)`, the camera's translation and rotation too. A crossing
   from behind, outside the frame, or of a closed portal does nothing.
3. **"In front" has a slack** of 1 mm (`PORTAL_SLACK`): a point is in front of a portal when its
   signed distance is at least −1 mm, behind it beyond. A player that just came out of the twin
   is in front of it by what it was behind the first portal, more than the slack, so that no
   rounding of `through` ever leaves it behind the twin's plane, in the wall and colliding
   with it. The slack is less than the box's inset (2 mm), so the eye never gets behind the
   front face of the portal's box either.
4. **The wall that lets the player in** (`Portals::letting_through`). While the current centre
   of the player is in front of an open portal (decision 3), within its frame shrunk by half the
   player's width on each side (|x| ≤ 0.6, |y| ≤ 1.1 in the portal's frame), the six wall cells
   of that portal do not collide with it (`check_player_collision`, through a new
   `ChunkMap::check_collision_box_except`). It is the current centre that counts, not the
   candidate one, so that the move that takes the centre behind the plane is allowed, and is
   then the crossing. Mobs and dropped items still collide with the wall.
5. **The client's camera.** `player_movement_system` adds `turned` to the yaw of the camera
   (`CameraController::angle_x`), which `camera_control_system` turns the camera by in the same
   frame; the next frame's input carries the turned camera. `reconcile_prediction` replays with
   `simulate_player_movement` and ignores what it reports, so a replay never turns the camera:
   the live prediction has turned it already, once.
6. **The twin's stand-in.** A closed-colour box (the stand-ins, and the surface of a lone
   portal) is not drawn from behind the plane of its portal: `portal.wgsl` discards its
   fragments when the camera is behind that plane (the plane is a new uniform). A portal camera
   always stands behind its twin's plane, so it never sees the twin's stand-in, even within the
   0.12 block where it has the plain projection and would stand inside that box; a stand-in
   seen from in front of its portal, as through another portal, is drawn as before.
7. **The outline.** The aimed block is not outlined when the face aimed at is the face of an
   open portal (`Portals::opens`): the player sees the view through it, not the wall. A closed
   portal is a wall, outlined as before. Aiming through to the other side is not done.
8. **Footsteps.** A crossing makes the stride jump by the distance between the portals, which
   gives at most one step: left as is (lot 5 adds a sound when passing).
9. **e2e.** In solo, the server must know the portals: the player is saved in creative,
   flying, high in the air with the gun (`GameFolder::with_player`); a mod (`/course`) builds a
   stone platform under it with two walls of 2 × 3, one ahead facing it and one to its right
   facing west; the player lands, shoots both portals and walks into the blue one. Every frame,
   its position and yaw are recorded: the yaw turns once, by a quarter turn, and no step is
   longer than a frame's walk, the crossing step measured through the portal. A sequence of
   screenshots across the crossing, step by step, goes to the report for review, and the
   shots on either side of the crossing differ no more than two steps on one side do.

## Tasks

### Task 1: The geometry of passing (`shared/src/world/portal.rs`)
- [x] Tests: `a_point_in_front_of_the_frame_is_let_through_only_within_it_shrunk`,
  `a_closed_portal_lets_nothing_through`, `a_segment_into_the_frame_crosses_the_portal`,
  `a_segment_outside_the_frame_from_behind_or_into_a_closed_portal_crosses_nothing`,
  `an_open_portal_opens_its_face_only`. See them fail.
- [x] `PORTAL_SLACK`, `Portal::local`, `Portals::{letting_through, crossed, opens}`.

### Task 2: Collision and crossing in the simulation
- [x] `ChunkMap::check_collision_box_except`.
- [x] Tests (movement): `walking_into_an_open_portal_takes_the_player_out_of_its_twin`
  (position, camera turned, `turned` once), `the_wall_beside_the_opening_still_stops_the_player`,
  `a_flyer_crossing_the_plane_outside_the_frame_stays_on_its_side`,
  `a_portal_seen_from_behind_is_a_wall`, `a_closed_portal_is_a_wall`,
  `the_speed_goes_through_the_portal_turned_with_it`,
  `a_jump_through_a_portal_keeps_its_height`. See them fail.
- [x] `MovementOutcome`, the crossing at the end of `simulate_player_movement`, the exemption in
  `check_player_collision`. Callers updated (`simulation.rs`, footsteps test, client).

### Task 3: The client's camera
- [x] Tests (`client/src/player/update.rs`): `a_predicted_crossing_turns_the_camera`,
  `a_replay_crossing_a_portal_does_not_turn_the_camera`,
  `a_walk_through_a_portal_is_predicted_without_a_single_correction` (the `Session`, with a
  stand-in for the input set that copies the camera's yaw into the frame). See them fail.
- [x] `player_movement_system` turns `CameraController::angle_x`.

### Task 4: Rendering
- [x] Tests: `the_colours_and_the_rims_know_the_plane_of_their_portal` (portal/mod.rs),
  `the_face_of_an_open_portal_is_not_outlined` (interactions). See them fail.
- [x] `PortalParams::plane`, the discard in `portal.wgsl`; the outline condition.

### Task 5: Passing in solo (e2e)
- [x] `GameFolder::with_player`; `e2e::portals::walking_through_a_portal_is_seamless`, with the
  screenshot sequence. Look at the images.

### Task 6: Checks, docs, report
- [x] `cargo test`, clippy, fmt; `docs/ARCHITECTURE.md` (passing, in Network and in the client
  frame); report in `.superpowers/sdd/portals/lot-3-report.md`, screens in `lot-3-screens/`.
