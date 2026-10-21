# Weather Lot 1 (The weather state, shared and server) Implementation Plan

**Goal:** The server holds a weather (clear, rain, thunderstorm) that changes on its own, keeps
it in the save, tells every client, takes `/weather <clear|rain|thunder>`, and draws lightning
strikes during thunderstorms. The client eases its rain and thunder levels towards the weather
it is told and shows them in the debug overlay. Nothing is drawn yet (lots 2 to 5). To see:
F3, `/weather thunder`, the `Weather:` line climbs to 1.00 in 20 s.

**Spec:** `docs/superpowers/specs/2026-10-21-weather-design.md`, "Lot 1 — the weather state,
shared and server", and its "Decisions".

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- The messages are append only; `PROTOCOL_ID` goes from 24 to 25 once, with the first change
  of the wire, for the whole branch.
- `FORMAT_VERSION` stays 5: a `world.ron` without `weather` starts clear.
- Random draws go through pure helpers taking `rng: &mut impl Rng`, tested with a seeded
  `StdRng`; the systems pass `rand::thread_rng()`.
- `timeout 600 cargo test -p <crate>` while working; `timeout 900 just check` (fmt, clippy
  `-D warnings`, every test) at the end.

## Decisions (where the spec is silent)

1. **Where things live.** `shared/src/world/weather.rs` holds `WeatherKind` and the cycle
   (`spell_ticks`, `next_kind`), the model both sides agree on. The server holds
   `ServerWeather` (a resource) and the lightning in `server/src/world/weather.rs`; the client
   holds `ClientWeather` in `client/src/world/weather.rs`.
2. **Lightning timer.** `ServerWeather` keeps the ticks to the next strike (`strike_in`), drawn
   (3–15 s) whenever a thunderstorm starts, set by the cycle, by `/weather` or by a load, and
   again after each strike. It is not saved: a storm loaded waits a fresh draw.
3. **Strike column.** A player drawn among the players, a direction drawn uniformly, a
   distance drawn uniformly in 16–160 blocks. The struck block is the highest block of that
   column from 128 blocks above the player down to 128 below (the vertical reach of a render
   distance); a column with no block there (not loaded, or empty) gets no strike.
4. **Tick order.** The weather runs in `ServerSet::Simulate`, after the water: it counts down,
   changes and tells the clients (`WeatherSet`), then a strike falls if one is due
   (`LightningStrike`). Both go to the authenticated clients only, like `TimeSet`.
5. **`/weather` of the weather already there** draws a new duration for it but sends nothing:
   the clients have it.
6. **Client levels.** Each level follows a ramp that moves linearly at 1/20 per second towards
   its target; the level is the smoothstep of its ramp. So a full change takes 20 s and starts
   and stops gently, and a change that turns back midway does so without a jump.
7. **Overlay line.** `Weather: <kind> <rain>`, with ` thunder <thunder>` added while the
   thunder level is above 0, levels with two decimals: `Weather: rain 0.85`.
8. **Leaving a world** resets `ClientWeather` to clear (`clear_resources`), as the chunks are
   forgotten.

## Tasks

### Task 1: The weather model (`shared/src/world/weather.rs`)
- [ ] Tests: `WeatherKind` names and `from_name` round trip, any case; the variants are pinned
  (0, 1, 2); `spell_ticks` stays within 12 000–120 000 for clear and 6 000–18 000 for rain and
  thunder, and spreads over its range; after clear comes rain about 2 in 3 and thunder about 1
  in 3; after rain or thunder comes clear. See them fail.
- [ ] `WeatherKind` (`Default` clear), `ALL`, `name`, `from_name`, `CLEAR_TICKS`,
  `STORM_TICKS`, `spell_ticks`, `next_kind`. See them pass. Commit `feat(shared): the
  weather and its cycle`.

### Task 2: The server holds the weather (`server/src/world/weather.rs`)
- [ ] Tests: a new weather is clear for a clear spell; `tick` counts down and returns nothing
  until 0, then changes to the next kind with a spell of that kind; `set` puts a kind for a
  natural spell; `from_saved` keeps a saved weather and starts clear without one; `saved`
  gives the kind and ticks left. See them fail.
- [ ] `ServerWeather`, `SavedWeather` (in `format.rs`), `new`, `from_saved`, `set`, `tick`,
  `saved`; the resource is inserted by `init` (new world: clear). See them pass. Commit
  `feat(server): the server holds the weather`.

### Task 3: The weather on the wire (`shared`, `server`, `client` compiles)
- [ ] Tests: `WeatherSet` (11) and `LightningStrike` (12) pinned, `AuthRegisterResponse`
  built with `weather`; `WeatherKind` pinned with the network enums. The weather system: a
  change is sent to every authenticated client (local renet client), no change sends
  nothing. See them fail.
- [ ] The variants, the field, `PROTOCOL_ID` 25; `weather_system` in `ServerSet::Simulate`;
  the auth response carries the weather; the client ignores the two messages for now. See
  them pass. Commit `feat: the weather is sent to the clients`.

### Task 4: Lightning (`server`)
- [ ] Tests: no strike outside a thunderstorm; in a storm the timer counts down and a strike
  is due when it reaches 0, then waits 3–15 s again (seeded); `struck_block` finds the top
  block of a column, `None` for an empty or unloaded one; `draw_strike` lands 16–160 blocks
  from the player on the top block, `None` without players. See them fail.
- [ ] `strike_due`, `struck_block`, `draw_strike`; the weather system sends
  `LightningStrike` to every authenticated client. See them pass. Commit `feat(server):
  lightning strikes during thunderstorms`.

### Task 5: `/weather` (`shared`, `server`)
- [ ] Tests: `parse_command` reads `weather clear|rain|thunder`, the words as written like
  `/time`'s, else the usage; `run_command` sets the weather and answers "Weather set to
  rain"; the usages test counts 4 commands; the chat suggests `weather` and its kinds
  (`a_slash_alone_suggests_every_command_with_its_arguments`). Session test:
  `weather_set_changes_every_client_and_answers_its_author_only` (a client joining after gets
  the weather in its auth response). See them fail.
- [ ] `COMMANDS` entry, `Command::SetWeather`, `WEATHER_USAGE`; the handler sends
  `WeatherSet` to every authenticated client when the kind changed. See them pass. Commit
  `feat: /weather sets the weather`.

### Task 6: The weather is saved (`server`)
- [ ] Tests: `WorldMeta` writes and reads back `weather`, a file without it reads `None`;
  `WorldData` from and to `WorldMeta` carries it; a save writes the current weather; the
  session test: a weather set before a restart is there after it. See them fail.
- [ ] The field (`#[serde(default)]`), the `From` impls, `legacy.rs`, `queue_save`,
  `load_world`. See them pass. Commit `feat(server): the weather is saved with the world`.

### Task 7: The client eases the weather (`client`)
- [ ] Tests: `ClientWeather::settled` puts the levels at once; `advance` moves rain towards 1
  for rain and thunder, thunder towards 1 for thunder only, 20 s for a full change, gently
  (the level after 1 s is well under 1/20), back to 0 for clear; a turn back midway is
  continuous; the network applies `WeatherSet` and the auth response settles; the overlay
  line; leaving the world resets it. See them fail.
- [ ] `ClientWeather`, `ease_weather_system`, the network, the overlay line, the reset. See
  them pass. Commit `feat(client): the client eases the rain and thunder levels`.

### Task 8: Closing
- [ ] `docs/ARCHITECTURE.md` (tick, messages, `world.ron`); spec decisions appended; `just
  check`; commit `docs: the weather state (lot 1)`.
