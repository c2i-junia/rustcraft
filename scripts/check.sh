#!/usr/bin/env bash
# The checks of the repository, run by `just check` and `just test-fast`.
#
#   scripts/check.sh [--force]   fmt check, clippy -D warnings and the whole test suite
#                                (`just test`).
#   scripts/check.sh test-fast   the test suite without the e2e tests.
#
# `check` remembers the tree it last passed on: the tree of the working directory, untracked
# files included and ignored ones left out, is stored in `target/.check-passed` after a
# successful run. When the tree has not changed, the next run returns at once. `--force` (or
# FORCE=1) skips that memo. A failing run stores nothing.
#
# The test suite runs under a lock shared by every worktree of the machine, so that two
# suites never compete for the CPU.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

lock="${XDG_RUNTIME_DIR:-/tmp}/rustcraft-tests.lock"

# Runs a command once no other test suite is running.
run_tests() {
    (
        if ! flock -n 9; then
            echo "check: waiting for another test run to finish (lock $lock)"
            flock 9
        fi
        "$@"
    ) 9>"$lock"
}

if [ "${1:-}" = "test-fast" ]; then
    run_tests cargo test --workspace -- --skip e2e::
    exit 0
fi

force="${FORCE:-}"
if [ "${1:-}" = "--force" ]; then
    force=1
fi

# The tree of the working directory, computed in a temporary index so that the real one is
# left alone.
index="$(mktemp -u)"
trap 'rm -f "$index"' EXIT
tree="$(GIT_INDEX_FILE="$index" git add -A && GIT_INDEX_FILE="$index" git write-tree)"

memo="${CARGO_TARGET_DIR:-target}/.check-passed"
if [ -z "$force" ] && [ -f "$memo" ] && [ "$(cat "$memo")" = "$tree" ]; then
    echo "check: already passed for tree $tree"
    exit 0
fi

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
run_tests just test

mkdir -p "$(dirname "$memo")"
echo "$tree" >"$memo"
