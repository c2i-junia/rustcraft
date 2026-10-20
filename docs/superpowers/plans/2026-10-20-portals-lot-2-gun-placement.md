# Portals Lot 2 (Portal gun and placement) Implementation Plan

**Goal:** Players place their portals with a portal gun: the item, the gun branch of the block
interactions with the fit rules, the removal rules, and the server telling every client the
portals of the world. A placed pair is drawn by lot 1's rendering, which reads the client's
`Portals`.

**Spec:** `docs/superpowers/specs/2026-10-20-portals-design.md`, "Decisions", "Architecture"
and "Lots", lot 2. Out of scope: passing through (lot 3), the chunks around far twins (lot 4),
recursion, sounds and the body (lot 5).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- Enums append only, pinned by tests. `PROTOCOL_ID` 18 → 19, once for the branch.
- TDD: a failing test first, then the code. `cargo test` (workspace), `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean at the end.

## Decisions (where the spec is silent, or departs from it)

1. **The item.** `ItemId::PortalGun`, appended, `ItemType::Generic`, stacks to one. Its texture
   is drawn (16 × 16, white body, a blue and an orange accent): Pixel Perfection has none, as
   Minecraft has no such item. Recipe, shaped, of what is found underground: glass for the
   lens, iron ore for the body, gold ore for the core, a stick for the handle.
   ```
   glass     iron_ore  iron_ore
   .         gold_ore  iron_ore
   .         .         stick
   ```
   There is no creative inventory on this branch: the item browser of `main` lists
   `ItemId::ALL`, which holds it, and `/give portal_gun` finds it by its name.
2. **One shot per press.** `Player::portal_triggers_held` (not saved) holds whether the left
   and the right button were down in the player's previous input; a colour is shot when its
   button is down now and was not then. It follows every input, whatever is held, so that
   picking the gun while a button is down shoots nothing. The client runs the block
   interactions on every frame, aiming at a block or not, as the server does on every input:
   otherwise its idea of the buttons held could part from the server's.
3. **The gun branch.** While the selected hotbar slot holds the gun, the block interactions
   only shoot: no mining, no placing. Left is blue, right orange. A right click on a crafting
   table or a chest still opens it (the client sends no right click then).
4. **Reach.** 32 blocks (`PORTAL_GUN_REACH`) from the centre of the player to the centre of the
   aimed block, measured as the block reach is. The ray, 20 blocks long so far, goes 36 blocks
   (the reach, plus the eyes above the centre and the corner of the block).
5. **The fit** (`world/portal.rs`, `fit_portal`). The aimed face must be vertical. Of the six
   2 × 3 placements on that face that contain the aimed cell, by the distance of the centre of
   their opening to the point the ray hit (the order of their anchors breaks ties), the first
   valid one: its six wall cells hold blocks with a full hitbox that are `Solid` (not glass,
   leaves, plants or water: the portal's box lies inside the wall, and would show through), the
   six cells in front of them do not collide (air, flora, liquid), and it shares no wall cell
   with a portal on the same face but its own, the portal of the same owner and colour, which
   it replaces. The point hit is where the ray meets the plane of the aimed face, computed
   from the same ray (`raycast::aim_ray`).
6. **Refusals.** A refused shot changes nothing. The server then marks the portals as changed,
   as `touch_block` re-states a block, so that the list is sent again and corrects a client
   that predicted otherwise.
7. **What changed.** `Portals` records whether it changed since `take_changed` (an insert, a
   removal, or `touch`); the server drains it once per tick and then sends
   `ServerToClientMessage::PortalUpdates(Vec<Portal>)`, every portal of the world, to every
   player in the world. A joining player gets the list after its `AuthRegisterResponse`.
8. **Removal.** A player's portals go when it leaves (`cleanup_player_from_world`). At the end
   of the simulation of a tick (`remove_unfit_portals_system`, after the water flows, so after
   every block change of the tick: commands and mods run before it, in `Receive`), a portal one
   of whose wall or front cells changed goes when its wall and front no longer fit (rule 5).
   A wall block only hit, its breaking progress growing, still fits. Only portals near a
   change are looked at: a portal whose chunks are not loaded on the server stays.
9. **Serde.** `Portal`, `PortalColor` and `FaceDirection` derive `Serialize`/`Deserialize`;
   their variants are pinned in the message tests. `PortalUpdates` goes on the standard
   channel.
10. **The client.** `PortalUpdates` replace the client's `Portals` (`Portals::replace`). The
    prediction is the shared simulation, run by `handle_block_interactions`; a reconciliation
    replays only the movement, so a shot is never replayed. Until the server's list comes,
    the predicted portal is drawn; the server's list then wins, whatever the client
    predicted.

## Tasks

### Task 1: The item
- [x] Tests: the pinned encoding and `ALL` hold `PortalGun` (32 → 33); the gun stacks to one
  and is no tool; the recipe makes it, mirrored too.
- [x] `ItemId::PortalGun`, `get_default_type`, `get_max_stack`, the recipe, the texture
  `data/graphics/base_textures/items/PortalGun.png`.

### Task 2: The fit (`shared/src/world/portal.rs`)
- [x] Tests: the nearest of the six placements to the hit point; a placement whose wall is not
  solid full blocks, or whose front collides, gives way to the next; plants and water in front
  are fine; overlapping a portal of another colour or player is refused, one's own same colour
  is not; nothing fits anywhere → `None`.
- [x] `Portal::front_cells`, `Portal::fits`, `fit_portal`, `Portals::overlaps`.

### Task 3: The gun branch (`shared/src/players/blocks.rs`)
- [x] Tests: left places blue and right orange on the aimed wall, and neither breaks nor places
  a block; one shot per press; horizontal faces and out of reach (beyond 32, while 30 is in
  reach) are refused; shooting a colour again moves it; a refusal on the server marks the
  portals changed, on a client it does not.
- [x] `Player::portal_triggers_held`, `PORTAL_GUN_REACH`, `aim_ray`, the ray of 36 blocks, the
  branch.

### Task 4: The network
- [x] Tests: `PortalUpdates` and the variants of `PortalColor` and `FaceDirection` pinned;
  `Portals::take_changed`. `PROTOCOL_ID` 19.
- [x] Serde, the variant, the client replacing its `Portals`.

### Task 5: The server
- [x] Tests: a leaving player's portals go, the others' stay; a portal goes when a wall cell is
  broken or a front cell becomes solid, stays when water flows in front or a wall cell is only
  hit; the list is sent to every player when it changed, and to a joining player.
- [x] `remove_unfit_portals_system`, `broadcast_portals_system`, the join.

### Task 6: The client and the e2e test
- [x] The block interactions run without an aimed block.
- [x] e2e (`client/src/e2e/portals.rs`): a mod builds two walls high above the player, who holds
  the gun; a left click on one places the blue portal, a right click on the other the orange
  one, both open (their cameras exist); mining a wall cell of the blue portal by hand removes
  it on the server, which closes the orange one.

### Task 7: Docs
- [x] `docs/ARCHITECTURE.md`: the gun, the fit, the removal, `PortalUpdates`.
