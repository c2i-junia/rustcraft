# Survival Lot 8 (Hostile spawning, difficulty, three simple mobs) Implementation Plan

**Goal:** Hostile mobs appear in the dark around the players, hunt the survival ones and hurt
them; `/difficulty peaceful` removes them. Three of them in this lot: the Gnawer (cave rat in
packs), the Bramble (bush that bites at night in the woods) and the Roller (boulder that winds
up and charges). To see: `/time set night`, walk into a cave or a forest away from torches, or
hatch a Gnawer egg in survival, and lose hearts.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 8".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 30. Append only: `MobKind` 10–12 `Gnawer`, `Bramble`, `Roller`;
  `MobAction` 4–5 `Chase`, `Attack`; `ItemId` 162–164 `GnawerSpawnEgg`, `BrambleSpawnEgg`,
  `RollerSpawnEgg`. `FORMAT_VERSION` unchanged: `WorldMeta.difficulty` has a serde default.
- New `ServerMob` state (`attack_wait`) is `#[serde(skip)]`: neither saved nor sent, so the
  pinned encodings of `MobUpdate` and of saves do not move.

## Decisions (where the spec is silent)

1. **Night** is `tick % DAY_DURATION_IN_TICKS < DAY_DURATION_IN_TICKS / 2`
   (`shared::world::hostiles::is_night`), the first half of the cycle, which starts at sunset.
2. **Difficulty** lives in `ServerWorldMap.difficulty`, loaded from and saved to `world.ron`
   (`WorldMeta.difficulty`, `WorldData.difficulty`, copied by every `From` between them).
   `/difficulty <peaceful|normal>` is a `Command` like `/weather`. The spawning system removes
   every hostile on each tick while peaceful ("at once": the next tick), and spawns none.
3. **Spawning** (`server/src/mob/spawning.rs`, after the mobs move): every 40 ticks, for each
   player, one attempt: a random angle and a horizontal distance in 24..64; a random height
   within 24 of the player's feet, from which at most 24 cells are scanned down for a floor (a
   block with a hitbox, not a liquid, with two cells free of hitboxes and liquids above). The
   spot must be 24 blocks from every player, its chunks loaded. Bounded: ≤ 24 lookups for the
   floor, the sky scan (`sees_sky`, which skips chunks of air), and a cube of 17³ cells around
   the spot for light emitters and leaves, stopped at the first emitter.
4. **Dark** = (night and the spot sees the sky) or (a block above it: `!sees_sky`), and no
   block with `light_emission() > 0` in the cube of 8 blocks around it (Chebyshev).
