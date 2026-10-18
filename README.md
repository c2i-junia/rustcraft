# rustcraft

Minecraft game clone written in Rust, using the Bevy game engine.

<br>

![image](./docs/preview1.png)

![image](./docs/preview2.png)

<br>

# Table of contents

- [Features](#features)
- [Installation](#installation)
    - [Package Managers](#package-managers)
    - [Manual](#manual)
- [Controls](#controls)
- [Saves](#saves)
- [How to Build](#how-to-build)
    - [Prerequisites](#prerequisites)
    - [Running the Project](#running-the-project)
- [Contributing](#contributing)
    - [Guidelines](#guidelines)

<br>

# Features
- Procedurally generated worlds with biomes (Plains, Forest, Mountains, Desert, Ice Plain, Flower plains), trees, cactus, tall grass and flowers.
- Multiplayer functionality.
- Day/night cycle
- Block breaking and placing mechanics.
- Inventory system with a functional hotbar.
- World saving and loading.
- In-game chat.
- Customizable keybindings, and a key pair (O and P) that sets how far away mobs are drawn.

<br>

# Installation

## Manual

You can download the game for **Windows** and **Linux** from the [Releases page](https://github.com/c2i-junia/rustcraft/releases) of the GitHub repository.

Once downloaded, extract the archive for your operating system. The executable will be located in the `/bin` folder of the extracted directory.

## Package Managers

### Arch Linux

There are 2 [AUR](https://aur.archlinux.org) packages available for rustcraft:

- [rustcraft](https://aur.archlinux.org/packages/rustcraft): The latest release built from source
- [rustcraft-bin](https://aur.archlinux.org/packages/rustcraft-bin): The latest release pre-built

You can install them using your preferred [AUR helper](https://wiki.archlinux.org/title/AUR_helpers), e.g. `paru`:
```bash
paru -S rustcraft
```

<br>

# Controls

| **Category**     | **Action**             | **Key(s)**                  |
|-------------------|------------------------|-----------------------------|
| **Movement**      | Jump                  | Space                       |
|                   | Strafe Left           | A / Arrow Left              |
|                   | Strafe Right          | D / Arrow Right             |
|                   | Walk Backward         | S / Arrow Down              |
|                   | Walk Forward          | W / Arrow Up                |
|                   | Toggle Fly Mode       | F                           |
|                   | Fly Up                | Space                       |
|                   | Fly Down              | Left Shift                  |
|                   |                        |                             |
| **Gameplay**      | Destroy Block         | Left Mouse Button           |
|                   | Place Block           | Right Mouse Button          |
|                   |                        |                             |
| **Inventory**     | Open/Close Inventory  | E                           |
|                   | Pick up stack         | Left Click                  |
|                   | Pick up half of stack | Right Click (with empty mouse) |
|                   | Deposit 1 item        | Right Click (over valid stack) |
|                   | Deposit MAX items     | Left Click (over valid stack) |
|                   | Exchange stacks       | Left Click (over a different stack or full valid stack) |
|                   |                        |                             |
| **Miscellaneous** | Open Chat             | T                           |
|                   | Reload Chunks (rebuild their meshes) | R            |
|                   | Toggle FPS Display    | F3                          |
|                   | Toggle Perspective    | F5                          |
|                   | Toggle Chunk Debug    | F4                          |
|                   | Toggle Block Debug    | F6                          |
|                   | Toggle Raycast Debug  | F7                          |
|                   | Decrease Mob Draw Distance | O                      |
|                   | Increase Mob Draw Distance | P                      |
|                   | Exit Game             | Escape                      |

<br>

# Saves

Each world has its own folder under `saves/`: `world.ron` holds the metadata (name, seed, time, mobs), `chunks/` has one file per chunk and `players/` one file per player. The game saves every 60 seconds, when a player leaves and when the game is closed. Worlds saved by older versions are converted on their first load, and the old file is kept as `world.ron.legacy`.

<br>

# How to Build

## Prerequisites

To build and run this project, you need the following tools and dependencies installed:

### 1. **Rust**
- Install Rust using [Rustup](https://rustup.rs)
- The repository pins its nightly toolchain (with the Cranelift backend) in `rust-toolchain.toml`: rustup installs it automatically on the first `cargo` command run in the repository.

### 2. **Just**
- **Just** is used in this project to manage build. Install it using Cargo:
  ```bash
  cargo install just
  ```
> Note: You can also install Just using your system's package manager.

### 3. **Dependencies**

Install the required dependencies based on your operating system:

#### Windows
- **Git Bash** is required to ensure the commands in the `Justfile` and scripts run correctly. Download and install [Git Bash](https://git-scm.com/).
- After installation, make sure Git Bash is added to your system's `PATH`. You can verify it by running:
  ```bash
  bash --version
  ```

#### Arch Linux
```bash
sudo pacman -S base-devel mold clang vulkan-radeon vulkan-tools vulkan-swrast
```
- `vulkan-swrast` is the software renderer the UI screenshot tests draw with.
- Replace `vulkan-radeon` with:   
  - `vulkan-intel` for Intel GPUs.   
  - `nvidia-utils` for NVIDIA GPUs.   

#### Ubuntu/Debian
```bash
sudo apt install -y build-essential mold clang mesa-vulkan-drivers vulkan-tools pkg-config libasound2-dev libudev-dev
```
- For NVIDIA GPUs, also install:
    ```shell
    sudo apt install -y nvidia-driver nvidia-vulkan-icd
    ```

## Running the Project

To compile and run the game locally, use the following commands:

> Note: the first compilation will be slow depending on your hardware, next compilations will be incremental and thus faster.

```sh
# Clone the repository
git clone https://github.com/c2i-junia/rustcraft

# Navigate to the project directory
cd rustcraft
```

Debug mode:
```sh
./run1.sh        # this will compile the project and run the client 

./run-server.sh  # (optional) do this only to test multiplayer features
```

Release mode:
```sh
# Build the project in release mode
just generate-release-folder

# Run the executable
./release/bin/rustcraft         # run the client

./release/bin/rustcraft-server  # (optional) do this only to test multiplayer features
```

<br>

# Contributing

Feel free to submit issues or open pull requests. If you want to know where to help, refer to the existing issues.

[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) explains how the three crates fit together, the order of a server tick and of a client frame, and the save format.

## Guidelines 

### Checks
Run `just check` before committing: it checks the formatting (`cargo fmt`), runs clippy without any warning allowed, and runs the tests. The CI runs the same three steps.

### UI screenshots
On Linux, the tests also draw the UI off screen and compare it with the golden screenshots of `client/tests/golden/`. They draw with lavapipe, the software renderer of Mesa (`vulkan-swrast` on Arch, `mesa-vulkan-drivers` on Ubuntu/Debian), so that every machine draws the same pixels.

- A test without a golden writes it and fails: look at the image, then commit it.
- After a deliberate change of the UI, `UPDATE_GOLDEN=1 cargo test -p client golden` rewrites the goldens: look at their diff before committing them.
- A test that fails writes `<name>.actual.png` and `<name>.diff.png`, with the differing pixels in red, next to the golden. The CI uploads them as the `golden-failures` artifact.

### End-to-end UI tests
The tests in `client/src/e2e/` run the whole client headless, with the same wiring as the game (`app::add_client`), and drive it like a player: they find elements on screen, click, type, and read what is drawn. They need no window, but, like the golden screenshots, they draw with lavapipe (see above) and are ignored on other systems than Linux. Each `Harness` has its own temporary game folder and no fixed network port, so tests run in parallel; in game, they go through the real integrated server.

- **Locators** (`Locator`, imported as `L`) describe what a player sees: `L::text("Singleplayer")` first, as text is what the player reads. For elements without text, use `L::name("Play My world")` (the `Name` of the element) or `L::marker::<Hotbar>()` (a component). `L::row("My world")` is the row of a text, and `locator.within(scope)` restricts a locator to the descendants of another. Hidden elements are never found, and a text made of spans is matched on its whole content.
- **Real input**: `click` and `hover` move the cursor and send the button messages, which the UI systems read as for a mouse; `press`, `key_down` and `key_up` send keys, `type_text` characters, `scroll` the wheel. A test never calls a system or changes a resource to do what a player does.
- **Waits**: a click is not a transition, so a test waits for its result with `wait_for(&locator, SCREEN)` or `wait_until("what", timeout, |world| ...)`. `SCREEN` (2 s) is for the screens, `SERVER` (30 s) for what goes through the integrated server (loading a world, a chat message). A wait that times out, or a `get` that finds nothing, fails with the locator (or the description) it waited for, the texts visible on screen, and the line of the test; nothing hangs, and a watchdog aborts the whole test process, naming the test, when a harness lives longer than 300 s.

A test is a few lines:

```rust
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn singleplayer_opens_the_world_list() {
    let mut h = Harness::new();
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    h.click(&L::text("Singleplayer"));
    h.wait_for(&L::text("World list"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Solo);
}
```

- **Goldens**: `h.assert_screenshot("menu_main")` compares the screen with `client/tests/golden/menu_main.png`, as described in "UI screenshots": `UPDATE_GOLDEN=1 cargo test -p client golden` rewrites them.
- **Review snapshots**: `h.snapshot("name")` saves the screen as `target/ui-snapshots/<test>/<name>_<width>x<height>.png` when `UI_SNAPSHOTS=1`, and does nothing otherwise. The ignored test `every_screen_at_three_resolutions` saves every screen at 800x600, 1280x720 and 1920x1080 (30 files): run `UI_SNAPSHOTS=1 cargo test -p client review -- --ignored` and look at them after a change of the UI.
- **What a test freezes**: only the behaviour we want. A known defect is not frozen by a test that passes with it: its test is written with the intended behaviour, `#[ignore = "defect: ..."]`, and enabled by the commit that fixes it.

### Commit messages:

- We follow the [Conventional Commit specification](https://www.conventionalcommits.org/en/v1.0.0/). Our commit types are inspired by the [Karma specification](http://karma-runner.github.io/6.4/dev/git-commit-msg.html)
  ```
  <type>[optional scope]: <description>

  [optional body]

  [optional footer(s)]
  ```

  Allowed <type> values: 
  - **feat** for a new feature for the user, not a new feature for build script. Such commit will trigger a release bumping a MINOR version.
  - **fix** for a bug fix for the user, not a fix to a build script. Such commit will trigger a release bumping a PATCH version.
  - **perf** for performance improvements. Such commit will trigger a release bumping a PATCH version.
  - **docs** for changes to the documentation.
  - **style** for formatting changes, missing semicolons, etc.
  - **refactor** for refactoring production code, e.g. renaming a variable.
  - **test** for adding missing tests, refactoring tests; no production code change.
  - **build** for updating build configuration, development tools or other changes irrelevant to the user.

- Write commit messages in the present tense (e.g., "Add feature X" instead of "Added feature X").

### Branches
- Use the naming convention `<type>/<name>` for branches introducing new features. Only use lowercase letters, numbers, and dashes.
- The `main` branch should always compile successfully and be free of warnings. Use `cargo check`.
- Experimental branches are allowed to include code that does not build successfully.
- Prefer rebasing over merging.
