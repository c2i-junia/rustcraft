# Portals Lot 8 (The shot) Implementation Plan

**Goal:** The portal gun's shot is seen: a short glowing bolt of the portal's colour flies from
the shooter's eye to the hit point at 100 blocks a second, leaving a fading trail, and bursts
into a few sparks where it lands; the portal opens when it lands. Every player sees every shot:
the shooter predicts its own, the server tells the others. A refused shot flies too and bursts
without a portal.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Extension: floors and ceilings,
every entity, the shot", the "The shot" row and lot 8.

**Done when:** tests of the message and of the bolt's flight time, and a screenshot mid-flight.

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: a failing test first, then the code. `cargo test --workspace`, `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean at the end.
- `PROTOCOL_ID` stays 25: the branch already bumped it; `PortalShot` is appended to
  `ServerToClientMessage` and its index pinned.
- Only the visuals wait for the bolt: the simulation and the server have the portal from the
  tick it was placed, and a player may cross it before it is drawn.
- Negligible cost: a few entities per shot, gone after the burst; nothing done per frame when
  no shot is in flight.

## Decisions

1. **What a shot is** (`shared/src/players/blocks.rs`). `PortalShot { from, to, color, placed }`:
   `from` the origin of the aim ray (the camera in first person, the player in third), `to` where
   the ray meets the aimed face (the hit point `fit_portal` already uses), or, when nothing is
   aimed within the ray, the point `PORTAL_GUN_REACH` along it; `placed` whether a portal was
   put. Every shot of the gun is one, refused or not; `BlockInteractions` and `ActionsOutcome`
   gain `shots`. The aim ray stops at the wall behind a portal as before: shots never go
   through portals.
2. **The message.** `ServerToClientMessage::PortalShot { shooter, from, to, color, placed }`,
   appended (index 12), on the standard channel. `shooter` is added to the fields the spec
   names: a client needs it to know whose portal waits for the bolt.
3. **The server** keeps the shots of the tick in `ServerWorldMap::portal_shots`, filled by
   `handle_player_inputs_system`. `broadcast_portals_system` sends each to every client with a
   player in the world but the shooter, before the `PortalUpdates` of the same tick: on the one
   ordered channel, a client always learns of the bolt no later than of its portal.
4. **The client's shots** (`client/src/portal/shot.rs`). A `ShotFired` message, written by the
   prediction (`handle_block_interactions`, from the shots of the simulation, its `from` moved
   `SHOOTER_OFFSET` blocks along the ray so that the bolt does not fill the screen on its first
   frame) and by the network (`PortalShot`). In `PostUpdate`, before `sync_portals`:
   - `spawn_bolts`: per shot, a bolt (an elongated capsule, the portal's colour toward white)
     and its trail (a tapered streak whose alpha fades from the bolt to its tail), unlit,
     additive, brighter than white so that the bloom of the shaders catches them, saturated and
     still bright without them. Meshes and materials made once.
   - `fly_bolts`: a bolt is at `from.lerp(to, age / flight)`, `flight = distance / BOLT_SPEED`;
     the trail runs from where the bolt was `TRAIL_TIME` earlier to the bolt. When it lands the
     bolt is hidden and bursts into `SPARKS` sparks thrown back toward the shooter (the fox's
     dust particles, `mob::fox::spawn_particle`, made `pub(crate)`); the trail shrinks into the
     hit point over `TRAIL_TIME`, then both are despawned.
   - `hold_shot_portals`: while a placed shot's bolt flies, `ShownPortals` holds the client
     map's portals with that shooter's portal of that colour as it is drawn (or absent if none
     is). `None` when no such bolt flies, the map's portals then shown as they are: no work and
     no copy without shots.
5. **What waits.** `sync_portals` draws `ShownPortals` (so the new portal, its opening sound,
   its twin's opening and the remeshing of its wall come when the bolt lands); the portal
   cameras (`place_portal_cameras`) and the meshing of the chunks (which leaves the faces of the
   portals out) read it too, so a wall never shows a hole before the portal is drawn. On other
   clients the shot arrives before or with the `PortalUpdates`, so the same hold applies; a
   portal whose bolt is unknown shows at once. A refused shot holds nothing.
6. **Sound.** None added: the portal's opening sound, already played when it is drawn open,
   now comes when the bolt lands.

## Tasks

- [x] 1. Shared: `PortalShot` in `BlockInteractions` / `ActionsOutcome`. Tests: a placed shot
  from the eye to the face; a refused one (no room) still hits the face; a shot at nothing
  goes the reach along the ray.
- [x] 2. The message, appended and pinned (index 12), and a serde round trip.
- [x] 3. Server: `portal_shots`, sent to every player but the shooter before the portals.
  Session test: bob is told alice's shot before the portal, alice is not; a refused shot
  reaches bob unplaced.
- [x] 4. Client: flight maths (time at 32 blocks is 0.32 s; position; trail), `ShotFired`,
  bolts, sparks, `ShownPortals`; `sync_portals`, cameras and meshing on it. Tests: a placed
  shot's portal is drawn and heard only once its bolt lands, a refused shot holds nothing and
  its bolt is gone after the burst, a portal with no bolt shows at once, a bolt's entities
  despawn.
- [x] 5. Wiring: prediction and network write `ShotFired`.
- [x] 6. e2e: the gun fired at a wall, a screenshot mid-flight (the portal not drawn yet) and
  one of the sparks, then the portal drawn.
- [x] 7. Docs: `docs/ARCHITECTURE.md`, the README's portal line.
