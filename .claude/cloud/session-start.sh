#!/usr/bin/env bash
# SessionStart hook: in a cloud session only, builds in the background what setup.sh left. It
# does nothing on a computer.
[ "${CLAUDE_CODE_REMOTE:-}" = true ] || exit 0

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/opt/rustcraft-target}"
export PATH="$HOME/.cargo/bin:$PATH"
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

if ! pgrep -x cargo >/dev/null; then
    nohup nice -n 5 cargo build --workspace --tests >/tmp/warm-build.log 2>&1 </dev/null &
fi
echo "Cloud session: the workspace is being built in the background (/tmp/warm-build.log," \
    "CARGO_TARGET_DIR=$CARGO_TARGET_DIR). A cargo command waits for it: run it in the background."
exit 0
