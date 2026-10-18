#!/usr/bin/env sh

just create-game-folders

# RUST_BACKTRACE=1 \
RUST_LOG=client=debug,server=debug,shared=debug,warn \
cargo run \
--package client \
--bin client \
-- \
--game-folder-path $PWD/appdata/client-1 \
--assets-folder-path $PWD/data \
--player-name PlayerOne
