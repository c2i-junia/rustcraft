# Iteration speed: implementation plan

Implements the three phases of the iteration-speed study (French Claude Doc
https://claude.ai/artifact/6T3PoLhBMQX8HTPR3MQs7P). The spec for the content-registry tasks
(R0-R9) is `docs/design/content-registry.md`: read the section a task names before starting it.

Branch `feat/iteration-speed`, worktree `/home/user/Code/rustcraft/feat-iteration-speed`,
from `main` 2d1ea403.

## Global constraints

- Work only in `/home/user/Code/rustcraft/feat-iteration-speed`. Never touch `main/` or another
  worktree, never push, never merge. The user's `appdata/` folders are not yours.
- One or more commits per task, in the repository's style: `type(scope): a sentence that says
  what the change does` (see `git log`). No `Co-Authored-By` trailer, ever.
- Code, comments and docs in English, in the repository's style: doc comments in full
  sentences that say what a thing is for.
- The machine is shared with other sessions (load 20-140 happens). Never write `sleep`,
  `until` or `pgrep -f` loops to wait for a command: run long commands with
  `run_in_background` and wait for their completion notification. Timing-based tests can fail
  under load: rerun a failing test alone once before concluding it is broken.
- Tests: while iterating, run the targeted tests of what you change
  (`cargo test -p <crate> <filter>`). Before committing, `just test-fast`. Once at the end of
  the task, after the last edit, `just check` (fmt + clippy + the whole suite, about 4-6 min;
  run it in the background). Do not run checks again without a change in between.
  Clippy on `main` may already report errors in files you did not touch: note them, do not fix
  them.
- The 38 UI goldens (`client/tests/golden/`) must stay byte-identical unless the task says
  otherwise. Never regenerate a golden to make a test pass without saying why in the report.
- The game must keep working in solo, in multiplayer and in the release build
  (`cargo build --release --no-default-features`).
- Never open a game window on the user's desktop. To run the game, use
  `gamescope --backend headless -W 1280 -H 720 -- env -u WAYLAND_DISPLAY <command>`.
- Keep `PROTOCOL_ID` (`crates/protocol/src/lib.rs`) and `FORMAT_VERSION`
  (`server/src/world/format.rs`) bumps to the tasks that say so.

---

## Phase 1

### Task W1: workflow notes for agents in CLAUDE.md

Add to `CLAUDE.md` (keep the existing Textures section first) short sections:

- **Tests and builds.** Targeted tests while iterating; `just test-fast` before a commit;
  `just check` once per lot, after the last edit. Long commands run in the background and the
  agent waits for the completion notification: no `sleep`/`until`/`pgrep -f` loops (a
  `pgrep -f` loop matches itself). Do not run `cargo clippy` right after `cargo build`
  expecting reuse (clippy rebuilds, about 30 s); run it at the end of a lot. Do not rerun a
  check without a code change.
- **Where things are.** UI goldens in `client/tests/golden/` (`UPDATE_GOLDEN=1` rewrites the
  ones a test run touches, `client/src/ui/golden.rs`); benchmarks in
  `client/src/world/rendering/bench.rs`, `client/src/e2e/bench.rs`, `server/src/world/bench.rs`
  (run in release with `--ignored`); assets in `data/`; generators in `scripts/`; recipes in
  `Justfile`; architecture in `docs/ARCHITECTURE.md`. Never `find /`.
- **Running the game.** Headless only, under gamescope (command above).

Acceptance: the file reads well, stays short (under about 80 lines added), and later tasks
(B1, H1, T1, R9) will append their own lines.

### Task W2: a `just check` that remembers, and one heavy test run at a time

1. `scripts/check.sh` runs what `just check` runs today (fmt check, clippy `-D warnings`,
   `cargo test --workspace`). Before running, it computes the tree of the working directory
   including untracked, non-ignored files (`GIT_INDEX_FILE=<temp> git add -A` then
   `git write-tree`, never touching the real index). If that tree equals the one stored after
   the last successful check (`target/.check-passed`), it prints
   `check: already passed for tree <hash>` and exits 0. On success it stores the tree.
   `just check --force` (or `FORCE=1`) skips the memo.
2. The heavy part (`cargo test --workspace`, and `just test-fast`) runs under
   `flock "${XDG_RUNTIME_DIR:-/tmp}/rustcraft-tests.lock"`, so that two worktrees never run
   their suites at once; while waiting it prints one line saying it waits for another run.
3. `Justfile`: `check` and `test-fast` call the script. Note both behaviours in `CLAUDE.md`.

Acceptance: running `just check` twice with no change in between, the second returns at once;
touching a file makes it run again; a failing run stores nothing.

### Task B1: a `play` profile to play at release speed

The study measured: a release-like profile with `incremental = true` and `codegen-units = 256`
rebuilds in 21-52 s (release: 72-188 s) and runs worldgen and meshing at release speed.

1. `Cargo.toml`: `[profile.play]` inheriting `release`, `incremental = true`,
   `codegen-units = 256`, with a comment saying what it is for and the measured figures.
2. `Justfile` recipe `play` that runs the client in that profile with default features, with
   the arguments of `run-release.sh` (`--game-folder-path $PWD/appdata/client-1
   --assets-folder-path $PWD/data`, after `just create-game-folders`), extra recipe arguments
   passed through.
3. `CLAUDE.md`: one line under Running the game.

Acceptance: `cargo build --profile play` builds; the recipe's command line is right (run it
once headless for a few seconds, or `--help`).

### Task A1: assets reload while the game runs (dev feature)

1. Client `dev` feature: also turn on Bevy's `file_watcher` and `embedded_watcher`. The
   release build (`--no-default-features`) must not watch.
2. The 10 shaders (`client/src/**/*.wgsl`): today loaded with `load_internal_asset!` (and some
   with `include_str!`, e.g. `world/precipitation`, `world/lightning`, `world/rendering/
   overcast.rs`). Load them so that a saved `.wgsl` reloads in a running dev game: Bevy's
   `embedded_asset!` + `embedded_watcher` (handles by `embedded://` path) or another Bevy
   mechanism; keep the release binary self-contained (shaders embedded).
3. The e2e harness (`client/src/e2e/harness.rs`), the golden tests and the benches must not
   start file watchers (hundreds of test apps): set the `AssetPlugin` watch override off there.
4. Sounds, music and `.glb` models already load through the AssetServer: check that a changed
   file reloads (or note precisely what does not follow, e.g. scenes already spawned).

Acceptance: goldens unchanged; release builds; a manual headless run shows Bevy reloading a
touched `.wgsl` (quote the log line in the report).

### Task R0: block and item textures reload while the game runs

Spec: `docs/design/content-registry.md` section 4.2 step 5 and lot 0 in section 5.

On `AssetEvent::<Image>::Modified` for a texture of `AtlasHandles` (`client/src/world/
rendering/materials.rs`), rebuild the block texture array, its table, `BlockLooks` and the
item atlas, update the `ChunkMaterial` assets in place (chunk entities keep their handles), and
remesh with the existing `respawn_every_group` (`client/src/world/rendering/render.rs`).
Extract "rebuild the textures" as a function the registry reload (R2) will call.

Acceptance: a test drives a Bevy app (or the systems) through a modified image and checks the
rebuild happened; goldens unchanged; manual headless check described in the report.

### Task H1: hot patching for the human (merge the prototype)

Cherry-pick the four commits of branch `study/hotpatch` (05c9cad2, 92ce87a0, 014f28f5,
c6dfa163: opt-in `hot` feature, `client/src/hot.rs` menu re-enter and stale-system warning,
`just hot`, `docs/HOT_RELOAD.md`). Then extend the re-enter to the in-game screens: after a
patch in `GameState::Game`, rebuild the HUD (spawned on `OnEnter(GameState::Game)`) and the
pause screens without leaving the world, if that can be done safely (despawn the HUD roots and
run their setup systems again); if it cannot be done safely, document why in
`docs/HOT_RELOAD.md` instead. `CLAUDE.md`: one line on `just hot` (human tool; agents run it
headless only).

Acceptance: the default build and the release build are unchanged in behaviour; `cargo build
-p client --features hot --no-default-features` builds; the report says whether a patch was
exercised (dx is installed: `~/.cargo/bin/dx`, 0.7.10).

---

## Phase 2

### Task R1: one property table, generated from today's match arms

Spec: `docs/design/content-registry.md` sections 1.1-1.2 and lot 1 in section 5.

`shared/src/content/`: `Content`, `BlockProps`, `ItemProps`, `content()`, `install()` (the
process-global snapshot, old snapshots leaked). `Content::builtin()` fills the table by calling
today's property functions (renamed `builtin_*`); the public getters of `impl BlockId` /
`impl ItemId` read the table. No behaviour change, no protocol change. `BlockLooks::get` made
bounds-safe.

Acceptance: whole suite green, goldens unchanged. Benchmarks before and after, in release,
3 runs each, reported: `server` `bench_generation` and `client` `bench_meshing`
(commands in the bench files' headers). No regression beyond run-to-run noise.

### Task R2: override files and live reload of block and item properties

Spec: `docs/design/content-registry.md` sections 2.1-2.2, 4.1-4.4 and lot 2 in section 5.

`data/content/rustcraft/*.ron` may override pure-data fields of existing blocks and items by
name. Server: a `notify` watcher (debounced) and a `/reload` command; validation of the whole
set, all-or-nothing install, errors to the log and to the players' chat. A
`ServerToClientMessage::Content` (snapshot) sent at join and on reload; client install
(remote) and selective rebuild (R0's texture rebuild + remesh only when looks change).
`ContentGeneration` resource for client caches. Bump `PROTOCOL_ID` once. Pure
`Content::load(dir) -> Result<Content, ...>` for tests; tests never `install()` except one
serialised test of the install path.

Acceptance: a test loads an override file and sees the property change; a test of a bad file
(error names file and key, nothing installed); goldens unchanged; a manual headless check
(change a block's light or hardness in a file, see it applied) described in the report.

### Task A2: the language files reload

`client/src/i18n.rs` embeds `data/lang/*.ron` with `include_str!` in a `OnceLock`. Read them
from the assets folder at start-up instead (tests: from `CARGO_MANIFEST_DIR`), and in dev reload
them when they change, so that a changed text shows without recompiling (screens rebuilt by the
same re-enter as U1/H1, or on next spawn). The release build must still find its texts
(release folder has `data/`; check `just generate-release-folder` and `scripts/generate_release.py`).

Acceptance: tests and goldens unchanged; a test of reload on a changed table.

### Task U1: the UI theme from a file

`client/src/ui/theme.rs` holds 36 constants used in about 88 files. Move the values to
`data/ui/theme.ron` (same values: goldens must stay identical), read at start-up into a
process-global snapshot with accessor functions (constants used in other constants become
functions too), reloaded in dev on change, with the current menu screen re-entered after a
change (reuse the re-enter of `client/src/hot.rs`, made available without the `hot` feature).
Embedded default if the file is missing (tests, release without data).

Acceptance: goldens identical; a test of the reload; manual headless check of a colour change.

### Task T1: agents can drive and see the game (Bevy Remote Protocol)

Behind an opt-in client feature `brp`: `bevy/bevy_remote` + `bevy_brp_extras` 0.22.10 (the
version built for Bevy 0.19.1; 0.23 needs Bevy 0.20). A `just brp` recipe starting the game
headless under gamescope with the remote plugin; `.mcp.json` (project scope) entry for
`bevy_brp_mcp` if a version compatible with bevy_brp_extras 0.22 exists (install it with
`cargo install --locked`), else document the raw JSON-RPC calls (curl). Exercise it: list
entities, read a component, send a key, take a screenshot; put the commands in `CLAUDE.md`.

Acceptance: default and release builds unaffected; report shows the calls working.

---

## Phase 3

Spec for all: `docs/design/content-registry.md` section 5 (lots 3-9) and the sections it
references. Each task keeps the suite green and the goldens identical unless it says otherwise.

### Task R3: the content files become the source of block and item data

Lot 3: a one-shot dump writes the full files from `builtin()`; a test asserts files ==
builtin; then the match arms are deleted (`blocks.rs`, `items.rs`); `Behaviour` enum and flags;
face textures / tint / sway (`client/src/world/rendering/voxel.rs`) move to `look`.

### Task R4: `BlockId(u16)` and per-world name lists

Lot 4: `BlockId(u16)` with `engine_blocks!` associated consts spelled as today's variants;
`index()`; names in RON serde; world name list + chunk palette remap on load/save; legacy list
of today's 280 names; join name check; `FORMAT_VERSION` 6; `PROTOCOL_ID` +1. Round-trip
property test; a v5 fixture world loads.

### Task R5: `ItemId(u16)` and implicit items

Lot 5: same treatment for items; implicit item per block; container / furnace file remap;
item browser and i18n by table; remove the hard-coded item counts (`/ 361`). `PROTOCOL_ID` +1.
Acceptance adds: a new placeable block defined only in a `.ron` file and its PNG, no Rust,
appears in a running game after a reload (test + manual check).

### Task R6: recipes, smelting, fuel and tags in data

Lot 6.

### Task R7: families (the palette)

Lot 7: template expansion; convert `palette.rs` and `palette_recipes`; shrink the engine lists
to the names Rust really uses.

### Task R8: mob data

Lot 8: stats, hunt, strike, drops, model, clips, sounds in `mobs.ron`; `MobKind` stays.

### Task R9: content docs and a content checker

Lot 9: `docs/CONTENT.md` (schema + examples), a `--check-content` CLI that validates the files
without starting a world, error messages with file and key, a `CLAUDE.md` note.
