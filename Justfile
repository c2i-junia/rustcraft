remove-game-folders:
    rm -rf appdata

create-game-folders:
    mkdir -p appdata

    # client 1
    mkdir -p appdata/client-1 appdata/client-1/saves
    touch appdata/client-1/servers.ron

    # client 2
    mkdir -p appdata/client-2 appdata/client-2/saves
    touch appdata/client-2/servers.ron

    # server
    mkdir -p appdata/server appdata/server/saves

debug: create-game-folders
    cargo build

release: create-game-folders
    cargo build --release --no-default-features

# Runs the client at release speed with a fast rebuild (the `play` profile). Extra arguments go
# to the client.
play *args: create-game-folders
    cargo run --profile play --package client --bin client -- \
        --game-folder-path {{justfile_directory()}}/appdata/client-1 \
        --assets-folder-path {{justfile_directory()}}/data {{args}}

generate-release-folder:
    cargo build --release --no-default-features

    # create folder
    mkdir -p release
    mkdir -p release release/saves release/data release/bin

    # copy paste data folder 
    cp -r data release/

    # add config files 
    touch release/servers.ron

    # add other files 
    cp CHANGELOG.txt release/
    cp LICENSE.txt release/
    cp docs/ASSETS.md release/

    # copy paste binaries 
    cp target/release/client release/bin/rustcraft
    cp target/release/server release/bin/rustcraft-server

generate-release-folder-server:
    cargo build --release --no-default-features --bin server

    # create folder
    mkdir -p release
    mkdir -p release release/bin

    # the content files, which define the blocks and items: run the server with
    # `--assets-folder-path data` from the release folder
    mkdir -p release/data
    cp -r data/content release/data/

    # add other files 
    cp CHANGELOG.txt release/
    cp LICENSE.txt release/

    # copy paste binaries 
    cp target/release/server release/bin/rustcraft-server

# Runs the client headless under gamescope for SECONDS with the Bevy Remote Protocol on
# http://127.0.0.1:15702 (the `brp` feature), for an agent to drive it and take screenshots of it.
# Run it in the background; CLAUDE.md lists the calls. Extra arguments go to the client.
brp seconds="240" *args="": create-game-folders
    scripts/headless.sh {{seconds}} cargo run --package client --bin client --features brp -- \
        --game-folder-path {{justfile_directory()}}/appdata/client-1 \
        --assets-folder-path {{justfile_directory()}}/data --player-name PlayerOne {{args}}

# Validates the content files (`data/content/rustcraft/*.ron`) as the game reads them, without a
# world, a window or a network: the errors name the file and the key. Pass a folder of `*.ron`
# files to check that one instead. Builds the server only. See docs/CONTENT.md.
check-content *args:
    cargo run --quiet --package server --bin server -- \
        --assets-folder-path {{justfile_directory()}}/data --check-content {{args}}

# Fmt, clippy and the whole suite; returns at once when the tree already passed.
# `just check --force` runs it anyway.
check *args:
    scripts/check.sh {{args}}

# Every test. The client's, end-to-end for the most part, take the longest: the other crates' run
# beside them rather than after. Each client draws with lavapipe, whose threads default to one
# per core: with 16 clients at once, 8 each spend less CPU on keeping them busy (the suite took
# 255 s instead of 330), and 4 slow the tests of the shaders past their waits.
test:
    #!/usr/bin/env bash
    set -u
    cargo test --workspace --no-run --quiet || exit 1
    cargo test --workspace --exclude client & others=$!
    LP_NUM_THREADS="${LP_NUM_THREADS:-8}" cargo test -p client; client=$?
    wait $others; others=$?
    exit $(( client || others ))

test-fast:
    scripts/check.sh test-fast

# Runs the client under the Dioxus CLI with hot patching: a saved change to a system's body is
# patched into the running game, and the screen shown is rebuilt (the menu screen, or the HUD and
# the pause menu). Needs dioxus-cli 0.7.10 (`cargo install dioxus-cli --version 0.7.10 --locked`).
# Extra arguments go to dx: `--interactive=false` prints a plain log. A tool for the owner, who
# runs it on the desktop; agents run it headless only, as
# `scripts/headless.sh 240 just hot --interactive=false`. See docs/HOT_RELOAD.md.
hot *dx_args: create-game-folders
    # dx needs the linker arguments of a fresh build of the client: without this, a client left
    # unchanged since the last `just hot` fails to start ("Missing linker args for fat link")
    touch client/src/main.rs
    BEVY_ASSET_ROOT="{{justfile_directory()}}/client" \
    RUST_LOG="${RUST_LOG:-info,bevy_ecs::system=debug,wgpu=error,naga=warn}" \
    dx serve --hot-patch {{dx_args}} \
        -p client --bin client --no-default-features --features hot \
        "--args=--game-folder-path {{justfile_directory()}}/appdata/client-1 --assets-folder-path {{justfile_directory()}}/data --player-name PlayerOne"
