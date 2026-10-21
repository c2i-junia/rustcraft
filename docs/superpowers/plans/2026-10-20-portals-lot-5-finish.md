# Portals Lot 5 (Finish) Implementation Plan

**Goal:** A portal seen through a portal shows its view, down to three levels, the last showing
the portal's colour; the local player sees its own body through the portals; a portal that opens
and a player that goes through one are heard. Plus the findings parked for this lot: the cost of
the `NoCpuCulling` toggle measured and the re-activation of a portal camera asserted, the extra
footstep of a crossing, the hairline of a portal seen edge-on.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Architecture" (`client`, the
material bullet: "or shows the portal's colour when closed or at the last level of recursion")
and "Lots", lot 5.

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: a failing test first, then the code. `cargo test` (workspace), `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean at the end.
- No network or save change: `PROTOCOL_ID` stays 22. No new audio file.

## Decisions

1. **Which views recurse.** The camera of level `l + 1` of a portal P stands where the camera
   of level `l` of P would be through P: `through(P) · camera(P, l)`, the player's camera being
   level 0. So P recurses into itself: what is seen through P shows P again when P lies in front
   of its twin (two portals facing each other, or at an angle where P is seen out of its twin).
   That is the only way a portal can be seen through itself with a pair, and it keeps one
   camera per portal and level: 3 per open portal, bounded. Any other portal seen through a
   portal (another player's, say) shows its colour, as at recursion 1.
2. **Cameras of the deeper levels draw only when needed.** Level `l + 1` of P draws when level
   `l` draws, sees the front of P (frustum and plane, as `sees`), P is not wholly behind the
   near plane of level `l` (the twin's plane) and not beyond level `l`'s fog. Its fog is
   `portal_fog(player's fog, distance from camera(P, l) to P)`, which for level 1 is lot 4's.
3. **Order and images.** Level `l` draws with `order = -l`: the deepest first, the player's
   camera last. Two images per portal: level `l` draws into image `l % 2`. Level 3 and level 1
   share image 1: level 2 reads it between them, and draws into image 0, which level 1 reads.
   No camera reads the image it draws.
4. **Layers.** The surfaces of a portal are now one per level: level 1 on `SURFACE_LAYER` (the
   player's camera); level `k + 1` (reading the image of level `k + 1`) on a layer of its own,
   seen only by P's camera of level `k`. P's colour stand-in goes on a layer of its own too,
   seen by every portal camera but P's of the levels that see P's view instead. Each drawn
   portal takes a free slot, and its layers are `FIRST_PORTAL_LAYER + 3 · slot + {0, 1, 2}`
   (stand-in, surface seen by level 1, surface seen by level 2). The camera layers are computed
   by a pure function and set on every frame (`set_if_neq`), so they follow portals that come
   and go.
5. **Depth.** `PORTAL_RECURSION = 3`, the spec's; a resource `PortalRecursion` (default 3) lets
   the benchmark and the tests compare with 1. At the deepest level allowed, a camera sees its
   own portal's stand-in.
6. **The body.** The local player's body goes on `BODY_LAYER` (4) in first person, which the
   portal cameras draw and the player's camera does not; on layer 0 in third person. Its colour
   stays `LOCAL_PLAYER_COLOR` (`HIDDEN_PLAYER_COLOR` goes). The sky light gets `BODY_LAYER` too,
   so that the body keeps its shadow in first person (lights only shadow the layers they share).
7. **Sounds.** Two `Sound`s from files already there: `PortalOpen` plays
   `sounds/place/glass.ogg` (a knock and a glassy ring) at the centre of a portal drawn open
   (opened, moved, or placed with its twin there), `PortalPass` plays `sounds/player/pickup.ogg`
   (a short rising pop) when the local player goes through a portal. `LocalPlayerMoved` says
   whether the frame went through a portal; that frame then plays the passing sound instead of
   a step and does not count toward the stride (the extra footstep of lot 3).
8. **Hairline.** Recheck lot 3's glance shot; if the line is the rim seen edge-on, fade the rim
   as it turns edge-on (shader, the cosine between the view ray and the plane).
9. **Measurements.** The e2e benchmark gains a facing-portals case: recursion 3 against
   recursion 1, and the frame on which a portal camera turns on (the `NoCpuCulling` toggle)
   against the frames in view, on the GPU (`BENCH_GPU=1`) and lavapipe.

## Tasks

### Task 1: Recursion (`client/src/portal/`)
- [ ] Tests (App, no GPU): facing portals give three active cameras for the portal looked at,
  with orders -1, -2, -3, each the previous one through the portal; images `l % 2`; with
  `PortalRecursion(1)` only level 1 draws; a portal not in front of its twin draws level 1
  only; camera layers (pure function): own surface of the next level, every stand-in but its
  own, its own stand-in at the last level; the image follows the window for every level.
- [ ] Code: `PortalCamera { level }`, `DrawnPortal` with slot, cameras, images and the view
  materials per level; `place_portal_cameras` walks the levels; `resize_portal_images` per
  drawn portal.

### Task 2: e2e of recursion and the body (`client/src/e2e/portals.rs`)
- [ ] Facing portals 16 blocks apart, the player between them looking at the blue one: the
  wall around the third blue opening, seen in the second, is cobblestone, not blue; the third
  shows blue; the player's body, seen through the blue portal, is red. Screenshots.
- [ ] `the_view_through_a_portal_is_the_other_side`: look away and back, the sand is seen
  again (the view restarted with its chunks).

### Task 3: The body (`client/src/player/`)
- [ ] Tests: in first person the body is on `BODY_LAYER` only, in third person on layer 0; its
  colour is `LOCAL_PLAYER_COLOR` in both; the portal cameras draw `BODY_LAYER`, the player's
  camera does not; the sky light shadows it.

### Task 4: Sounds (`client/src/audio/`, `client/src/portal/mod.rs`)
- [ ] Tests: a crossing frame plays `PortalPass` and no step, and the stride is not advanced by
  the jump; a portal drawn open writes `PortalOpened` at its centre, a portal drawn closed or
  left unchanged does not; the files exist.

### Task 5: Hairline, measurements, docs
- [ ] Hairline: recheck, fix if cheap.
- [ ] Benchmark: recursion 3 vs 1, the hitch of a view starting. Numbers to the report.
- [ ] `docs/ARCHITECTURE.md`, README if it lists the feature.
