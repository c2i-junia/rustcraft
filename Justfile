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

    # add other files 
    cp CHANGELOG.txt release/
    cp LICENSE.txt release/

    # copy paste binaries 
    cp target/release/server release/bin/rustcraft-server

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
