# Hot patching the client (`just hot`)

`just hot` runs the client under the Dioxus CLI (`dx serve --hot-patch`) with Bevy's
`hotpatching` feature. Each time a `.rs` file of the workspace is saved, dx rebuilds the client
and patches the new code into the running game: no restart, the world and the menus stay where
they were.

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
--precise 0.8.9`, `cargo update -p rustversion`, then with the feature declared
`cargo update -p dioxus-devtools --precise 0.7.10`.

dx starts the game from `target/dx/`, not from the repository: the recipe passes the absolute
paths of `data/` and `appdata/client-1`, and `BEVY_ASSET_ROOT` for the fonts and sounds. It
touches `client/src/main.rs` first: dx fails to start an unchanged client ("Missing linker args
for fat link").

Extra arguments go to dx: `just hot --interactive=false` prints a plain log instead of the TUI.

Agents, or anyone not wanting a window on their desktop, can run the game in a headless
compositor: `gamescope --backend headless -W 1280 -H 720 -- env -u WAYLAND_DISPLAY just hot
--interactive=false`.

## What a patch reaches

A patch takes 17-30 s from the save to the new code running (Ryzen 9 5900XT, other builds
running: load 20-30); about 11 s of it is rustc, the rest dx (replaying `shared` and `server`,
which the client embeds, extracting assets, linking).

| Change | Patched? |
| --- | --- |
| Body of a system of the client (`Update`, `FixedUpdate`, observers...) | yes, from its next run |
| Body of a function of `shared` (or `server`) the client calls, even a small one | yes |
| Setup function of a menu screen (`OnEnter(MenuState::X)`) | yes: the screen is rebuilt, see below |
| Setup function of the HUD or of another `OnEnter(GameState::Game)` system | no: it ran already; leave the world and join it again |
| `Startup` systems, plugins (`app.add_systems`, new systems, new resources) | no: restart |
| Parameters of a system (a new `Res`, `Query`...) | no: the system keeps its OLD code, see below |
| Type layouts (fields of a component, a resource, an enum) | no: restart (may crash) |
| Assets (`data/`), shaders | not by dx; Bevy's file watcher is not on |

### Menu screens

A patch re-enters the menu screen shown (`client/src/hot.rs`): `NextState::set` to the same
state runs `OnExit` (which despawns the screen, `DespawnOnExit`) and `OnEnter` (which spawns
it with the patched code). What lived only in the screen's entities is lost: the focus goes
back to the screen's `AutoFocus`, text typed in its fields and scroll positions are gone; what
lives in resources stays. While a dialog is open, the screen waits for it to close: the dialog
is not part of the screen and would be left over the new one. In game, nothing is re-entered.

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
