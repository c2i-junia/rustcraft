#!/usr/bin/env sh

just create-game-folders

RUST_LOG=server=info \
cargo run \
--release \
--bin server \
-- \
--port 8000 \
--game-folder-path $PWD/appdata/server
