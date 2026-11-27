# Hot patching the client (`just hot`)

`just hot` runs the client under the Dioxus CLI (`dx serve --hot-patch`) with Bevy's
`hotpatching` feature. Each time a `.rs` file of the workspace is saved, dx rebuilds the client
and patches the new code into the running game: no restart, the world and the menus stay where
they were, and the screen shown is rebuilt with the new code.

## Setup

1. Install the CLI whose version matches the `subsecond` crate in `Cargo.lock` (0.7.10):
   `cargo install dioxus-cli --version 0.7.10 --locked`. dioxus 0.8 alphas do not match Bevy
   0.19.
2. `just hot`. The first run builds Bevy again with the `hotpatching` feature and without
   `dynamic_linking` (14 min on a loaded machine); later runs build only what changed, about
   30 s.

The client's `hot` feature (`client/Cargo.toml`) turns `bevy/hotpatching` on. It is off by
default: `cargo build`, `cargo test` and the releases do not change. Bevy 0.19.1 asks for
dioxus-devtools `0.7.0-alpha.1` and subsecond `0.7.0-rc.0`, which cargo resolves to alphas that
do not build on our nightly; the lock file pins the 0.7.10 crates instead (with libloading
0.8.9 and rustversion 1.0.23, which they need). To redo it: `cargo update -p libloading
--precise 0.8.9`, `cargo update -p rustversion --precise 1.0.23`, then with the feature declared
`cargo update -p dioxus-devtools --precise 0.7.10`. That last step also moves the `windows-sys`
edges of crates the default build uses (rustix, tempfile, socket2, errno...): put their entries
back as they were, so that the lock differs only by the crates hot patching needs.

dx starts the game from `target/dx/`, not from the repository: the recipe passes the absolute
paths of `data/` and `appdata/client-1`, and `BEVY_ASSET_ROOT` for the fonts and sounds. It
touches `client/src/main.rs` first: dx fails to start an unchanged client ("Missing linker args
for fat link").

Extra arguments go to dx: `just hot --interactive=false` prints a plain log instead of the TUI.

`just hot` is a tool for a human at the desktop. Agents never run dx on the desktop: they run it
headless, for a bounded time, through `scripts/headless.sh`, which kills dx, cargo and the game
together once the time has passed (a bare `timeout` around gamescope would leave the game
running):

```
scripts/headless.sh 240 just hot --interactive=false
```

The first build under dx (its `desktop-dev` profile, Bevy built again) takes longer than a
headless run: build it first, without running the game, then run `just hot`, which builds only
the client again (about 20 s):

```
BEVY_ASSET_ROOT=$PWD/client timeout -k 10 1500 dx build --fat-binary \
    -p client --bin client --no-default-features --features hot
```

To patch a running headless game, save a change to a `.rs` file while it runs: dx sees it as it
would on the desktop. Run 2026-11-27 (load 50-70): the fat build took 312 s, `just hot` started
the game 17 s after it was run, and a line added to `home_setup` was patched in 37 s, the main
menu re-entered with it.

## What a patch reaches

A patch takes 17-30 s from the save to the new code running (Ryzen 9 5900XT, other builds
running: load 20-30); about 11 s of it is rustc, the rest dx (replaying `shared` and `server`,
which the client embeds, extracting assets, linking).

| Change | Patched? |
| --- | --- |
| Body of a system of the client (`Update`, `FixedUpdate`, observers...) | yes, from its next run |
| Body of a function of `shared` (or `server`) the client calls, even a small one | yes |
| Setup function of a menu screen (`OnEnter(MenuState::X)`) | yes: the screen is rebuilt, see below |
| Setup function of the HUD (`SpawnHud`) or of the pause menu | yes: they are rebuilt in game, see below |
| Another `OnEnter(GameState::Game)` system (the chat, the clouds, the resets of a new world) | no: it ran already; leave the world and join it again |
| `Startup` systems, plugins (`app.add_systems`, new systems, new resources) | no: restart |
| Parameters of a system (a new `Res`, `Query`...) | no: the system keeps its OLD code, see below |
| Type layouts (fields of a component, a resource, an enum) | no: restart (may crash) |
| Assets (`data/`), shaders | not by dx; Bevy's file watcher is not on |

### Menu screens

A patch re-enters the menu screen shown (`client/src/hot.rs` sends `RebuildScreens`,
`client/src/ui/rebuild.rs` does it; a saved language file does the same): `NextState::set` to the same
state runs `OnExit` (which despawns the screen, `DespawnOnExit`) and `OnEnter` (which spawns
it with the patched code). What lived only in the screen's entities is lost: the focus goes
back to the screen's `AutoFocus`, text typed in its fields and scroll positions are gone; what
lives in resources stays. While a dialog is open, the screen waits for it to close: the dialog
is not part of the screen and would be left over the new one.

### In game

In game, a patch rebuilds the HUD and the pause menu without leaving the world
(`rebuild_in_game_screens` in `client/src/ui/rebuild.rs`). The HUD's setup systems are in a schedule of
their own, `SpawnHud` (`client/src/ui/hud/mod.rs`), which entering a world runs: the reticle, the
panel at the bottom with the hotbar, the inventory and the bars over it, the item browser, the top
right column with the minimap and the FPS counter, and the debug overlays. `respawn_hud`
despawns the roots it spawned last and runs it again; the pause menu, if open, is despawned and
spawned again by `spawn_pause_menu`. What lived only in their entities goes back to how it
starts: the zoom of the minimap, the focus in the pause menu. The slot selected in the hotbar is kept. Their systems that draw only what
changed (the hearts, the drumsticks, the bubbles, the recharge gauge, the effects) remember the
entity they drew in, so that a new HUD is drawn at once.

The rebuild waits until nothing but the pause menu is open over the game: no inventory, chest
or other panel, no settings or progress over the pause menu, no world map, death screen or
dialog, and no text being typed. The inventory and the panels are part of the HUD, and what they
hold (the stack held by the cursor, a search typed) would be lost; the settings and the other
screens are not rebuilt and would be left over the new HUD. Close them and the HUD is rebuilt in
the next frame.

Not rebuilt: the chat, which keeps the lines received (spawned from messages read once, they
could not be shown again), and the other systems of `OnEnter(GameState::Game)`, which also reset
the state of a new world (the trade shown, the recharge, the frame times); leave the world and
join it again for them.

### Changed parameters

Bevy finds a system's new code by the symbol of its `run`, which is generic over the system's
parameters: once they change, the patch has no such symbol, and the system silently keeps the
code the game was started with, old body included. Bevy says nothing. `client/src/hot.rs`
collects the systems Bevy moved to the patched code (it logs `system <name> hotpatched` at the
debug level, which the recipe's `RUST_LOG` keeps), and a few frames after each patch warns of
the systems of the client that run every frame and were not moved:

```
WARN client::hot: hot patch: client::foo::bar still runs its OLD code: its parameters changed? Restart `just hot`
```

Systems of `OnEnter`/`OnExit` and of `First` are not checked.
