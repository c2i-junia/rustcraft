# Portals

Approved proposal: https://claude.ai/artifact/F6tSa6kBQivUR53SiZX3G3 (2026-10-20, every default
accepted). Branch `feat/portals`, worktree `feat-portals`.

## Goal

Portals as in Portal: two linked openings on walls. What one shows is rendered in real time from
the other, and a player walks through one and comes out of the other without a cut, keeping its
momentum. Each player owns a blue and an orange portal, placed with a portal gun.

## Decisions

| Topic | Choice |
|---|---|
| Placement | A new item, the portal gun (`ItemId::PortalGun`, appended, stacks to one). Held in the selected hotbar slot, a left click places the blue portal and a right click the orange one, on the aimed face; it then neither breaks nor places blocks. One shot per press. Obtained with `/give` and in the creative inventory, plus a crafting recipe. |
| Size | 2 × 3 blocks (2 wide, 3 high), aligned on the block grid. |
| Surfaces | Vertical faces only (`FaceDirection::{PlusX, MinusX, PlusZ, MinusZ}`). |
| Fit | Of the six 2 × 3 placements that contain the aimed cell, the first valid one, by the distance of its centre to the hit point. Valid: the six wall cells are solid full blocks, the six cells in front of them do not collide (air, flora, liquid), and it shares no wall cell with a portal of another colour or another player, but on the opposite face (a pair through a wall one block thick). Otherwise nothing happens. |
| Reach | 32 blocks for the gun (the block reach stays 5). |
| Ownership | One pair per player, seen and crossed by everyone. Placing a colour again moves that portal. |
| Open | A portal is open when its twin exists. An open portal shows the view and lets players through; a lone one shows its colour and is a wall. |
| Lifetime | Not saved: the portals of a player go when it leaves, and all of them when the server stops. A portal also goes when one of its wall cells changes or a cell in front of it becomes solid. |
| Who passes | Players, keeping their speed, rotated with the portal. Mobs and dropped items later. |
| Network | `PROTOCOL_ID` bumped once for the branch: 19 on the branch, 22 after rebasing onto main. Save format unchanged. |

## Architecture

### `shared`

- `world/portal.rs`: `PortalColor { Blue, Orange }`, `Portal { owner: PlayerId, color, anchor:
  IVec3, face: FaceDirection }` (anchor = the lower-left wall cell seen from the front), and
  `Portals`, the portals of the world: lookup by owner and colour, the twin of a portal, the
  frame and plane of a portal in world space, and `through(portal) -> Affine3A`, the transform
  from the space in front of a portal to the space in front of its twin (`M_twin · R_y(π) ·
  M_portal⁻¹`). Pure, unit tested.
- `Portals` lives in `ChunkMap`, as the contents of the chests do, so that the simulation of both
  sides reads it: placement in `players/blocks.rs` (the gun branch of
  `simulate_player_block_interactions`), passing in `players/movement.rs`.
- Passing (`simulate_player_movement`): when the centre of the player crosses the plane of an
  open portal from its front, inside its frame, the position, the velocity and
  `player.camera_transform` go through `through(portal)`, and the simulation reports the
  rotation so that the client turns its camera (live prediction only, never while replaying
  inputs). While the centre of the player is in front of an open portal's frame, shrunk by half
  the player's width, the wall cells of that portal do not collide.
- Messages: `ServerToClientMessage::PortalUpdates(Vec<Portal>)`, every portal of the world,
  sent to everyone when one changes and to a player when it joins. Appended variants only.

### `server`

- Placement and removal are the shared simulation's; the server broadcasts the list when it
  changed after a tick, removes a player's portals when it leaves, and removes the portals whose
  wall or front changed (after the block changes of the tick).
- Chunks behind portals: each client also gets the chunks within `PORTAL_VIEW_RADIUS` (4) of the
  twin of every open portal within its render distance, after its own, and keeps them while that
  stays true (same unload margin). The chunks the server loads for players include them.

### `client`

As built, after the rulings of the ledger (`.superpowers/sdd/portals/progress.md`): Bevy's
oblique `near_clip_plane` instead of a custom projection, the main camera's tonemapping instead
of `Tonemapping::None` (colours through a portal match), `order: -level` for the recursion, and,
after the rebase onto main, `PortalGun` item 36 and `PROTOCOL_ID` 22.

New module `client/src/portal/`:

- `camera.rs`: per open portal, a `Camera3d` rendering into an `Image` the size of the window
  (`RenderTarget::Image`), `order: -level` (the deepest level of recursion first), the
  tonemapping of the player's camera, its fog ending at `PORTAL_VIEW_RADIUS` chunks. Its transform is computed in `PostUpdate` before transform
  propagation, from the player's camera: `through(portal) · camera`. Inactive when the portal is
  out of the player's view or seen from behind.
- `projection.rs`: the main camera's `PerspectiveProjection` with Bevy's oblique
  `near_clip_plane` set to the twin's plane, the plain perspective when the camera is within
  0.12 block of the plane.
- `material.rs` + `portal.wgsl`: `PortalMaterial`, unlit and unfogged, samples the portal
  camera's image at `(position.xy - viewport.xy) / viewport.zw`, or shows the portal's colour
  when closed or at the last level of recursion; a separate frame mesh draws the glowing rim on
  the wall around the opening.
- The surface is a box half a block deep drawn by its inner faces, behind the plane, so that the
  near plane of the player's camera never cuts it while passing, and the boxes of a pair through
  a wall one block thick never meet.
- `RenderLayers`: a portal camera does not see its own portal's surface (wgpu forbids sampling
  the image being written); it sees a closed-colour stand-in instead.
- `PortalUpdates` replace the client's `Portals`; the gun's placement is predicted like blocks.

## Lots

1. **Model and rendering.** `shared/src/world/portal.rs` with its tests, `Portals` in
   `ChunkMap`; the whole `client/src/portal/` rendering at recursion 1, fed by the client's
   `Portals` (no placement yet: tests insert portals into the client map). Done when: unit tests
   of the transform and the projection (a point behind the plane is clipped, one in front is
   not, depth is reversed), a golden/review screenshot of a view through a portal, and the frame
   time measured with and without a portal in view.
2. **Portal gun and placement.** The item (texture, recipe, creative), the gun branch of the
   block interactions with the fit rules, removal rules, `PortalUpdates`, join and leave,
   `PROTOCOL_ID` 19. Done when: simulation tests of fit and refusals, server tests of removal on
   leave and on block change, an e2e test placing both portals.
3. **Passing through.** Crossing, the wall that lets the player through inside the frame,
   speed and orientation, the client camera turned on live prediction only. Done when:
   simulation tests (crossing inside and outside the frame, from behind, a closed portal, speed
   rotated, replay after reconciliation), and passing without a jump in solo.
4. **The other side.** Chunks within `PORTAL_VIEW_RADIUS` of the twins sent and kept, the fog
   of the portal camera. Done when: server tests that a far twin's chunks are sent once its
   portal is within reach and unloaded after, and a screenshot through a portal to a far place.
5. **Finish.** Recursion down to 3 levels with two images per portal, the local player's body
   seen through portals (hidden from the main camera by render layer instead of a transparent
   colour), a sound when a portal opens and when passing, reusing existing sounds.

Each lot: a plan in `docs/superpowers/plans/`, TDD, `cargo test` and `cargo clippy` clean,
`docs/ARCHITECTURE.md` updated where the lot changes what it describes, review before the next
lot. Nothing merged or pushed without asking the user.