5. **Place → kind** (`hostiles::spawn_kind`, pure, a table later lots extend):
   - y > 100: Roller (mountains, and caves under them).
   - covered and y < 50: Gnawer 3 in 4, Roller 1 in 4 (caves).
   - night, on grass, leaves within 8 blocks: Bramble. "Forests and jungles" are told by the
     leaves around the spot (`BlockId::sapling()` is some), from the same scan, rather than by
     the biome, which the server would have to compute from the noises. Under leaves the spot
     does not see the sky: the Bramble asks for the night, not for the sky.
   - anything else: nothing (lot 9's Stumpling takes the open grass at night).
   Packs: Gnawer 2–4, others 1, cut to the caps left.
6. **Caps**: 10 hostiles within 64 blocks of the attempting player, 40 in the world. Despawn:
   a hostile farther than 96 blocks from every player is removed, checked on every attempt
   tick (40 ticks); with no player at all, every hostile is.
7. **Generic hostile behaviour** (`server/src/mob/hostile.rs`): each kind has a `Hunt`
   (sight, forget distance, speed) and a `Strike` (damage, reach, cooldown, knock back) in
   shared (`MobKind::hunt`, `MobKind::strike`). A prey is a survival, living player
   (`hostiles::is_prey`). Without a prey, a hostile looks for the nearest within its sight
   with a clear line of sight (`hostiles::line_of_sight`, eye to eye, stepped every 0.25
   block, any block with a hitbox blocks it), and wanders like an animal when it walks
   (`speed > 0`) or stands. With a prey, it keeps it until it is farther than its forget
   distance, dies, leaves or turns creative. The reach is measured from the mob's centre to the
   nearest point of the player's box. A strike is `Player::hurt(damage, Mob(kind))` then
   `knock_back` when it took health; it shows as `MobAction::Attack` for 10 ticks, so that the
   client plays the Attack clip once per strike, then back to `Chase` (or `Idle` for a mob that
   does not walk). `attack_wait` counts the cooldown down.
8. **Hit by a player**: a hostile does not flee (lot 5's `Flee`); it takes the player as its
   prey (`hostile::provoke`) whatever its sight, and keeps its knock back.
9. **Numbers** (spec): Gnawer 6 health, 2 damage, 2.8 b/s, cooldown 15, reach 1.0, sight 16,
   size 0.6 × 0.6 (the model is 0.375 wide, 0.625 tall with its ears, 1.5 long with its tail:
   the spec's 0.6 width, its height measured). Bramble 12 health, 3 damage every 20 ticks within
   2.5 blocks, never walks (sight and reach 2.5), size 1.0 × 0.75 (measured). Roller 16
   health, sight 12, wind-up 20 ticks (Attack), rolls at 7 b/s straight along where it faced
   when the wind-up ended (`Chase`, the Walk clip), at most 60 ticks; hitting a prey's box:
   4 damage, knock back 8, then rests 20 ticks; hitting a block (it does not climb): stunned 40
   ticks (`Idle`); size 0.75 (measured). Melee kinds (Gnawer, Bramble) knock back nothing.
   The roll moves the position without a horizontal velocity, so that `is_flung` does not take
   it for a throw.
10. **Loot** in `combat::loot`: Gnawer nothing, Bramble 1–3 sticks, Roller 1–2 cobblestone and
    flint 1 in 5.
11. **Client**: `MobClip::Attack`; `HOSTILE_CLIPS` = Attack 0, Idle 1, Walk 2; `Chase` plays
    Walk, `Attack` plays Attack once (not repeated); the Roller's roll is its Walk clip.
12. **Spawn eggs** for the three, drawn by `scripts/generate_spawn_eggs.py`, Mobs category:
    creative testing, and the e2e test.

## Extension point for lots 9–12

`hostile::act` does the generic part (cooldown, prey kept or forgotten, prey found) and then
calls one arm per kind in `hostile::hunt`:

```rust
match mob.kind {
    MobKind::Gnawer | MobKind::Bramble => melee(mob, chunks, prey, delta),
    MobKind::Roller => roll(mob, chunks, players, prey, delta),
    // lot 9: MobKind::Stumpling => melee(...) plus its fire and dawn rules
    // lot 10: MobKind::Drowner => swim(...), lot 11: Cinderling => hover_and_throw(...),
    // lot 12: GustOwl => circle_and_dive(...)
    _ => {}
}
```

A new kind adds: its `MobKind` variant, `health`, `size`, `display_name`, `is_hostile`,
`hunt`, `strike`, an arm in `spawn_kind` (place) and in `combat::loot`, an arm above, and its
model in `client::mob::models::model` (with `HOSTILE_CLIPS`). A flying or swimming kind
(lots 10–12) will also need a gate around `behavior::apply_gravity`, which every kind of this
lot goes through.

## Tasks

1. Shared rules: `is_night`, `Difficulty`, `MobKind` variants and stats, `is_dark`,
   `surroundings` scan, `spawn_kind`, `pack_size`, `line_of_sight`, `is_prey`. Tests.
2. Difficulty: `WorldMeta`/`WorldData`/`ServerWorldMap`, load and save, `/difficulty` with
   completion. Tests (ron round trip, old file reads Normal, parse, completion).
3. Hostile framework in the server: `hostile.rs` (prey, chase, forget, strike, cooldown),
   `combat` provoke, `spawning.rs` (attempts, ring, caps, despawn, peaceful). Tests.
4. Client clips: `MobClip::Attack`, `HOSTILE_CLIPS`, play once. Tests.
5. Gnawer (packs, caves), Bramble (stationary), Roller (wind-up, roll, stun), loot, eggs.
6. e2e: a Gnawer hatched next to a survival player bites it; snapshot of the three models.
7. `just check`, goldens, spec Decisions.
