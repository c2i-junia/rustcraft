#!/usr/bin/env sh

just create-game-folders

RUST_LOG=client=info \
cargo run \
--release \
--bin client \
-- \
--game-folder-path $PWD/appdata/client-1 \
--assets-folder-path $PWD/data \
--special-flag \
--player-name PlayerOne