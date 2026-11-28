#!/usr/bin/env sh

# Like run-release.sh, at release speed with a fast rebuild (the `play` profile, see Cargo.toml).
# Content files (data/content/rustcraft/*.ron) and textures reload in game when saved.
# Extra arguments go to the client.

just create-game-folders

# RUST_BACKTRACE=1 \
RUST_LOG=client=debug,server=debug,shared=debug,warn \
cargo run --profile play \
--package client \
--bin client \
-- \
--game-folder-path $PWD/appdata/client-1 \
--assets-folder-path $PWD/data \
"$@"
