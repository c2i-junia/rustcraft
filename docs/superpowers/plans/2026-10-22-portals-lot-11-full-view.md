# Portals, lot 11: the full view through an approached portal

Requirements and evidence: `.superpowers/sdd/portals/seam-diagnosis.md` (cause 4, F4),
`.superpowers/sdd/portals/lot-9-report.md` (`held_reach`), `.superpowers/sdd/portals/rebase-5-report.md`
(open items 3 and 5). Branch `feat/portals`, from `fb82384d`.

## Goal

A far twin is crossed as seamlessly as a close one. Portals are often a few chunks apart, and
through them the player saw only the 4 chunks the server sent around the twin, under a short
fog. Once the player is near a portal, the client gets the twin's whole render distance, the
portal camera's fog follows what is really held there, and crossing needs no load.

## Server (shared `ChunkReach`, `broadcast_world.rs`, the loader, the unload)

- `PORTAL_APPROACH_RADIUS` (chunks): a portal is *approached* when its chunk is within that many
  chunks of the player's, along x, y and z. Its value is chosen from the measured time to
  generate and send a render distance and the sprint speed (see the report).
- The area of a twin: the player's render distance (horizontal and `vertical_render_distance`)
  when its portal is approached, `PORTAL_VIEW_RADIUS` otherwise. `ChunkReach` keeps, for each
  open portal, the chunk of its twin and the way the twin faces.
- The order of an area (`twin_area_offsets(radius, facing)`, cached as `render_distance_offsets`
  is): shell after shell (the boxes the client's fog counts), the shells behind the twin's plane
  counting twice as far, nearest first within a shell. The areas of several twins are merged by
  that order, each chunk once.
- `keeps(chunk, margin)` applies the margin to the approach too: the full area is kept while the
  portal is within `PORTAL_APPROACH_RADIUS + margin`, then shrinks to `PORTAL_VIEW_RADIUS +
  margin`. No thrashing at the edge.
- The broadcast keeps the list of the portal chunks with the reach and how many of them, in
  order, the client holds all of (as for its own chunks), so the large area is not looked at
  again on every tick. Budget unchanged: `PORTAL_CHUNKS_PER_BROADCAST`. Chunks within the
  player's own render distance are never in the list, and `sent` dedups.
- The loader (`active_chunks`) and the unload (`KEPT_MARGIN`) follow `ChunkReach`, so the area
  is generated and forgotten by the same rule.

Tests (red first): an approached portal sends the twin's render distance, a far one only
`PORTAL_VIEW_RADIUS`; the order (nearest first, the front first, shells); the budget per
broadcast and the player's own chunks first; the area unloads after leaving the approach plus
the margin, not before; the loader generates the full area.

## Client

- **Held area** (`portal/held.rs`): per twin, how many shells of the area the client holds
  *and has meshed*, the chunks in front of the twin's plane only, extended shell by shell as
  chunks arrive and measured again when one leaves. `ChunkRender` gets `meshed`, false when a
  chunk of the group arrives and true once a mesh made after it is shown (or the group needs
  none).
- **Fog** (`held_reach`): the sent box is now the held box (`radius = shells held`), its top and
  bottom uncounted past `vertical_render_distance · 16` once it is as high as the player's own
  box; the reach a camera's fog ends at grows towards it at a bounded speed while the camera
  draws (no pop), and drops at once (no hole). Unit tests: a partly held area clamps, a full
  area gives the player's fog, the growth is smooth and bounded.
- **Meshing**: a chunk of a twin's area waits for its neighbours within that area too
  (`WaitingChunks`), as within the player's.
- **Layers**: unchanged rule; checked across the crossing in the e2e (first frame against
  loaded).
- **Shadow casters**: the groups within `SHADOW_CASTER_DISTANCE` of an approached twin cast too.
- **Clouds**: the mesh window covers the reach around the player and around each approached
  twin.
- **Rain occlusion**: decide from the code whether a second map around the twin is clean;
  otherwise note it.

## Tests and measures

- E2E `seam_mid_{off,high}_noon_is_seamless` and `seam_far_{off,high}_noon_is_seamless`: real
  portals (the gun), the twin 6 and 21 chunks away; the player goes away beyond the approach,
  comes back to the portal, the area is waited for, the seam measured with the close gates
  (median of 32–64 and 64–96, and 96+, sky and screen with the haze). Run red on `fb82384d`.
- Server bench `bench_portal_area`: chunks sent, bytes, ticks to send the area, at RD 8 and 12.
- `bench_portal_frame_time` before and after.
- Screens before/after in `.superpowers/sdd/portals/lot-11-screens/`.

## Steps

1. Plan (this file).
2. E2E mid and far gates written, run red on the old code (before numbers, before screens).
3. Shared `ChunkReach` areas + server tests; broadcast progress; loader and unload tests.
4. Client held area + fog + meshed flag + tests.
5. Waiting chunks, shadow casters, clouds (+ rain occlusion if clean).
6. Bench, e2e green, frame bench, docs (ARCHITECTURE, spec), report.
