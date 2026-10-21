# Portals Lot 7 (Every entity) Implementation Plan

**Goal:** Mobs and dropped items go through open portals on their own, server-side simulation,
with the crossing rule players follow: position and velocity through the pair's transform, a
mob's body yaw turned with it. Clients show them jumping to the other side, never interpolated
across the gap.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Extension: floors and ceilings,
every entity, the shot", the "Every entity" row and lot 7. Out of scope: the shot (lot 8).

**Done when:** simulation tests for a dropped item falling through a floor portal and a mob
walking through a wall portal, and an e2e of a dropped item thrown through.

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: a failing test first, then the code. `cargo test --workspace`, `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean at the end.
- `PROTOCOL_ID` stays 25: fields are appended to structs already on the wire, no enum changes.
- Players behave exactly as before: the crossing is factored out of their code, their tests
  are unchanged.

## Decisions

1. **One crossing for every entity** (`shared/src/world/portal.rs`). What the player code does
   inline moves into `Portals`:
   - `passable_cells(center, margin)`: the wall cells of the open portals letting an entity of
     half-width `margin` through (`letting_through`), which do not collide.
   - `goes_into(from, to, margin)`: a move of the centre that goes into such a portal, made
     whatever lies behind its wall cells, and the crossing.
   - `pass(from, to, extents) -> Option<Passage { through, position }>`: the portal crossed,
     its transform, and where the centre comes out, clamped into the twin's opening shrunk by
     the box's half extents.
   The player's `check_player_collision`, `goes_into_portal` and `pass_through_portal` call
   them; the "way out of a wall" rule stays the player's.
2. **The wall exemption for entities** is the players' rule with each entity's own half-width:
   `DROPPED_ITEM_SIZE / 2` for an item, half the larger of a mob's width and depth (its box is
   axis-aligned whatever its yaw). Players shrink the opening by their half-width across and
   along the up alike; entities do the same, so the rule is one.
3. **Dropped items** (`step_dropped_item`): their collisions skip the passable cells of where
   the item is at the start of the step; a move along an axis that goes into a portal is made
   whole and ends the step's motion; then, if the centre crossed, the item passes: position,
   velocity, off the ground. Pickup, merge and lifetime are untouched: they work on the other
   side as anywhere.
4. **Mobs** (`server/src/mob/behavior.rs`): `collides_at` skips the passable cells of where the
   mob is; gravity, the walk and the flight make a move into a portal whatever lies behind.
   After gravity, and again after the action's move, a mob whose centre crossed passes:
   position, velocity, off the ground, its body yaw turned as a camera's would be
   (`Look::through`, the yaw of where it faces), and a walk's target position through the same
   transform, so it walks on the way it went rather than back. No pathfinding: a mob goes
   through when its wander takes it there.
5. **No interpolation across the jump.** `DroppedItem` and `ServerMob` gain `passes: u32`
   (appended, `#[serde(default)]` for the RON saves), counting the portals gone through,
   wrapping. `DroppedItemState` carries it (appended). A client that sees the count change puts
   the entity where the server says at once, rotation included for a mob, instead of easing it
   there. A counter, not a flag: an update missed or merged still changes it.
6. **The far side.** Items are sent by the chunks a client has, which include those around the
   far twins (lot 4): an item that crosses to a far twin is sent to the clients that see it
   there, removed from those that don't; the server keeps simulating it while those chunks are
   loaded, and holds it still otherwise, as any entity. Mobs are sent by distance: they are now
   also sent when they stand in the chunks a client sees through a portal (`ChunkReach::keeps`
   but not `within`), and forgotten when they leave both. Neither vanishes nor is duplicated:
   one id, one entity.

## Tasks

1. Shared crossing in `Portals` (`passable_cells`, `goes_into`, `pass`), unit tests; the player
   code calls them, its tests unchanged.
2. Dropped items cross: test of an item falling through a floor portal (out of a wall at the
   same speed, picked up there), of one resting on a floor portal that opens; the code.
3. Mobs cross: test of a sheep walking through a wall portal (out of the twin, yaw turned,
   target through, nothing stuck), of one falling through a floor portal; the code.
4. `passes` on the wire; client tests that an item and a mob whose count changed jump, and the
   code in `client/src/dropped_items` and `client/src/mob/sync.rs`.
5. Mobs within a client's portal reach are sent: test in `broadcast_world.rs`, the code.
6. e2e: a water bucket thrown by a player with a full inventory falls into a floor portal and
   comes out of a wall, seen on the client on the other side, without positions in between.
7. `docs/ARCHITECTURE.md`; full checks; report.
