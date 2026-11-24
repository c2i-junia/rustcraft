# Survival, death, hunger and hostile mobs — design

Date: 2026-11-24. Branch `feat/survival-death`, worktree `feat-survival-death/`, from main
`80544cca` (protocol 29; first cut from `c5eec11b`, rebased on 2026-11-25).

The user asked for death in survival (falls, drowning, fire), hostile mobs modelled in Blender,
and a visual proposal first: https://claude.ai/artifact/4KGRvVzjr4xWM7AV6Vyh4X . They then said
"go" on every recommendation of the proposal and widened the scope: simpler hostile mobs as
well, lava, hunger, meat, and some farming. Every other decision is delegated; this file records
them. Later rulings are appended under **Decisions**.

## Rulings from the proposal

- **Death drops the inventory** where the player died, as dropped items (the item-dropping code
  exists). Creative never drops.
- **All mobs**: the four of the proposal plus three simple ones.
- **Lava is in** (the user overrode the proposal's "no").
- **`/difficulty peaceful|normal`**, per world, saved in `world.ron`; peaceful removes and stops
  hostile mobs only.
- **Hunger is in** (the user overrode the proposal's "out"), with meat and crops to eat.

## Global rules

- Server authoritative, 20 ticks/s. Pure rules (damage, air, hunger, flow, growth, fire spread)
  live in `shared` as functions unit-tested there; server systems apply them.
- Enums that travel or are saved (`BlockId`, `ItemId`, `MobKind`, `DeathCause`, messages,
  `MobAction`, `NetworkAction`) are **append only**, and their pinned-index tests get the new
  variants. `PROTOCOL_ID` goes 29 → 30 once, in lot 1, and stays 30 for the whole branch.
  `FORMAT_VERSION` only moves if a saved encoding changes; new `Player`/`WorldMeta` fields use
  `#[serde(default)]` so old saves load.
- Textures come from Pixel Perfection (see `CLAUDE.md`), already downloaded to
  `/tmp/claude-1000/-home-user-Code-rustcraft/9b840b79-4241-4412-a63d-04e24ad06d76/scratchpad/pp/`
  (`block/`, `item/`, `gui/sprites/hud/`); fetch any other file from the raw GitHub URL. Credit
  each file in `docs/ressources.txt` and in the commit message.
- Sounds: `data/sounds/player/hurt.ogg` already exists and nothing plays it. New sounds follow
  `scripts/generate_sounds.py`.
- Mob models: `scripts/generate_hostiles.py` (Blender, headless) builds
  `data/models/animated/{Stumpling,Drowner,Cinderling,GustOwl,Gnawer,Bramble,Roller}.glb`,
  already generated, each with clips Attack / Idle / Walk = glTF animations 0 / 1 / 2, 24 fps,
  metres, feet on the origin, facing +Z. Run it with
  `blender --background --factory-startup --python scripts/generate_hostiles.py -- [Name ...]`.
- Each lot: its plan in `docs/superpowers/plans/2026-11-24-survival-lot-N-<topic>.md`, test
  first, `just check` green, one or more Conventional Commits in English, **no attribution
  trailer**. Game text stays in English.

## Lots

### Lot 1 — one door for damage, and feedback

- `DamageSource` in shared: `Fall`, `Void`, `Drowning`, `Fire`, `Lava`, `Starvation`,
  `Mob(MobKind)`. `Player::hurt(amount, source)` replaces `hurt(damage, cause)`: game mode,
  **invulnerability** of 10 ticks after a hit (a second hit inside it is ignored; Void and
  Starvation pierce it), sets `death_cause` from the source.
- `DeathCause` gains, appended: `Drowning` "You ran out of air", `Fire` "You went up in
  flames", `Lava` "You tried to swim in lava", `Starvation` "You starved to death",
  `Mob(MobKind)` "<A Stumpling> got you", `ThrownOff(MobKind)` "<A Gust Owl> threw you off a
  ledge". A fall within 3 s (60 ticks) of a hit by a mob is `ThrownOff` that mob. Mob display
  names: Stumpling, Drowner, Cinderling, Gust Owl, Gnawer, Bramble, Roller, and the animals'.
- `Player::knock_back(direction, strength)`: horizontal push and a small hop, applied through
  the movement state the client reconciles with.
- Client feedback when the local player's health drops: red edge vignette fading over 0.5 s,
  a short camera tilt, `player/hurt.ogg`. Hearts blink briefly.
- PROTOCOL_ID 30.

### Lot 2 — drowning

- `Player.air: u16`, max 300 ticks (10 bubbles of 30). It drops by 1 a tick while the **eyes**
  are in a liquid (the block at eye height), refills 6 a tick otherwise. At 0: 2 damage every
  20 ticks, `DamageSource::Drowning`. Creative never loses air. Respawn refills.
- HUD: 10 bubbles (`air.png`, `air_bursting.png` for the one bursting) on the right above the
  hotbar, mirroring the hearts, shown only while air < max. Lot 6 puts the food row there;
  bubbles then sit above food.

### Lot 3 — random block ticks and fire

- A server `random_block_ticks` system: each tick, for each loaded chunk near a player (within
  the simulation distance), 3 random blocks per 16³ section get `random_tick`. Shared pure rules
  decide what each block does; this lot uses it for fire, lot 7 for crops.
- `BlockId::Fire`: no hitbox, a small ray hitbox so a left click puts it out at once, cross
  shape like tall grass, light 15, `Decoration` (water washes it away), animated `fire_0.png`
  (8 frames, `frame_time: 2`). Age kept in `BlockData.level`.
- Flammability table (shared): logs, planks, leaves, tall grass, fern, vine, flowers, cocoa,
  crafting table catch and burn; stone, dirt, sand, chest etc. do not.
- Fire rules on its random tick: it ages; past age 15 or with no support (solid block below or
  a flammable neighbour) it goes out. Each tick it may set a flammable neighbour on fire (chance
  by flammability) and may burn the block it sits on / next to it into air. Rain puts out fire
  that sees the sky. At most 64 fire updates per tick in the world.
- `ItemId::Flint` (gravel drops it 1 time in 10, else gravel), `ItemId::FlintAndSteel`
  (shapeless iron ingot + flint, durability 64): right click on a face sets fire on the air
  block in front.
- Player in a fire block: `burning_ticks = 80` (4 s). While burning, 1 damage every 20 ticks
  (`DamageSource::Fire`). Water (feet in water) or rain clears burning. Sent to the client:
  a flame overlay at the bottom of the screen while burning.

### Lot 4 — lava

- `BlockId::Lava`: liquid like water, light 15, animated `lava_still.png` (16 frames,
  `frame_time: 4`), opaque-ish orange fog when the eyes are in it, movement slower than water.
- It flows with the water rules made generic over the liquid: lava spreads 3 blocks on flat
  ground and steps every 30 ticks instead of 5.
- Water meets lava: a lava source becomes `BlockId::Obsidian` (new, diamond pickaxe, long break
  time, `obsidian.png`), flowing lava becomes cobblestone.
- In lava: 4 damage every 10 ticks (`DamageSource::Lava`) and burning set to 300 ticks. Lava sets
  flammable neighbours on fire on its random ticks.
- `ItemId::LavaBucket` (`lava_bucket.png`): the bucket takes lava sources and pours them like
  water. It is also furnace fuel (20 000 ticks, gives the bucket back).
- World generation: cave air at y ≤ 10 is filled with lava sources (lava lakes at the bottom of
  caves). Spawn search must not put players in lava.

### Lot 5 — combat

- Client→server `AttackMob { mob_id }`, appended. The client aims at mob hitboxes as well as
  blocks: a left click on a mob closer than the aimed block attacks it (no mining). The server
  checks reach (3.5 blocks from the eyes to the hitbox) and a 10-tick cooldown.
- Damage: fist 1; wooden and golden sword 4, stone 5, iron 6, diamond 7; axes wood 3, gold 3,
  stone 4, iron 5, diamond 6; other tools 2. A weapon loses 1 durability per hit (2 for tools
  that are not swords). Axes deal ×1.5 to Stumpling (lot 9).
- `ServerMob` gains `health` and `hurt_ticks` (10 ticks of invulnerability). Per kind max
  health: chicken 4, rabbit 3, sheep 8, pig 10, cow 10, goat 10, fox 10, wolf 8, cat 10,
  horse 15. A hit knocks the mob back (6 blocks/s horizontally, a small hop) and makes animals
  flee for 5 s. `MobUpdate` carries the hurt flash so the client tints the mob red for 0.3 s and
  plays a hurt sound.
- At 0 health the mob is removed and drops loot as dropped items: pig 1–3 RawPorkchop, cow 1–3
  RawBeef + 0–2 Leather, sheep 1–2 RawMutton, chicken 1 RawChicken + 0–2 Feather, rabbit 1
  RawRabbit; others nothing. New items with their textures: RawPorkchop, CookedPorkchop,
  RawBeef, CookedBeef, RawMutton, CookedMutton, RawChicken, CookedChicken, RawRabbit,
  CookedRabbit, Leather, Feather (category: a new `Food` category for food; Leather and
  Feather go to Materials). Furnace smelts each raw meat into the cooked one.

### Lot 6 — hunger and eating

- `Player.food: u8` (0–20), `saturation: f32` (≤ food), `exhaustion: f32`. Exhaustion grows:
  sprinting 0.1 per block, jumping 0.05 (0.2 while sprinting), swimming 0.01 per block, breaking
  a block 0.005, attacking 0.1, taking damage 0.1, regenerating a point of health 6. At 4 it
  wraps: saturation −1, or food −1 when saturation is 0.
- Health regeneration (existing 1 point per 80 ticks) only when food ≥ 18. At food 0, 1 damage
  every 80 ticks (`DamageSource::Starvation`, which can kill). Sprinting is refused at food ≤ 6.
  Creative: no hunger. Peaceful does not change hunger. Respawn: food 20, saturation 5.
- Eating: holding right click with a food item for 32 ticks (1.6 s) eats it (stack −1) when food
  < 20. The client shows the held item bobbing and plays a munch sound; letting go cancels.
- Food values (food, saturation): Bread 5/6, Carrot 3/3.6, Potato 1/0.6, BakedPotato 5/6,
  RawPorkchop 3/1.8, CookedPorkchop 8/12.8, RawBeef 3/1.8, CookedBeef 8/12.8, RawMutton 2/1.2,
  CookedMutton 6/9.6, RawChicken 2/1.2, CookedChicken 6/7.2, RawRabbit 3/1.8, CookedRabbit 5/6.
- HUD: 10 drumsticks (`food_full/half/empty.png`) right above the hotbar, filled right to left;
  the `*_hunger` variants are not used. Bubbles move above them.

### Lot 7 — farming

- `BlockId::Farmland`: a hoe right-clicked on the top of grass or dirt with air above makes
  farmland (hoe durability −1). Texture `farmland.png` top, `farmland_moist.png` when moist (water
  within 4 blocks horizontally, same level or one below; recomputed on random ticks; state in
  `BlockData.level`), dirt sides. Breaking it drops dirt. A block placed on it or a jump onto it
  is not needed in this lot.
- Crops: `BlockId::Wheat`, `Carrots`, `Potatoes`, decorations with a cross shape, no hitbox,
  age 0–7 in `BlockData.level`, one texture per stage (wheat `wheat_stage0..6` +
  `wheat_stage_full_7` scaled to 16x16; carrots and potatoes use stage `age / 2` of their 4).
  They only stand on farmland; without it they pop off as their seed item.
- Growth on random ticks: one age with chance 1/3 on moist farmland, 1/6 on dry.
- Items: WheatSeeds (12.5 % drop from tall grass and fern), Wheat, Bread (crafting: three wheat
  in a row), Carrot and Potato (each 2 % from tall grass; they are planted as crops), BakedPotato
  (furnace). Breaking ripe wheat drops 1 wheat + 0–3 seeds, unripe 1 seed; ripe carrots or
  potatoes drop 2–4, unripe 1.
- Right click on the top of farmland with seeds, a carrot or a potato plants it (eating has
  priority only when not aiming at farmland).

### Lot 8 — hostile spawning, difficulty, and the three simple mobs

- Shared `is_night(tick)` from `DAY_DURATION_IN_TICKS` (the cycle starts at sunset; night is
  the first half). `WorldMeta.difficulty: Difficulty { Peaceful, Normal }` (default Normal),
  `/difficulty <peaceful|normal>` with completion; peaceful removes hostile mobs at once.
- Spawning: every 40 ticks per player, one attempt at a random loaded column 24–64 blocks away
  horizontally. Dark = (night and sky visible) or (a solid block above) and no light emitter
  (torch, lit furnace, fire, lava) within 8 blocks. The place picks the kind (table below).
  Caps: 10 hostiles per player within 64 blocks, 40 per world. Hostiles 96 blocks away from
  every player despawn. Spawning ignores the game mode; attacks ignore creative and dead
  players.
- Generic hostile behaviour: `MobAction` gains, appended, `Chase` and `Attack`. A hostile sees a
  survival player within 16 blocks with a clear line of sight, walks at it, attacks when in
  reach, with a per-kind cooldown. It forgets the player 24 blocks away. Hostiles have health,
  take hits like animals (lot 5), and drop loot.
- Client: a `HOSTILE_CLIPS` table (Attack 0, Idle 1, Walk 2); `Chase` plays Walk, `Attack` plays
  Attack once.
- **Gnawer** (cave rat): caves (solid block above, y < 50), packs of 2–4. 6 health, 2 damage,
  2.8 blocks/s, cooldown 15 ticks, small hitbox (0.6 × 0.5). Drops nothing.
- **Bramble** (bush ambusher): forests and jungles at night, on grass. Never moves; when a
  player comes within 2.5 blocks its lid opens (Attack) for 3 damage every 20 ticks. 12 health.
  Stays at day. Drops 1–3 sticks.
- **Roller** (boulder): caves and mountains (y > 100). When it sees a player within 12 blocks it
  winds up for 20 ticks (Attack clip), then rolls straight at 7 blocks/s until it hits a block or
  a player: 4 damage and a strong knock back (8). After hitting a block it is stunned 40 ticks.
  16 health. Drops 1–2 cobblestone and flint 20 %.

### Lot 9 — Stumpling

Surface (grass, plains and forests) at night in the dark. 24 health, 3 damage, 1.2 blocks/s,
cooldown 20 ticks. Axes deal ×1.5. Flees fire within 6 blocks. At dawn it sinks into the ground
(despawns, 2 s sinking) leaving nothing. Drops 2–4 sticks, charcoal 30 %.

### Lot 10 — Drowner

Water at least 3 deep, any time (more at night: double the chance). Swims in 3D in water at
3 blocks/s, 0.8 on land, goes back to water. Attack: grabs (2 damage) and drags the player
down 1 block/s for 40 ticks, during which the player cannot swim up. 18 health. Drops nothing.

### Lot 11 — Cinderling

Caves below y = 20 in the dark. Hovers 2 blocks above the ground, keeps 6 blocks from its
target, throws an ember every 60 ticks: a projectile in an arc (add a tiny glowing `Ember` model
to the generator and a projectile entity; the simplest sound way in this codebase is a mob kind
with no health that the server moves and removes) that deals 1 damage, sets burning 60 ticks,
and sets the flammable block it lands on on fire. Dies in water or rain. 10 health. Drops 1–2
coal, flint 10 %.

### Lot 12 — Gust Owl

Above y = 110 at night. Flies, circles 6–10 blocks above its target, then dives: 2 damage and a
knock back of 8 blocks/s; deaths by falling within 3 s are `ThrownOff(GustOwl)`. Sneaking
halves knock back from any source. 14 health. Drops 1–3 feathers.

### Lot 13 — death drops, balancing, docs

- On death in survival, the whole inventory (and the cursor stack) drops where the player died,
  scattered; it despawns after the usual dropped item lifetime (raise it to 5 min if lower).
- An e2e test dies from each cause it can stage cheaply.
- README features, `docs/ARCHITECTURE.md` (tick systems, random ticks, combat, spawning),
  `CHANGELOG.txt`.

## Decisions

- **Rebase on main `80544cca`** (2026-11-25). Appended after main's variants: `BlockId::Fire`
  96, `Lava` 97, `Obsidian` 98; `ItemId::FlintAndSteel` 146, `Obsidian` 147, `LavaBucket` 148;
  the flint and steel recipe is the last one (85). `ItemId::Flint` (110) and its drop from
  gravel were main's already: lot 3 reuses them, and main's texture. A flint and steel lights
  an unlit campfire as a flint does.
- Main moves a player by its horizontal velocity (the portals' throws) in
  `apply_wished_direction`, keeps it through the air and drops it on landing, in a liquid or in
  flight. The knock back uses that: lot 1's own braking (`apply_horizontal_velocity`) is gone,
  so a knock back in water does not push.
- What main already has that later lots must build on, not redo: mob blows
  (`server/src/mob/combat.rs`: `aimed_mob` casts against the mobs' boxes with `aabb_ray_hit`
  and stops at the block `aim` finds, reach 3 or 5 with the `Spear`; `ItemId::attack_damage`;
  `ServerMob::damage` against `MobKind::health`; sheep drop mutton and wool); food
  (`ItemId::food_health`, eaten by a right click to give health back, refused at full health:
  hunger should change that door); farming (`TilledSoil`, rye, berry bushes, saplings) grown by
  `server/src/world/growth.rs`, a second random-tick system beside lot 3's `world/ticks.rs`.
  The player's aim is `players::blocks::aim` (`raycast` within `AIM_DISTANCE` 20, the bucket's
  `raycast_to_source`, the portal gun's `raycast_far`); `shared/src/world/reach.rs` is about
  which chunks a client is sent, not about the player's reach.
