# Portals Lot 6 (Floors and ceilings) Implementation Plan

**Goal:** Portals on every face of a solid full block: the four walls as before, and the floor
(`PlusY`) and the ceiling (`MinusY`), the opening lying along the shooter's facing. The
transform between two portals becomes a general rigid transform, players fall into a floor
portal and come out of a wall or a ceiling at the same speed, set upright, and everything drawn
of a portal works on any face. With the gun held, a right click on a container shoots.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Extension: floors and ceilings,
every entity, the shot", lot 6. Out of scope: mobs and dropped items through portals (lot 7),
the shot's bolt (lot 8).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: a failing test first, then the code. `cargo test` (workspace), `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean at the end.
- `PROTOCOL_ID` stays 25: `Portal` gains a field at its end (appended data), and
  `FaceDirection`, which it uses, already has its variant order pinned.
- Walls behave exactly as before: the existing tests stay green, but those asserting the old
  representation (the struct literals, the yaw a crossing reported).

## Decisions (where the spec is silent, or departs from it)

1. **The orientation.** `Portal` gains `up: FaceDirection`, the direction of its 3-block side
   in the world: `PlusY` on a wall, one of the four horizontal directions on a floor or a
   ceiling. Its frame is `(right, up, normal)` with `right = up × normal` (on a wall, as before,
   `Y × normal`); the wall cells go right and up from the anchor. One general field rather than
   an option used on horizontal faces only: every rule (cells, frame, fit, overlap, cover,
   crossing) reads it alike.
2. **The facing.** `opening_up(face, direction)`: `PlusY` on a wall; on a floor or a ceiling,
   the horizontal axis the shot goes most along, in its direction (away from the shooter, a wall
   tipped over forwards). `fit_portal` takes the shot's direction and keeps its six placements,
   distance to the hit point, fit and overlap rules, applied to that footprint.
3. **The transform.** `through` is unchanged in form, `M_twin · R_y(π) · M_portal⁻¹`: in the
   frames, `R_y(π)` is the half turn about the twin's up, so with general frames it is the
   general rigid transform of the spec.
4. **Players after passing.** Position and velocity through the transform; the camera's look
   direction too, then set upright: yaw from the new look (from its up when it looks straight
   up or down), pitch clamped to ±89° as the camera controller clamps it (`MAX_PITCH_DEGREES`
   moves to `shared`), no roll. The eye keeps its height above the centre. `MovementOutcome::
   turned` becomes `Option<Look { yaw, pitch }>`, the new look; the client sets its camera
   controller to it on live prediction only, the server keeps the camera the simulation set.
5. **The exit point.** Out of the twin, the centre is clamped into the twin's opening shrunk by
   the player's half extents along its right and up, so that a box falling out of a floor
   portal into a wall portal (1.8 high in a 3-high opening) fits it. Out of a wall into a wall it
   changes nothing: the box had to fit the opening to get in.
6. **Momentum out of a portal.** Players have no horizontal velocity today (walking is not
   velocity). For "coming out of a wall at the same speed", the horizontal velocity is now
   integrated while the player is in the air on foot, blocked per axis by blocks as the walk is,
   and dropped when it lands, flies or is in a liquid. Only a portal gives a horizontal
   velocity, so nothing changes for a player that never passes a floor or ceiling portal.
7. **Falling through a floor.** A vertical step whose centre goes into an open portal that lets
   the player through is made whatever the cells behind the wall cells hold (a floor thicker
   than one block would otherwise stop a fall a hair past the plane, killing its speed), and
   ends the vertical move of the input: the rest of the fall goes on out of the twin.
8. **The wall that lets the player in** is unchanged: the opening shrunk by half the player's
   width on each side, in the portal's frame (both horizontal on a floor), the controller's
   ruling. A player standing on an open floor portal, its centre over the shrunk opening, falls
   in. The way out of a wall works for a floor whose twin closes: the player jumps out.
9. **Rendering.** Everything is built in the portal's frame (box, rim, surface, culling,
   material plane) and follows the general frame. The oblique near plane: Bevy skips it only
   when the plane's normal in view space is exactly `(0, 0, -1)`; the existing workaround (`near`
   = the distance behind the plane) holds for any face, tested with floor and ceiling twins
   seen straight on. The mesher leaves out the top and bottom faces a portal covers.
10. **Containers.** With the gun held, `handle_block_interactions` no longer opens a crafting
    table, a chest or a furnace on a right click: the right click goes to the simulation, which
    shoots the orange portal. Sneaking is not needed.
11. **The bucket's ray.** Main's rays go 20 blocks; the branch lengthened the shared ray to 36
    for the gun. Only the gun's aim now goes 36 (`raycast::raycast_far`); the bucket's ray to the
    water sources and every other item's go 20 as on main, so a source beyond 20 blocks is no
    target (main: nothing aimed, nothing re-stated), and the reach stays 5.

## Tasks

1. `Portal::up`, general frame, cells, local; `opening_up`; `fit_portal` with the direction;
   tests of fit on floors and ceilings with each facing, of the transform (floor → wall,
   floor → ceiling, wall → floor), of letting through and crossing on a floor. Every literal
   gains `up: FaceDirection::PlusY`.
2. Gun: no more `HorizontalFace` refusal; a simulation test shooting a floor and a ceiling.
3. Movement: momentum, the fall through a floor, the exit clamp, the upright look; tests of
   falling through a floor portal and out of a wall at the same speed, floor → ceiling, standing
   on a floor portal, the way out of a floor whose twin closed, the look after passing.
4. Client: the controller sets the look; the container right click; the bucket's ray.
5. Rendering: the mesher's horizontal faces, the projection tests for floors and ceilings, a
   portal-system test of a floor portal drawn and seen.
6. e2e: falling floor → wall in solo, a screenshot through a floor portal.
7. ARCHITECTURE, README, report.
