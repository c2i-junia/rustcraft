# Notes for Claude

## Textures

Before drawing a texture for a new block or item, look for it in the Pixel Perfection resource
pack (XSSheep's, kept up by Nova_Wostra): https://github.com/NovaWostra/Pixel-Perfection-Chorus-Eddit

- Item textures are in `assets/minecraft/textures/item/`, block textures in
  `assets/minecraft/textures/block/`, named like Minecraft's ids: `wooden_pickaxe.png`,
  `oak_planks.png`. A file is fetched from
  `https://raw.githubusercontent.com/NovaWostra/Pixel-Perfection-Chorus-Eddit/main/<path>`.
- Here they go in `data/graphics/base_textures/items/` and `.../blocks/`, named after the
  `ItemId` or `BlockId` variant: `wooden_pickaxe.png` becomes `WoodenPickaxe.png`. An item
  with no texture in `items/` shows the block texture of the same name, so a block item needs
  one only when its icon differs.
- Check that the texture is 16x16, like ours. An animated one is a vertical strip of frames: a
  block texture keeps them all, with its `.mcmeta` written as `<Name>.anim.ron` next to it
  (`frame_time`, `interpolate`); of an item texture we take the first.
- A block Minecraft draws as a model of its own, like the chest, has no block texture: its
  faces are cut from its sheet in `assets/minecraft/textures/entity/` (64x64 for the chest,
  each part upside down since Minecraft 1.15) and grown to 16x16 by repeating their edges.
- A block with a texture per face follows the pack's model in `assets/minecraft/models/block/`:
  north is our front (-Z), then south, west and east are our back, left and right.
- The repository has no license file, only the credits in its `pack.txt`. Say which textures
  came from it in the commit message, and credit the pack in `docs/ressources.txt`.
- Draw a texture only when the pack has none for it.

## Tests and builds

- While iterating, run the targeted tests of what you change (`cargo test -p <crate> <filter>`).
  Run `just test-fast` before a commit, and `just check` once per lot, after the last edit.
  Do not rerun a check without a code change in between.
- `just check` remembers: when the working tree (untracked files included) is the one that last
  passed, it prints `check: already passed for tree <hash>` and returns at once. A failing run
  stores nothing, and `just check --force` skips the memo. The test suite of `just check` and
  `just test-fast` runs under a lock shared by all worktrees, so a second run waits for the
  first instead of competing with it.
- Run long commands in the background and wait for their completion notification. Never write
  `sleep`, `until` or `pgrep -f` loops to wait: a `pgrep -f` loop matches itself.
- Do not run `cargo clippy` right after `cargo build` expecting reuse: clippy rebuilds (about
  30 s). Run it at the end of a lot.

## Content

Blocks, items, recipes, smelting, fuel, tags, families, aliases and mobs are data, in
`data/content/rustcraft/*.ron`: adding or tuning one is an edit there, with no Rust and no build.
After editing them, run `just check-content` (about 1 s once built) instead of building: it
names the file, the key and the reason of every error. In a running game or server `/reload`
reads them again (a saved file is read by itself). The schema and examples are in
`docs/CONTENT.md`.

## Where things are

- UI goldens: `client/tests/golden/`. `UPDATE_GOLDEN=1` rewrites the ones a test run touches
  (`client/src/ui/golden.rs`).
- Benchmarks: `client/src/world/rendering/bench.rs`, `client/src/e2e/bench.rs` and
  `server/src/world/bench.rs`. Run them in release with `--ignored`.
- Assets are in `data/`, generators in `scripts/`, recipes in `Justfile`, and the architecture
  in `docs/ARCHITECTURE.md`. Never `find /`.

## Running the game

Agents run the game headless only, never on the desktop, through `scripts/headless.sh SECONDS
<command>`: it runs the command under headless gamescope and kills it with everything it started
once SECONDS have passed (exit 124). Never use a bare `timeout` around gamescope: it kills
gamescope and leaves the game running, and the run never ends.

To play at release speed with fast rebuilds, use `just play [client args]` (the `play` profile).
Agents run it through `scripts/headless.sh` like any other command; the owner runs it directly.

`just hot` (hot patching under dx, `docs/HOT_RELOAD.md`) is the owner's tool; agents run it only
headless, as `scripts/headless.sh 240 just hot --interactive=false`.

### Driving the game and seeing it (BRP)

`just brp [SECONDS] [client args]` (in the background) runs the client headless for SECONDS
(240 by default) with the Bevy Remote Protocol, a JSON-RPC server on http://127.0.0.1:15702
(the `brp` feature, `client/src/brp.rs`). It plays in `appdata/client-1`; for a world of your
own, run `scripts/headless.sh 240 cargo run -p client --features brp -- --game-folder-path
<scratch folder> --assets-folder-path $PWD/data --player-name PlayerOne` instead.
`RUSTCRAFT_BRP_PORT=15703` (default 15702) moves the server, for two games at once. Then:

```sh
brp() { curl -s --max-time 10 http://127.0.0.1:15702 -H 'content-type: application/json' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-null}}"; echo; }
brp rpc.discover                                            # the methods
brp rustcraft/screenshot '{"path":"/abs/shot.png"}'          # written a few frames later
brp world.query '{"data":{"components":["bevy_ui::widget::text::Text"]}}'   # the texts shown
brp world.get_components '{"entity":ID,"components":["bevy_ui::ui_transform::UiGlobalTransform"]}'
brp world.query '{"data":{"components":["bevy_transform::components::transform::Transform"]},"filter":{"with":["bevy_camera::camera::Camera"]}}'
W=$(brp world.query '{"data":{},"filter":{"with":["bevy_window::window::PrimaryWindow"]}}' | grep -o '"entity":[0-9]*' | cut -d: -f2)
```

Only reflected types can be read (the Bevy ones, not most of the game's). The last six numbers
of a `UiGlobalTransform` are an affine matrix: its last two are the centre of the node, in the
window's pixels (1280x720). Input goes in as messages, so the game reacts as to a real one:

```sh
# a click at (640, 300): the cursor moves there, then the left button goes down and up
for m in "{\"CursorMoved\":{\"window\":$W,\"position\":[640.0,300.0],\"delta\":null}}" \
         "{\"MouseButtonInput\":{\"button\":\"Left\",\"state\":\"Pressed\",\"window\":$W}}" \
         "{\"MouseButtonInput\":{\"button\":\"Left\",\"state\":\"Released\",\"window\":$W}}"; do
  brp world.write_message "{\"message\":\"bevy_window::event::WindowEvent\",\"value\":$m}"; done
# a key: Pressed, then Released (in between, it is held: W walks forward)
brp world.write_message "{\"message\":\"bevy_input::keyboard::KeyboardInput\",\"value\":{\"key_code\":\"Enter\",\"logical_key\":\"Enter\",\"state\":\"Pressed\",\"text\":null,\"repeat\":false,\"window\":$W}}"
```

A letter key is `"key_code":"KeyW","logical_key":{"Character":"w"},"text":"w"`. To enter a new
world from the home menu: click Singleplayer (640, 300), click New world (1188, 45), Enter
(creates it and focuses its card), Enter (plays it).
