# Survival Lot 2 (Drowning) Implementation Plan

**Goal:** A survival player whose eyes are in a liquid loses a tick of air a tick, from 300 (10
bubbles of 30); out of it, the air comes back 6 a tick. With no air left, it takes 2 damage
every 20 ticks (`DamageSource::Drowning`). Creative never loses air, a respawn refills it. The
HUD shows the bubbles on the right above the hotbar while the air is not full. To see: walk into
a lake deeper than 2 blocks and stay at the bottom.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 2".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 29 (`PlayerUpdateEvent` gains `air`). `FORMAT_VERSION` unchanged: the air
  is not saved.
- `timeout 600 cargo test -p <crate>` while working, `timeout 1200 just check` at the end.

## Decisions (where the spec is silent)

1. **The eyes, one notion for the client and the server.** `EYE_HEIGHT` (0.8 above the centre
   of the box) and `SNEAK_BODY_SCALE` move from the client to `shared/src/players/constants.rs`.
   `Player::eye_position()` is where the camera ends up once the stance has eased: a sneaking
   player's eyes are lower. `eye_under_water(map, eye)` moves from `client/src/camera/
   underwater.rs` to `shared/src/world/liquid.rs` as `eye_in_liquid`: the eye is under the
   surface of the liquid of its cell, so the fog, the rain's sounds and the air agree. It reads
   any liquid (`BlockTransparency::Liquid`), so lot 4's lava counts.
2. **Where things live.** `shared/src/players/air.rs`: `MAX_AIR` (300), `AIR_REFILL` (6),
   `DROWNING_DAMAGE` (2), `DROWNING_INTERVAL` (20), `Player::breathe(eyes_in_liquid)`, and the
   HUD's pure rule `bubble(air, index)`. Both `Player.air: u16` and `Player.drowning_ticks: u16`
   (the ticks spent without air since the last hit) are `#[serde(skip)]`, the air defaulting to
   `MAX_AIR`: rejoining refills it, as it does the lot 1 timers.
3. **The cadence.** In a liquid, `breathe` takes a tick of air; at 0 it counts
   `drowning_ticks`, and every 20th hurts and starts the count again, as Minecraft's air going
   down to −20. So the first hit lands 20 ticks after the air runs out. Out of the liquid the
   count is reset. A dead player breathes no more. Creative refills as out of the liquid.
4. **Server.** `breathe_system` in `world/simulation.rs`, in `ServerSet::Simulate` after the
   inputs moved the players: `player.breathe(eye_in_liquid(chunks, player.eye_position()))`.
5. **Client.** `PlayerUpdateEvent.air` sets `Player.air`. `client/src/ui/hud/air.rs` spawns
   `AirBar`, a row as a child of the hotbar, at its right, `bottom: 100%`, filled from the
   right: like Minecraft, `bubble` gives `ceil((air − 2) × 10 / 300)` full bubbles and the next
   one bursting until `ceil(air × 10 / 300)`; the others are not drawn. It shows while the
   hearts would (survival, inventory closed) and the air is below `MAX_AIR`. Its label for the
   accessibility tree and the tests is `Air: 9 of 10 bubbles`. Lot 6 raises `AirBar` by the
   food row: its `Node.margin.bottom`.
6. **Textures.** Pixel Perfection's `gui/sprites/hud/air.png` and `air_bursting.png` (9x9, like
   the hearts) go to `data/graphics/hud/`, credited in `docs/ressources.txt`.

## Tasks

### Task 1: Air in shared
- [x] Tests (`air.rs`): eyes in a liquid lose a tick of air a tick; out of it the air refills
  6 a tick up to `MAX_AIR`; at 0, 2 damage every 20 ticks and a death by `Drowning`; creative
  loses none; a respawn refills; the bubbles of an air; `eye_position` with and without sneaking.
- [x] Tests (`liquid.rs`): the eyes in a liquid only below its surface; only the feet in water
  are not the eyes.
- [x] Constants, `breathe`, `bubble`, `eye_position`, `eye_in_liquid`, the client on the shared
  constants and function.

### Task 2: Server
- [x] Test: `breathe_system` takes air from a player whose head is under water, not from one
  whose feet only are.
- [x] The system in the schedule, `air` in `PlayerUpdateEvent`.

### Task 3: Client
- [x] Unit test: the label of the bubbles.
- [x] e2e (`survival.rs`): a player standing at the bottom of a lake sees the bubbles appear and
  go down.
- [x] `air` from the update, the HUD row, the textures and their credit, `docs/ARCHITECTURE.md`.

## Notes

- The lake e2e test puts the player at the bottom of the first lake 4 blocks deep or more near
  the spawn of the seed 42, found from the generator's column shapes at run time. It starts
  under water, so the check that no bubble shows with a full air went to the deadly fall test,
  on land.
