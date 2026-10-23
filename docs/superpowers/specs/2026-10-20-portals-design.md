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
| Placement | A new item, the portal gun (`ItemId::PortalGun`, appended, stacks to one). Held in the selected hotbar slot, a left click places the blue portal and a right click the orange one, on the aimed face (its ray goes through what a player walks through: vines, flora, torches, water; lot 13); it then neither breaks nor places blocks. One shot per press. Obtained with `/give` and in the creative inventory, plus a crafting recipe: a glass lens, a body of iron ingots round a gold ingot, a stick for the handle (iron and gold ore before main had ingots). |
| Size | 2 × 3 blocks (2 wide, 3 high), aligned on the block grid. |
| Surfaces | Vertical faces only (`FaceDirection::{PlusX, MinusX, PlusZ, MinusZ}`). |
| Fit | Of the six 2 × 3 placements that contain the aimed cell, the first valid one, by the distance of its centre to the hit point. Valid: the six wall cells are solid full blocks (jungle logs and planks, furnaces lit or out, chests, diamond ore and the iron, gold and diamond blocks are; leaves, vines and cocoa pods are not), the six cells in front of them do not collide (air, flora such as ferns and vines, liquid, poured water included; a cocoa pod, a furnace or a chest collides), and it shares no wall cell with a portal of another colour or another player, but on the opposite face (a pair through a wall one block thick). Otherwise nothing happens. |
| Reach | 32 blocks for the gun (the block reach stays 5). |
| Ownership | One pair per player, seen and crossed by everyone. Placing a colour again moves that portal. |
| Open | A portal is open when its twin exists. An open portal shows the view and lets players through; a lone one shows its colour and is a wall. |
| Lifetime | Not saved: the portals of a player go when it leaves, and all of them when the server stops. A portal also goes when one of its wall cells changes or a cell in front of it starts to collide (a cocoa pod does, a vine hung on the wall does not). |
| Who passes | Players, keeping their speed, rotated with the portal. Mobs and dropped items later. |
| Network | `PROTOCOL_ID` bumped once for the branch: 19 on the branch, 22 after the first rebase onto main, 23 after the second, 24 after the third (main's jungle took 23), 25 after the fourth (main's furnace, tools and bucket took 24), 27 after the fifth (main's crafting gestures took 25 and its weather 26). `PortalUpdates` and `PortalShot` come after main's `WeatherSet` and `LightningStrike`: indices 13 and 14. Save format unchanged. |

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
after the rebases onto main, `PortalGun` item 72 and `PROTOCOL_ID` 27, and portal cameras that
draw with the shaders chosen and leave tonemapping, grading, bloom and anti-aliasing to the
player's camera. After lot 10 they draw the god-ray haze too, through Bevy's volumetric fog
with two lines of its shader changed for the oblique near plane
(`world/rendering/volumetric_fog.rs`), and no SSAO, which needs the TAA they do not have: a
close twin is crossed without a seam at Off, Low and Medium at noon, and at High and Ultra at
noon, dusk and night as measured (Ultra night not run); the gates cover Off, Low, Medium and
High at noon and High at dusk.

New module `client/src/portal/`:

- `camera.rs`: per open portal, a `Camera3d` rendering into an `Image` the size of the window
  (`RenderTarget::Image`), `order: -level` (the deepest level of recursion first), the
  tonemapping of the player's camera, its fog ending at `PORTAL_VIEW_RADIUS` chunks (as built
  after lot 9: where the chunks held around the twin end, which through a close twin are the
  player's own, so that it gets the player's fog unclamped). Its transform is computed in `PostUpdate` before transform
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

## Extension: floors and ceilings, every entity, the shot

Asked by the user on 2026-10-21, after the five lots: portals on floors and ceilings too, every
entity going through them, and the gun's shot seen flying from the player to the surface.

