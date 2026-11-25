# Survival Lots 9–10 (Stumpling, Drowner) Implementation Plan

**Goal:** Two of the four big hostiles on lot 8's framework. The Stumpling, a walking stump,
comes out of the open grass at night, plods at the players, fears fire, and sinks back into the
ground at dawn. The Drowner lurks in deep water, swims at the players in 3D, and drags the one it
grabs down. To see: `/time set night` on a plain away from torches, or hatch their eggs (a
Drowner's in a lake) in survival.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 9", "Lot 10", and
lot 8's extension point.

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 30. Append only: `MobKind` 13–14 `Stumpling`, `Drowner`; `MobAction` 6
  `Burrow`; `ItemId` 165–166 `StumplingSpawnEgg`, `DrownerSpawnEgg`. `FORMAT_VERSION`
  unchanged: the new player state is not saved.
- One commit per mob at least.

## Decisions (where the spec is silent)

1. **Sizes** (measured on the glb, frame 0): the Stumpling is 1.125 wide with its arms, 0.44
   deep, 2.06 tall; its box is 0.9 × 1.95 (it must fit the two free cells `may_spawn_at`
   asks for). The Drowner is 1.0 × 0.81 × 1.56; its box 0.8 × 1.5.
2. **Stumpling place** (`spawn_kind`): at night on grass. On open grass (no leaves near) it is
   always a Stumpling ("plains"); in the woods (lot 8's leaves test) a Bramble or a Stumpling,
   half each ("forests"). Its darkness is lot 8's.
3. **Stumpling numbers**: 24 health, 3 damage, reach 1.2, cooldown 20, sight 16, forget 24,
   1.2 b/s (it wanders at 0.8). Axes deal ×1.5 (`weak_to_axes`).
4. **Fire**: a Stumpling with a `Fire` or `Lava` block within 6 blocks (Euclidean, cell
   centres, the nearest found by walking a list of offsets sorted by distance: at most 925
   lookups) flees from it (`MobAction::Flee`, horizontally, at its own speed) instead of
   hunting, and does not strike while it does.
5. **Dawn**: the spawning system, which knows the time, turns every Stumpling into
   `MobAction::Burrow` (40 ticks) on the first tick of the day it finds it. A burrowing
   Stumpling skips gravity and its hunt and goes down by its height over the 40 ticks, through
   the ground; the client draws it where the server says, so the ground hides it as it sinks
   (its clip is Idle). At 0 ticks left the spawning system removes it: no loot, as the spec
   says ("leaving nothing"). A Stumpling hatched by day burrows at once.
6. **Drowner place**: an attempt whose scan down meets water first (the first non-air cell)
   measures the depth of the water column (at most 16 cells); at 3 or more the place is
   `SpawnPlace.water_depth` and `spawn_kind` gives a Drowner with odds 1 in 4 by day, 1 in 2 at
   night ("double the chance"). Water needs no darkness ("any time"). It appears in the middle
   of the column. Other caps and rings are lot 8's.
7. **Drowner numbers**: 18 health, 2 damage, reach 1.5, cooldown 20, sight 16, forget 24,
   3 b/s in water, 0.8 on land.
8. **Swimming** (`hostile::swim`): a Drowner whose centre is in water floats: no gravity, its
   velocity dropped each step (water brakes throws and knock backs, as it does a player's). It
   swims straight at its prey's centre in 3D, a step taken only if the box is free there and
   its centre stays in water; it strikes in reach. Without a prey in water it rests. On land it
   strikes in reach too, but does not chase: it walks at 0.8 to the nearest water cell within 8
   blocks (feet height and one below), or wanders.
9. **Drag** (shared): `Player.dragged_ms`, part of `MovementState` (so the acknowledgement
   carries it and the client's prediction resumes from it, as from a knock back), set to 2000
   (40 ticks of input time) by `Player::drag`. While it is above 0 and the player is in a
   liquid, the vertical velocity is −1 b/s after the swim: jump, the swim up and the shore exit
   are void. It counts down by each input's clamped delta, on land too, where it does nothing
   else. A respawn clears it.
10. **Loot**: Stumpling 2–4 sticks and charcoal 3 in 10; Drowner nothing.
11. **Client**: their models with `HOSTILE_CLIPS`; `Burrow` plays Idle. Spawn eggs drawn by
    `scripts/generate_spawn_eggs.py`.

## Tasks

1. Shared: `MobKind::Stumpling` and its stats, `weak_to_axes`, `MobAction::Burrow`,
   `spawn_kind` rows, pinned indices. Tests.
2. Server: Stumpling arm (melee + fire flight), dawn burrow and removal, loot. Tests.
3. Client: model, clip, egg; e2e hatch at night next to a survival player; snapshot. Commit.
4. Shared: `MobKind::Drowner`, `SpawnPlace.water_depth`, drag in movement. Tests (shared,
   client reconciliation).
5. Server: water spawn, swim, drag on strike, gravity gate. Tests.
6. Client: model, egg; e2e in a lake; snapshot of both. Commit.
7. `just check`, goldens, spec Decisions.
