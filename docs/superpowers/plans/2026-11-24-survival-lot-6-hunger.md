# Survival Lot 6 (Hunger) Implementation Plan

**Goal:** A survival player gets hungry. Sprinting, jumping, swimming, breaking blocks, blows,
hits taken and the health it regenerates tire it (exhaustion); every 4 points of exhaustion
take a point of saturation, or of food once the saturation is gone. Health only comes back
with 18 food or more; with none, the player starves, 1 damage every 80 ticks, to death. At 6
food or less it cannot sprint. Food is eaten as main's (a right click) and gives food and
saturation, refused at full food. Ten drumsticks show the food over the right of the hotbar,
the bubbles above them. To see: sprint and jump for a long while, or join with a hungry
player file.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 6".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 30 (`PlayerUpdateEvent` gains `food`). `FORMAT_VERSION` unchanged: the
  new `Player` fields are `#[serde(default)]`, an old player file loads fed.
- `timeout 600 cargo test -p <crate>` while working, `timeout 1200 just check` at the end.

## Food values

`ItemId::food_health() -> Option<u32>` becomes `ItemId::food_value() -> Option<(u8, f32)>`:
(food points, saturation). Minecraft's values where Minecraft has the food; saturation is food
× 2 × Minecraft's modifier (0.1 poor, 0.3 low, 0.6 normal, 0.8 good).

| Item | Food | Saturation | Note |
|---|---|---|---|
| `Berries` | 2 | 0.4 | Minecraft's sweet berries (main: 1 health) |
| `RoastMushroom` | 4 | 4.8 | spec's 4, normal modifier (main: 2) |
| `RyeBread` | 5 | 6.0 | Minecraft's bread (main: 4) |
| `RawMutton` | 2 | 1.2 | |
| `RoastMutton` | 6 | 9.6 | |
| `RawPork` | 3 | 1.8 | |
| `RoastPork` | 8 | 12.8 | |
| `RawBeef` | 3 | 1.8 | |
| `RoastBeef` | 8 | 12.8 | |
| `RawChicken` | 2 | 1.2 | |
| `RoastChicken` | 6 | 7.2 | |
| `RawRabbit` | 3 | 1.8 | |
| `RoastRabbit` | 5 | 6.0 | |
| `Carrot` (lot 7) | 3 | 3.6 | not an item yet |
| `Potato` (lot 7) | 1 | 0.6 | not an item yet |
| `BakedPotato` (lot 7) | 5 | 6.0 | not an item yet |

## Decisions (where the spec is silent)

1. **Where things live.** `shared/src/players/hunger.rs`: `MAX_FOOD` (20),
   `RESPAWN_SATURATION` (5), `EXHAUSTION_WRAP` (4), `SPRINT_MIN_FOOD` (7: sprinting needs more
   than 6), `REGENERATION_MIN_FOOD` (18), `STARVATION_DAMAGE` (1), the exhaustion of each
   effort, `Player::exhaust(amount)`, `Player::may_sprint()`, `Player::metabolise()`,
   `Player::eat(food, saturation)`, and the HUD's pure rule `drumstick(food, index)`.
   `Player.food: u8`, `saturation: f32` and `exhaustion: f32` are saved, with serde defaults
   (20, 5, 0).
2. **Exhaustion is the server's.** `simulate_player_movement` measures it in its
   `MovementOutcome.exhaustion` (sprint 0.1 per horizontal block on foot, 0.01 per block moved
   in a liquid, a jump 0.05 or 0.2 sprinting; a flight costs nothing); `simulate_player_actions`
   adds 0.005 per broken block into `ActionsOutcome.exhaustion`. The client runs both and drops
   it; the server's input loop passes it to `Player::exhaust`. `Player::hurt` adds 0.1 when it
   takes health, the blow path in `server/src/mob/combat.rs` 0.1 when a blow lands, and
   `metabolise` 6 per point regenerated. `exhaust` does nothing in creative or dead; it wraps
   as many times as the amount holds 4.
3. **Food travels, exhaustion and saturation do not.** `PlayerUpdateEvent.food` sets the
   client's `Player.food` before its reconciliation replays inputs, so the client refuses the
   sprint at ≤ 6 as the server does (`update_stance` reads `may_sprint`). The client's own
   prediction of an eating changes its food until the server's next update.
4. **One clock for regeneration and starvation.** `regenerate_health_system` becomes
   `metabolism_system`: every `REGENERATION_TICKS` (80) it calls `metabolise` on each player:
   a survival player with ≥ 18 food and missing health gets a point back (and 6 exhaustion); a
   survival player with 0 food takes 1 `Starvation` damage, which can kill (pierces the
   invulnerability, lot 1). Creative keeps main's regeneration, without food.
5. **Eating.** `eat` asks `food_value`, refuses at `MAX_FOOD` (`Refusal::NotHungry`, renamed
   from `NotHurt`), adds the food (capped at 20) and the saturation (capped at the new food).
   The blow path's "food in hand does not shear" test reads `food_value`.
6. **Respawn**: food 20, saturation 5, exhaustion 0.
7. **HUD.** `client/src/ui/hud/food.rs` spawns `FoodBar`, a row as a child of the hotbar, at
   its right, `bottom: 100%`, filled from the right (`RowReverse`): each drumstick an
   `food_empty.png` container under a full or half fill, as the hearts. Shown when the hearts
   are (survival, inventory closed). Label `Food: 7.5 of 10 drumsticks`. `AirBar` sits a
   `FOOD_BAND` higher (its bottom margin).
8. **Textures.** Pixel Perfection's `gui/sprites/hud/food_{full,half,empty}.png` (9x9) go to
   `data/graphics/hud/`, credited in `docs/ressources.txt`.

## Hooks for later lots

- Any effort: `player.exhaust(amount)` on the server (constants in `players::hunger`).
- A new food: an arm in `ItemId::food_value`.

## Tasks

### Task 1: Hunger in shared
- [x] Tests (`hunger.rs`): exhaustion wraps on saturation then food; creative and dead get no
  exhaustion; regeneration only at ≥ 18 and costs exhaustion; starvation every metabolism at 0,
  to death; respawn values; an old player file loads fed; `may_sprint`; drumsticks.
- [x] Tests (`movement.rs`): sprint refused at food 6, allowed at 7 and in creative at 0; a
  sprint, a jump, a swim measure their exhaustion; walking costs nothing.
- [x] Tests (`blocks.rs`, `items.rs`): eating gives food and saturation, refused at full food;
  the food table.
- [x] The code.

### Task 2: Server
- [x] Tests: the metabolism system heals a fed player and starves a starving one; the inputs of
  a sprint make a server player hungry.
- [x] The system, `food` in `PlayerUpdateEvent`, exhaustion from the inputs, the blows.

### Task 3: Client
- [x] Test: an update with food 6 makes the replayed prediction refuse the sprint.
- [x] Unit test: the label of the drumsticks.
- [x] e2e (`survival.rs`): a hungry player cannot sprint, eats a roast beef, its drumsticks
  fill and it sprints again.
- [x] HUD row, textures and credit, air moved up, goldens, `docs/ARCHITECTURE.md`.

## Notes

- `load_player_data` copies the saved fields one by one onto a new player: the e2e test found
  the hunger fields missing there, and the server's load test now covers them.
- The e2e test waits for the food and the inventory together: the player's food was seen at 12
  a frame before the `Inventory` resource showed the beef gone.