| Topic | Choice |
|---|---|
| Surfaces | Every face of a solid full block: the four walls, the floor (`PlusY`) and the ceiling (`MinusY`). |
| Orientation on a floor or ceiling | The 2 × 3 opening lies along the shooter's facing, rounded to one of the four horizontal directions: its "up" (the 3-block side) points away from the shooter, as a wall tipped over forwards. `Portal` gains that orientation (appended data; the protocol was already bumped once for the branch). Fit, overlap and removal rules are unchanged, applied to the cells of that footprint. |
| The transform | `through(portal)` becomes a general rigid transform between two frames (`M_twin · R_half_turn_about_the_twin's_up · M_portal⁻¹`), no longer a turn about Y. |
| Players after passing | Position and velocity go through the full transform, so falling into a floor portal shoots the player out of a wall or a ceiling portal at the same speed. The camera keeps its look direction through the transform, then is set upright again: no roll, the pitch clamped as usual, the yaw from the new look direction. The simulation reports the new yaw and pitch; the client applies them on live prediction only, as for walls. |
| Crossing and walls | The same rules on any face: the centre crosses the plane from the front inside the opening, the opening's cells don't collide while the centre is in front of the opening shrunk by half the entity's width (or height, across a horizontal opening). A player standing on a floor portal falls into it. An entity that comes out of a floor portal too slowly to rise half a block clear of it rests on it: that portal does not take it back, and collides with it once its box is out, until it walks off or jumps, or either portal of the pair is shot again or goes (lot 13), so that it goes once through a pair of floors and stands on the twin rather than blink between them. The same for every pair whose exit is a floor: walking into a wall portal whose twin is a floor, a player comes up and stands on the floor portal, rather than fall back in and out of the wall. |
| Every entity | Mobs and dropped items pass too, with the same crossing rule on their own simulation (server-side), position and velocity transformed, a mob's body yaw turned with the portal. Clients see them jump to the other side without being interpolated across the gap. |
| The shot | A short glowing bolt of the portal's colour flies from the shooter's eye to the hit point at 100 blocks a second (a third of a second at the 32-block reach), leaving a fading trail, and bursts into a few sparks where it lands; the portal opens when it lands. Every player sees every shot: the shooter predicts its own, the server tells the others (`ServerToClientMessage::PortalShot { shooter, from, to, color, placed }`, appended; `shooter` tells a client whose portal waits for the bolt). A refused shot flies too and bursts without a portal. |

### Lots

6. **Floors and ceilings.** Orientation in `Portal`, fit on horizontal faces, the general
   transform, crossing and the wall exemption across a horizontal plane, players set upright
   after passing, rendering (oblique near plane, surface box, rim, culling) on any face. Also:
   with the gun held, a right click on a container (chest, crafting table, furnace) shoots
   instead of opening it. Done when: tests of fit on floors and ceilings with each facing, of the
   transform (floor → wall, floor → ceiling, wall → floor), of falling through a floor portal
   and coming out of a wall at the same speed, a screenshot through a floor portal, and e2e
   passing floor → wall.
7. **Every entity.** Mobs and dropped items crossing on the server, no interpolation across the
   jump on clients. Done when: simulation tests for a dropped item falling through a floor portal
   and a mob walking through a wall portal, and an e2e of a dropped item thrown through.
8. **The shot.** The bolt, its trail and sparks, the delay before the portal opens on the
   shooter's screen, `PortalShot` for the others. Done when: tests of the message and of the
   bolt's flight time, and a screenshot mid-flight.

## Extension: the full view of an approached twin

Through a far twin, the portal showed only the chunks within `PORTAL_VIEW_RADIUS` (4) of it, in
fog, and crossing into chunks not sent yet showed fog where there had been terrain: the seam
gates held only for a close twin. A twin 6 or 21 chunks away now crosses as seamlessly.

