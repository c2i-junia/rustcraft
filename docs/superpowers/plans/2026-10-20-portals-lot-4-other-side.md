# Portals Lot 4 (The other side) Implementation Plan

**Goal:** What an open portal shows is there even when its twin is far away: each client is
also sent the chunks around the twin of every open portal within its render distance, keeps
them while that stays true, and the server loads them; the portal camera's fog ends at
`PORTAL_VIEW_RADIUS` chunks behind the twin; the sun and the moon are where they should be in
the view through a portal.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Architecture" (the "Chunks
behind portals" bullet of `server`, the fog of `camera.rs`) and "Lots", lot 4. Out of scope:
recursion, the body through portals, sounds (lot 5).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: a failing test first, then the code. `cargo test` (workspace), `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean at the end.
- No network or save change: `PROTOCOL_ID` stays 19, no new message.

## Decisions (where the spec is silent, or departs from it)

1. **One rule, in `shared`** (`world/reach.rs`): `ChunkReach`, built from the chunk of a
   player, its render distance and the portals, says which chunks its client is sent and
   which it keeps. Sent: the chunks within the render distance, then, for each open portal
   whose chunk is within the render distance, the chunks within `PORTAL_VIEW_RADIUS` (4) of
   its twin's chunk (`within_render_distance`, so 9 × 9 × 9). Kept, with a margin `m`: the
   chunks within the render distance plus `m`, and those within `PORTAL_VIEW_RADIUS + m` of the
   twin of each open portal within the render distance plus `m`, so that a portal at the edge
   of the render distance does not send and unload its twin's chunks as the player walks to and
   fro. The chunk of a portal is the chunk of the cells it opens onto (`Portal::chunk`): what
   is seen through a portal is in front of its twin.
2. **The client never unloads on its own** (already so: only `ChunkUnloads` removes a chunk),
   so the server's rule is the client's: the client keeps a chunk exactly while the server
   does not tell it to forget it. The server's broadcast unloads with `UNLOAD_MARGIN`, its own
   memory (`world/unload.rs`) with `KEPT_MARGIN`, both through `ChunkReach::keeps`.
3. **Order and budget.** Per broadcast, the chunks around the twins may take up to a quarter
   of `MAX_CHUNKS_PER_BROADCAST` (counted as now, varied chunks only); the player's own chunks
   take the rest. So neither starves: a player loading a render distance of 64 still gets the
   view through a portal within a tick or two, and the view never takes more than a quarter.
   On the wire, the player's own chunks come first, nearest to it first, then the others,
   nearest to a twin first. The chunks around the twins (≤ 729 each) are looked at on every
   broadcast; only the player's own keep the `SentChunks::progress` shortcut, whose key becomes
   the whole `ChunkReach`, so a portal that opens, closes or moves recomputes the unloads.
4. **Loading.** `active_chunks` (the order the server loads in) yields the players' own chunks
   as before, then the chunks around the twins of each player's portals, those within a
   player's own reach and those already yielded left out. `LoadedAround` is keyed by the
   players' `ChunkReach`es. The chunks next to the player loaded at once (radius 1) are
   unchanged.
5. **The fog of a portal camera.** Fog is measured from the camera; a portal camera stands as
   far behind the twin as the player's camera is in front of the portal (`d`). Its fog ends at
   `d + PORTAL_VIEW_RADIUS × 16`, where the chunks sent around the twin end along x and z, and
   starts at `d + FOG_START × PORTAL_VIEW_RADIUS × 16`; neither farther than the player's own
   fog, so that the player never sees farther through a portal than around itself, and under
   water the near fog of the water (2 to 16 blocks), which lies between the eye and the
   portal, is kept as is. The colour is the player's. Set on every frame the camera draws
   (`d` changes as the player moves), which replaces lot 1's copy on change.
6. **The sun and the moon** (lot 1 review #6). They are drawn at `CELESTIAL_DISTANCE` (50) from
   the local player, so a portal camera far away sees them from the wrong place. They become
   what they stand for, directions: drawn 10⁶ blocks away, 2·10⁵ wide (the same angular size),
   not fogged (fog would hide them at that distance; the edge fog of the render distance hid
   them too at short render distances, which is how they were never seen under 4 chunks), and
   not frustum-culled (beyond the far plane Bevy culls with). From a portal camera 10 000
   blocks away from the player, they are off by 0.6° (they are 11° wide). Under water the
   player's camera does not see them, as the water fog hid them before: the quads are hidden
   then (`CelestialBody`). The lights are untouched (a directional light has no position).
7. **e2e.** In solo, render distance 3: the player, in creative and flying high, builds a wall
   north of it (mod `/far`), shoots the orange portal on it, has a pillar of sand built in front
   of it, flies 160 blocks (10 chunks) south, has a wall built there and shoots the blue portal
   on it. Through the blue portal it sees the sand, 10 chunks away, which the server would have
   unloaded from its client without this lot. Screenshot to the report.

## Tasks

### Task 1: The rule (`shared/src/world/reach.rs`)
- [ ] Tests: `the_chunks_around_the_twin_of_an_open_portal_within_reach_are_sent_after_the_own`,
  `a_lone_portal_or_one_out_of_reach_sends_nothing_more`,
  `the_chunks_around_twins_come_once_and_not_those_of_the_player`,
  `the_chunks_around_a_twin_are_kept_with_the_margin_while_its_portal_is_within_reach_plus_it`.
  See them fail.
- [ ] `PORTAL_VIEW_RADIUS`, `Portal::chunk`, `ChunkReach`.

### Task 2: Sending and unloading (`server/src/world/broadcast_world.rs`)
- [ ] Tests: `a_far_twins_chunks_are_sent_once_its_portal_is_within_reach_and_unloaded_after`,
  `the_chunks_around_a_twin_take_at_most_a_quarter_of_a_broadcast_and_come_after_the_own`,
  `a_portal_that_closes_unloads_the_chunks_around_its_twin`, a system test through
  `broadcast_world_state`. See them fail.
- [ ] `world_messages_around` takes a `ChunkReach`; the budget split; the order on the wire.

### Task 3: Loading and forgetting (`simulation.rs`, `unload.rs`)
- [ ] Tests: `the_chunks_around_a_twin_are_loaded_after_those_of_the_players`,
  `the_server_keeps_the_chunks_around_the_twin_of_a_portal_within_reach`. See them fail.
- [ ] `active_chunks` with the portals; `LoadedAround` keyed by reaches; `unload_far_chunks`.

### Task 4: The fog of a portal camera (`client/src/portal/camera.rs`)
- [ ] Tests: `a_portal_cameras_fog_ends_the_view_radius_behind_the_twin`,
  `under_water_or_with_a_short_render_distance_a_portal_cameras_fog_is_the_players`. See them
  fail.
- [ ] `portal_fog`, set by `place_portal_cameras`.

### Task 5: The sun and the moon
- [ ] Tests: `the_sun_is_seen_in_the_same_direction_from_far_away`,
  `under_water_the_sun_and_the_moon_are_hidden`. See them fail.
- [ ] `CELESTIAL_DISTANCE`, `CELESTIAL_SIZE`, `fog_enabled: false`, `NoFrustumCulling`,
  `CelestialBody`.

### Task 6: A far place (e2e)
- [ ] `e2e::portals::the_view_through_a_portal_reaches_a_far_place`; red before Task 2 lands
  (checked by running it with the server's portal chunks disabled), green after. Look at the
  images.

### Task 7: Checks, docs, report
- [ ] `cargo test`, clippy, fmt; `docs/ARCHITECTURE.md`; report in
  `.superpowers/sdd/portals/lot-4-report.md`, screens in `lot-4-screens/`.
