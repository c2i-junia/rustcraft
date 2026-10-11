#!/usr/bin/env sh

set -xe

# check if cargo is installed
if ! command -v cargo >/dev/null 2>&1; then
  echo "\033[1;31m[ERROR]\033[0m Cargo is not installed. Please install Rust from https://rustup.rs."
  exit 1
fi

# install `cargo watch`
cargo install cargo-watch

# install `just`
cargo install just

echo "Setup complete! All dependencies have been installed and configured."