| Topic | Choice |
|---|---|
| What is sent | Near an open portal, the player gets the twin's whole render distance (`TwinArea`), nearest the twin first and what lies behind its plane last, at most a quarter of each broadcast as before; past the approach and the unload margin, the area shrinks back to the view radius. About 4 900 chunks, 700 KiB over 7 broadcasts at a render distance of 8 (0.35 s), 8 900 chunks over 12 at 12. |
| Approach | 2 s ahead of the player at the speed of its stance or its velocity (`approach_radius`): 2 chunks on foot, 6 in a flying sprint, 8 at most. |
| Cap | The areas approached add at most 10 625 chunks (a render distance of 12) together to the player's own, the nearest portal first; nothing is capped up to a render distance of 12, and at 64 the twins add 3.8 % at most. |
| What the client counts as held | Whole shells of chunks around the twin, in front of its plane, held and with their group's mesh shown (`HeldAreas`, `ChunkRender::meshed`): a chunk not meshed yet is a hole, not terrain. |
| The portal camera's fog | Ends where the shells shown end, the player's own chunks counted too; their top and bottom do not count once they are as high as the player's own, so an approached twin wholly shown has the player's own fog. It opens at 96 blocks a second and closes at once. |
| Meshing | A group around a twin waits for its neighbours there as around the player (`WaitingChunks` takes every area). |
| Shadows | Groups within 12 chunks of an approached twin cast shadows, as around the player. |
| Clouds | The cloud mesh also holds a disk around each approached twin, each meshed on its own window: two disks cost twice one, however far apart. |
| Haze | Each camera, the player's and each portal camera that draws, has its own layer of haze centred under it, which only it draws (Bevy's volumetric shader changed to leave out another camera's volume): the player's view never changes with a portal. A box stretched over the twin was tried first and changed the player's own view. |
| Rain occlusion | As built after lot 12: each twin of a portal with cameras has an occlusion map of its own, centred on its opening, which its portal cameras bind instead of the player's, so roofs by a far twin stop the rain seen through the portal. Whether drops fall is decided per view: the player's eye, and for a portal camera the medium its twin opens into. |
| Tests | Seam e2e at 6 and 21 chunks, shaders Off and High, gated as a close crossing (median ≤ 6 for each band). |

### Lots

11. **The full view.** The server's approach and its tests (sent, unloaded, the order), the
    client's held shells, the eased fog, meshing, shadows and clouds around the twin. Done
    when: server and client unit tests, the mid and far seam e2e gated at Off and High noon,
    and the cost measured (chunks and time sent at render distances 8 and 12, and the frame
    time through a portal).

## As built after lot 12: the last seams

- **The grey horizon (F6)** was fixed by lot 11 (the portal cameras get the sky a frame after
  the player's camera, whose tables Bevy then takes); a gate (`horizon_colour`) now holds the
  horizon skirt within 4 of the sky at Medium, High and Ultra at noon, and at High at dusk,
  with portals opened before or after the shaders (at High noon). A patch of the sky switched off and on, for main, which
  has no portal cameras, is in `.superpowers/sdd/portals/lot-12-main-sky-switch.patch`.
- **Shadows at Medium**: every filter on the portal cameras measured within the noise of no
  shadows at all once the first measure of a run, always higher, is left out: unchanged.
- **The haze out of its layer**: a portal camera above or under its layer stands within its
  volume, stretched to hold it, and the shader marches only the layer; the player's volume is
  not cut, so its rays are marched as before. Nested views (level 2 and deeper) march no more
  steps than High, which saves 0.9–1.1 ms of 17 at Ultra with recursion 3; that the cap does
  not show rests on one facing scene, 0.15 apart against a noise of 0.08.
- **The weather**: occlusion maps per twin, the under-water decision per view (for the bolts
  too since lot 13), bolts by an
  approached twin, each view drawing only the bolts within the shown reach of its own eye, so
  a bolt by a far twin shows through the portal and not over the player's horizon. The drops
  in three portal views cost 0.09 ms at High and 0.15 ms at Ultra on the GPU (fix round 1;
  the first measure left them drawn); through the portal they are 7 % less contrasted than
  straight, still or moving, with TAA or without.
- Not changed: a strike's flash lights the player's world by its distance to the player, not
  to a twin.
- **The "unchanged view" gates** (the player's own view with a portal approached and with a
  bolt by the twin, the sky after the shaders are switched off and on; lot 13): each compares
  two windows of three shots, taken once the picture stopped drifting. The noise is the
  reference window's only, the largest difference of its shots, so that a view under test that
  flickers cannot hide behind its own noise. A class changed past 1.5 times that noise, at
  least 0.3 (0.5 for the sky gate), at most 1.5; the own-view gates also fail a window under
  test twice as noisy as the reference. Their resolution: about half a level of luma on the far
  terrain (96+), the sky and the whole picture, and 2 levels on the mid-distance terrain
  (64-96), which every run checks it would see; a single level there is not guaranteed.
