# Survival Lot 1 (One door for damage, and feedback) Implementation Plan

**Goal:** Every hurt goes through `Player::hurt(amount, DamageSource)`, which applies the game
mode, 10 ticks of invulnerability (pierced by the void and starvation) and the cause of a death,
a deadly fall within 3 s of a mob's hit included (`ThrownOff`). `Player::knock_back` pushes a
player through the movement state the client reconciles with. The client shows a hurt: a red
vignette, a camera tilt, the hurt sound, blinking hearts. To see: fly 8 blocks up in creative,
`/gamemode survival`, land.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 1".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `DeathCause` and `MobKind` are append only; `PROTOCOL_ID` 29 → 30 (`DeathCause` gains
  variants and travels in `PlayerUpdateEvent`). `FORMAT_VERSION` unchanged: the new `Player`
  fields are not saved.
- `timeout 600 cargo test -p <crate>` while working, `timeout 1200 just check` at the end.

## Decisions (where the spec is silent)

1. **Where things live.** `DamageSource`, `INVULNERABILITY_TICKS` (10), `THROWN_OFF_TICKS` (60)
   and `DeathCause::of(source, thrown_by)` in `shared/src/players/game_mode.rs`, next to
   `DeathCause`. `MobKind::display_name` in `shared/src/world/mobs.rs`.
2. **Timers count down, they are not stamped.** `Player.invulnerable_ticks: u32` and
   `Player.last_mob_hit: Option<(MobKind, u32)>` (the mob and the ticks left in the
   `ThrownOff` window), both `#[serde(skip)]`. A server system,
   `tick_damage_timers_system`, runs first in `ServerSet::Simulate` and calls
   `Player::tick_damage_timers`. Stamps of `ServerTime` would break under `/time set`, which
   moves that clock.
3. **`hurt` returns whether the hit landed** (for the exhaustion of lot 6). A hit by a mob
   records the mob even when the invulnerability swallows it, as its knock back still
   applies. A piercing hit restarts the invulnerability. Zero damage, a dead player or
   creative: nothing happens.
4. **The cause.** `Fall` becomes `ThrownOff(kind)` while `last_mob_hit` is set; the other
   sources map one to one. Messages: `Mob` "A Fox got you", `ThrownOff` "A Fox threw you off a
   ledge". `DeathCause::message` returns a `String`.
5. **Knock back.** `knock_back(direction, strength)` sets the horizontal velocity to
   `strength` along the horizontal part of `direction` and the vertical one to at least
   `KNOCK_BACK_HOP` (4 blocks/s, a quarter of a block). The movement simulation now moves the
   player by its horizontal velocity too, axis by axis against the blocks (a blocked axis loses
   its speed), then brakes it: 16 blocks/s² on the ground, 2 in the air or water. No `exp`:
   both sides compute the same. The velocity is part of `MovementState`, so the server's
   update, which differs from the prediction, makes the client resume from it.
6. **Client feedback** (`client/src/player/hurt.rs`): `detect_local_hurt` compares the local
   player's health with the frame before (a respawn or a mode change is not a hurt) and writes
   `LocalPlayerHurt`. It starts `HurtFeedback` (0.5 s). The vignette is a full screen node
   with a `RadialGradient`, transparent in the middle, red at the edges, its alpha fading out.
   The tilt rolls the camera by up to 8° (decaying), added in `PostUpdate` before the transform
   propagation and taken off in `First`, so the inputs and the aiming never see it. The audio
   plays `Sound::Hurt` (`player/hurt.ogg`). The hearts blink: the row turns hidden every other
   0.1 s while the feedback runs.

## Tasks

### Task 1: Damage in shared
- [x] Tests (`game_mode.rs`, `data.rs`): `hurt` takes the amount and returns true; creative,
  dead and zero take nothing; a second hit within 10 ticks is ignored, after 10 it lands; Void
  and Starvation pierce; the cause of each source; a fall within 60 ticks of a mob hit is
  `ThrownOff`, after it `Fall`; `respawn` clears the timers; messages and display names;
  pinned indices of the new `DeathCause` variants.
- [x] `DamageSource`, `DeathCause` variants, `of`, `message`, `MobKind::display_name`,
  `hurt`, `tick_damage_timers`, `PROTOCOL_ID` 30.

### Task 2: Knock back in shared
- [x] Tests (`movement.rs`): a knocked back player slides away and stops; a wall stops it; a
  hop lifts it.
- [x] `knock_back`, `KNOCK_BACK_HOP`, drags, `apply_horizontal_velocity`.

### Task 3: Server
- [x] Tests: the fall and void go through `hurt`; `tick_damage_timers_system` counts down.
- [x] Callers, the system in the schedule.

### Task 4: Reconciliation keeps a knock back
- [x] Test (`client/src/player/update.rs`): the server knocks the player back after the input
  it acknowledges; the client resumes from it, its replayed inputs carry the push, and the
  next update matches its prediction.

### Task 5: Client feedback
- [x] Unit tests: a drop of health writes `LocalPlayerHurt`, a respawn does not; the vignette
  alpha and tilt fade to 0 over 0.5 s.
- [x] e2e (`survival.rs`): a fall of 8 blocks hurts and shows the vignette, which goes again.
- [x] Module, sound, hearts blink, vignette, tilt.
