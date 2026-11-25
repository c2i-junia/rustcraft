# Survival Lot 5 (Combat, on top of main's blows) Implementation Plan

**Goal:** Main's blows become a fight: a hit mob is invulnerable for 10 ticks, knocked back
away from the player, flees for 5 s; a blow wears the weapon; a hit mob flashes red and is
heard; every animal drops its loot (meat, leather, feathers), and the meats are roasted in the
furnace or on a campfire and eaten. To see: hit a pig with a sword, roast the porkchops.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 5".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 30. `ItemId` is append only: `RawPork` 149, `RoastPork` 150, `RawBeef`
  151, `RoastBeef` 152, `RawChicken` 153, `RoastChicken` 154, `RawRabbit` 155, `RoastRabbit`
  156, `Leather` 157, `Feather` 158. `FORMAT_VERSION` unchanged: the new `ServerMob` field is
  `#[serde(skip)]`.
- `timeout 600 cargo test -p <crate>` while working, `timeout 1200 just check` at the end.

## Decisions (where the spec is silent)

1. **Invulnerability.** `ServerMob.hurt_ticks: u32`, `#[serde(skip)]` (neither saved nor
   sent), set to `MOB_INVULNERABILITY_TICKS` (10) by a blow that lands and counted down at
   the start of every mob step (`step_mobs`). A blow on a mob still invulnerable swings
   (the click is taken, the player's wait restarts) but does nothing: no damage, no knock
   back, no wear.
2. **Knock back.** The horizontal velocity becomes `KNOCK_BACK_SPEED` (6 blocks/s) away from
   the player (along the camera when they stand on the same spot), main's hop (4 blocks/s up)
   kept. 6 is above every flee speed but the horse's (6), so the mob is "flung" and flies by
   main's `fly` until it lands, which stops it: no new physics. A horse is not pushed
   sideways, only hopped.
3. **Flee for 5 s.** A blow sets `Flee` from the player with `action_ticks = FLEE_TICKS`
   (100). A fleeing mob counts its `action_ticks` down when it has some, and rests when they
   run out or it is `SAFE_DISTANCE` away, as before. A shy animal that flees a player who
   came close starts with 0 ticks: it flees until it is safe, as before.
4. **Damage.** `MobKind::blow_damage(held)` = `attack_damage` (1 bare-handed), ×1.5 rounded
   down with an axe against a kind whose `weak_to_axes()` is true: none yet, the Stumpling
   (lot 9) will be.
5. **Wear.** `ItemId::blow_wear()`: 1 for a sword, 2 for any other mining tool, 0 for the
   rest. The spear has no durability in main: it stays unworn (the spec's "1 for the spear"
   needs a durability main never gave it).
6. **Loot** (`server/src/mob/combat.rs`, `loot(mob, rng)`): sheep 1–2 `RawMutton` + 1 `Wool`
   unless sheared (main's), pig 1–3 `RawPork`, cow 1–3 `RawBeef` + 0–2 `Leather`, chicken 1
   `RawChicken` + 0–2 `Feather`, rabbit 1 `RawRabbit`, the others nothing. Dropped as main
   drops them, where the mob was, popping up.
7. **Food** (main's `food_health`, two points to a heart, Minecraft's food points until lot 6):
   raw pork 3, raw beef 3, raw chicken 2, raw rabbit 3, raw mutton 2 (main's raw mutton was
   not edible; lot 6 makes every meat a food, so it is now), roast pork 8, roast beef 8, roast
   chicken 6, roast rabbit 5, roast mutton 6 (main's).
8. **Cooking.** `smelting_result` turns each raw meat into its roast; the campfire roasts them
   too, as it does main's mutton (any raw item whose smelting result exists is accepted by the
   campfire's list of raw foods).
9. **Items.** Category `Materials`, as main's mutton, generic, stack 64. Textures from Pixel
   Perfection: `item/{porkchop,cooked_porkchop,beef,cooked_beef,chicken,cooked_chicken,
   rabbit,cooked_rabbit,leather,feather}.png`.
10. **Hurt flash, client** (`client/src/mob/hurt.rs`). The client sees a hurt when an update
    raises the mob's `damage` (which `MobUpdate` already carries: no new field travels). The
    mob entity's `Mob.hurt_seconds` starts at 0; while under `HURT_FLASH_SECONDS` (0.3) every
    mesh of its model wears a red copy of its material (one copy per original material, kept
    in `HurtMaterials`), and gets its own back after. A `MobHurt { at }` message plays
    `Sound::MobHurt` (`sounds/mob/hurt.ogg`, a punch and a squeal, `build_mob_hurt` in
    `scripts/generate_sounds.py`, written with `--only mob/`) where the mob is. The killing blow
    removes the mob at once, so it neither flashes nor is heard.

## Tasks

### Task 1: Items, food, cooking (shared)
- [x] Tests: pinned indices of the 10 items; their category, food and smelting; the campfire
  roasts a raw porkchop; `blow_wear` and `blow_damage`.
- [x] Items, textures, credits; `food_health`, `smelting_result`, campfire; `blow_wear`,
  `blow_damage`, `weak_to_axes`.

### Task 2: Blows (server)
- [x] Tests (combat.rs): a second blow within 10 ticks, from another player, is ignored; after
  them it lands; the mob is knocked back horizontally away from the player; it flees the
  player for 5 s, then rests; a sword loses 1, a pickaxe 2; each animal's loot, seeded.
- [x] `hurt_ticks`, knock back, flee ticks, wear, `loot`.

### Task 3: Hurt flash and sound (client)
- [x] Tests: an update that raises the damage starts the flash and writes `MobHurt`, one that
  does not, or a mob first seen hurt, does not; the meshes of a flashing mob wear the red
  copy, then their own material; `MobHurt` plays `Sound::MobHurt` where the mob is.
- [x] `hurt.rs`, sound, plugin.

### Task 4: Goldens, e2e counts, `just check`
- [ ] Item browser e2e counts 149 → 159; regenerate the goldens the new items change only.
