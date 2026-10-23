# Portals, lot 13: the older open items

Requirements: the ledger (`.superpowers/sdd/portals/progress.md`, lots 6–8 and 12), the lot 6
review (concern 2: the eye inside a ceiling twin), the extension's final review (minor 3: two
floor portals trap a resting entity), the rebase 3 note (the gun's ray stopped by vines), the lot
12 re-review (N2: bolts drawn under water) and the lot 12 report (three flaky "unchanged view"
gates). Branch `feat/portals`, from `b2a486a4`.

Each item: a test that reproduces it first (red), the fix, the test green, a red control where
the test is a gate. One commit per item at least.

## 1. The eye inside the ceiling on a ceiling exit

- Root cause to confirm: the crossing is decided on the centre of the box, the eye is 0.8 above
  it. Out of a ceiling twin, the centre comes out just under the plane and the eye is 0.8 above
  it, inside the ceiling's cells, until the fall takes it below (`Portals::pass` keeps the box
  straddling the plane as it went in). The same happens before a crossing, rising into a
  ceiling portal: the eye goes into the ceiling before the centre crosses.
- Fix, render only (the simulation, the inputs, the aim keep the eye where it is): while the
  segment from the centre to the camera crosses an open portal the centre is in front of
  (`Portals::crossed`), the eye is on the other side of that portal, and the camera is drawn
  from there, `through(portal) · camera`: out of a ceiling twin, from above the floor portal
  it fell into, looking through it, which is the view it had a frame before. Applied after
  `camera_control_system`, undone at the start of the next frame, so that the input, the aim
  and the shot read the camera as the simulation knows it.
- Tests: shared, a player falling through a floor portal out of a ceiling twin and one jumping
  into a low ceiling portal, frame by frame: the raw eye is inside a solid block for several
  frames (the reproduction), the eye as drawn never is, and it moves no further in a frame than
  the player does (no jump at the crossing). Client: the camera system applies it and the next
  frame's input gets the camera back. E2E: the real camera, frame by frame, never in a solid
  block during a fall floor → ceiling.

## 2. A resting entity between two floor portals

- Today: an entity resting on floor portal A, whose twin B is a floor too, falls in, comes out
  of B too slowly for its box to leave B, falls back into B, and so on, every few ticks, for
  ever (players can walk away; mobs and items cannot).
- Rule, the same for players, mobs and items (shared `Portals`): an entity that comes out of a
  floor portal too slowly to rise half a block clear of it (`REST_CLEARANCE`) rests on it: that
  portal does not take it back (no crossing, its cells collide once the box is out of them)
  until the entity has left it (its centre out of the opening's front, or higher than the
  clearance above it, as by a jump). If it comes out too slowly for its box to leave the
  cells at all, it is given the speed to lift it just clear, so that it lands on the portal.
  A fall from higher (a real bounce) goes on as before. The entity carries the portal it rests
  on (`Player`/`MovementState`, `ServerMob`, `DroppedItem`, never saved).
- Tests: a sheep resting on A goes through once and stands on B; same for an item and a
  player; a player falling from high still bounces back into B; a player resting on B that
  steps off and back on, or jumps, falls in again.

## 3. The gun's ray stops on vines

- Fix: the portal gun's ray goes through every block that does not collide (`get_hitbox` is
  `None`: vines, tall grass, ferns, flowers, torches, water), as the placement rule's front
  does; it still stops on cocoa pods, leaves, glass, which the placement then refuses.
- Tests: the gun aims through a vine, tall grass, a flower and a torch at the wall behind and
  places its portal there; a cocoa pod still stops it; the hand's ray still hits a vine.

## 4. Bolts under water, stale bind groups

- Lightning: a view marked `UnderWaterView` draws no bolt, as no drop. Test: an e2e with the
  eye under the lake and a bolt held in front, counting the views the lightning pass binds
  (0 under water, 1 above).
- Both passes: when a binding is missing, the views lose their bind group instead of keeping
  last frame's (defensive; reachable only before the first view uniforms).

## 5. The flaky "unchanged view" gates

- Measure first: the pictures of each window (3 shots, 5 frames apart) and how the noise pair
  compares with the comparison, over several runs two at a time.
- Make the noise floor robust: every pair of shots of both windows, the largest per class,
  and/or a comparison corrected for a uniform drift; whichever the data supports.
- Red control: a deliberate change of the player's own view when a portal is approached
  (and when the shaders were switched) must still fail the gates.
- Goal: 20/20 seam gates in a full pass, two at a time.

## Checks

`cargo clippy --all-targets -q -- -D warnings && cargo fmt --all -- --check && cargo test
--workspace --no-fail-fast`; the seam gates `E2E_LIFETIME_SECS=3000 cargo test -p client
e2e::portal_seam -- --ignored --test-threads=2`. `docs/ARCHITECTURE.md` and the spec updated
where behaviour changes. Report in `.superpowers/sdd/portals/lot-13-report.md`.
