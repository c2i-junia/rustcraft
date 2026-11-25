# Survival Lots 11–12 (Cinderling, Gust Owl) Implementation Plan

**Goal:** The two fliers on lot 8's framework. The Cinderling, a burning rock, hovers in the
deep caves, keeps its distance and lobs embers that hurt, set the player burning and light
what they land on; water and rain put it out. The Gust Owl circles high over the players of
the mountains at night and dives at them, throwing them off the ledges; sneaking halves any
knock back. To see: hatch their eggs next to a survival player, or `/time set night` on a peak
above y = 110, or walk the caves below y = 20.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 11", "Lot 12",
and the extension points of lots 8–10.

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 30. Append only: `MobKind` 15–17 `Cinderling`, `Ember`, `GustOwl`;
  `ItemId` 167–168 `CinderlingSpawnEgg`, `GustOwlSpawnEgg`. `FORMAT_VERSION` unchanged: no
  saved encoding changes (embers are not saved at all).
- One commit per mob at least.

## Decisions (where the spec is silent)

1. **Sizes** (measured on the glb, frame 0, Blender's z up): the Cinderling is 1.125 wide with
   its orbiting embers (its body 0.625), from 0.19 to 1.75 tall; its box is 0.7 × 1.6, drawn
   from its origin as the others. The Gust Owl is 2.5 wide with its wings spread, 0.94 deep,
   and its lowest voxel (the talons) is 0.4375 above the model's origin, its top 1.875: its
   box is 0.7 × 1.45, and the client lowers its model by that 0.4375 more than the others
   (`models::model_lift`) so that the talons sit at the bottom of the box. The Ember is 2 × 2 ×
   2 voxels (0.125); its box 0.25.
2. **The ember is a mob kind** (`MobKind::Ember`), as the spec suggests: main's portal gun
   bolt is drawn by the client alone and its hit is instant (no server projectile), and a
   dropped item is an item (picked up, saved, merged). An ember is neither hostile nor an
   animal (`MobKind::is_projectile`): `behavior::step_mobs` and `push_mobs` skip it, blows do
   not aim at it (`combat::aimed_mob`), the hostile caps do not count it, the natural animals
   are not of its kind, peaceful removes it with the hostiles, and the save leaves it out
   (`save::saved_mobs`), so that a load never finds one. Its `action_ticks` are its life left.
3. **Ember flight** (`server/src/mob/cinder.rs`, `cinder_system`, chained after the mobs move
   and before the spawning): exact ballistics under `EMBER_GRAVITY` (−16 b/s²), stepped by at
   most 0.25 block. It hits the first prey whose box its own touches: 1 damage
   (`DamageSource::Mob(Cinderling)`, so the death reads "A Cinderling got you") and burning for
   at least 60 ticks (`EMBER_BURNING_TICKS`). It lands in the first cell with a hitbox or a
   liquid: on a flammable block (`fire::flammability`) it lights the air cell it came from
   (`BlockId::Fire`, which the fire ticks then schedule as one a player lit); in water or lava
   it goes out. It goes after 100 ticks (`EMBER_LIFE_TICKS`), or over unloaded terrain.
4. **Cinderling place** (`spawn_kind`): covered and below y = 20 (`CINDER_HEIGHT`), in lot 8's
   dark: a Cinderling 1 in 3, else lot 8's cave rule (Gnawers or a Roller).
5. **Cinderling numbers**: 10 health, sight 16, forget 24, 2 b/s. Its `Strike` is its throw:
   1 damage, reach 16 (how far it throws), cooldown 60, no knock back. The throw needs a line
   of sight; it shows as `Attack` for 10 ticks. The ember leaves its centre aimed at the
   prey's centre, its flight lasting the horizontal distance / 8 b/s (0.25 to 2 s).
6. **Hover** (`hostile::hover_and_throw`): it floats (`behavior::floats`, its velocity dropped
   so a blow does not fling it). Its centre goes toward 2 blocks plus half its height above the
   first block with a hitbox or liquid within 8 cells below it (kept as is over a deeper void),
   and horizontally toward the ring at 6 blocks of its prey (it holds between 5.5 and 6.5), by
   position steps at its speed checked by `may_move_to`: straight, else vertical only, else
   horizontal only. Without a prey it hovers in place (`Idle`).
7. **Water and rain** (`cinder_system`): a Cinderling whose box touches a water cell, or whose
   centre sees the sky (`ticks::sees_sky`) while it rains or storms, goes out: removed, without
   loot (it was not killed). Loot when killed: 1–2 coal, flint 1 in 10.
8. **Gust Owl place**: at night under the open sky above y = 110 (`OWL_HEIGHT`), in lot 8's
   dark: a Gust Owl 1 in 2, else lot 8's rules (a Roller above y = 100).
9. **Gust Owl numbers**: 14 health, sight 24 (it hunts from 8 blocks up), forget 32, circles
   at 4 b/s. Strike: 2 damage, reach 0.5 (from its centre to the prey's box), cooldown 60 (the
   circling between two dives), knock back 8.
10. **Circle and dive** (`hostile::circle_and_dive`): it always floats. With a prey it circles
    (`Chase`, Walk clip) round a ring 4 blocks wide at 8 blocks above the prey's feet (the
    6–10 band), the point ahead on the ring moved by its speed; once its cooldown is over,
    within 6–10 blocks above the prey's feet and in sight, it dives (`Attack`, the clip played
    once) straight at the prey's centre at 10 b/s for 40 ticks at most. A dive that reaches the
    prey strikes (hurt then knock back along the dive) and climbs back to the circle; one a
    block stops, or that runs out, too, cooldown restarted. Without a prey it hovers (`Idle`).
11. **Thrown off**: lot 1's `last_mob_hit` already makes a fall within 60 ticks of the owl's
    hit `ThrownOff(GustOwl)`, "A Gust Owl threw you off a ledge" (a test checks it).
12. **Sneaking** halves any knock back (`Player::knock_back`, shared, so whatever pushes —
    Roller, Gust Owl, later sources — and the client's prediction agree).
13. **Client**: the three models with `HOSTILE_CLIPS` (the Ember's three clips are one slow
    spin); spawn eggs for the Cinderling and the Gust Owl (`scripts/generate_spawn_eggs.py`),
    none for the Ember.

## Tasks

1. Shared: `MobKind::Cinderling`, `Ember`, `is_projectile`, stats, `spawn_kind` row, pinned
   indices, egg. Tests.
2. Server: `hover_and_throw`, `cinder.rs` (flight, hit, fire, quench), skips for projectiles,
   save filter, loot. Tests with seeded rngs.
3. Model: Ember in `generate_hostiles.py`; client models and egg; e2e: hatched Cinderling sets
   a survival player burning; snapshot. Commit.
4. Shared: `MobKind::GustOwl`, `spawn_kind` row, sneak halving, ThrownOff message. Tests.
5. Server: `circle_and_dive`, loot. Tests.
6. Client: model with its lift, egg; e2e dive; line-up snapshot of all. Commit.
7. `just check`, goldens, spec Decisions.
