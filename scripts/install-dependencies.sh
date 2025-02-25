#!/usr/bin/env bash

set -xe

# check if cargo is installed
if ! command -v cargo &> /dev/null; then
  echo -e "\033[1;31m[ERROR]\033[0m Cargo is not installed. Please install Rust from https://rustup.rs."
  exit 1
fi

# install `cargo watch`
cargo install cargo-watch

# install `just`
cargo install just

# add the Nightly toolchain with the Cranelift backend
rustup install nightly
rustup default nightly
rustup component add rustc-codegen-cranelift-preview --toolchain nightly

info "Setup complete! All dependencies have been installed and configured."
