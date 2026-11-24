#!/usr/bin/env bash
# Setup script of the cloud environment of Claude Code: its text is pasted in the environment's
# "Setup script" field at claude.ai/code. It runs as root on Ubuntu 24.04, before the session.
#
# What it writes is cached only when it ends within about five minutes, and the dependencies take
# longer to build: it builds them for a limited time, into a target/ outside the repository, and
# a session builds the rest (session-start.sh).
set -uo pipefail

REPO_URL=https://github.com/c2i-junia/rustcraft
SEED=/opt/rustcraft-seed
export CARGO_TARGET_DIR=/opt/rustcraft-target
# Seconds the whole script may take, and since when
BUDGET=270
SECONDS=0

# As .github/workflows/ci.yml, without what the image already has
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    build-essential pkg-config libasound2-dev libudev-dev libx11-dev libxcursor-dev libxi-dev \
    libxrandr-dev libxinerama-dev libgl1-mesa-dev libegl1-mesa-dev libssl-dev libwayland-dev \
    libxkbcommon-dev libxkbcommon-x11-0 libdbus-1-dev cmake libfreetype6-dev libexpat1-dev \
    zlib1g-dev mold clang mesa-vulkan-drivers || exit 1

if ! command -v rustup >/dev/null; then
    curl -fsSL https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init \
        -o /tmp/rustup-init || exit 1
    chmod +x /tmp/rustup-init
    /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain none || exit 1
    # Before any other cargo of the image
    ln -sf "$HOME"/.cargo/bin/* /usr/local/bin/
fi
export PATH="$HOME/.cargo/bin:$PATH"

rm -rf "$SEED"
git clone -q --depth 1 "$REPO_URL" "$SEED" || exit 0
cd "$SEED" || exit 0
rustup toolchain install || exit 1
cargo fetch --locked || exit 0

left=$((BUDGET - SECONDS))
if [ "$left" -gt 20 ]; then
    # Interrupted, cargo keeps the dependencies already built
    timeout --signal=INT --kill-after=10 "$left" \
        cargo build --workspace --tests --keep-going >/opt/rustcraft-setup.log 2>&1
fi
echo "setup done in $SECONDS s" >>/opt/rustcraft-setup.log
exit 0
