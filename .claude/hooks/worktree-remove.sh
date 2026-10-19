#!/usr/bin/env bash
# WorktreeRemove hook: counterpart of worktree-create.sh.
set -euo pipefail

path=$(jq -r '.worktree_path' <<<"$(cat)")
[ -d "$path" ] || exit 0

git -C "$path" worktree remove --force "$path" >&2
