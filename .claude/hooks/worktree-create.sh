#!/usr/bin/env bash
# WorktreeCreate hook: creates the worktree, then reflinks the main checkout's
# target/ into it so cargo reuses the already-built dependencies.
# Claude Code reads the worktree path from stdout, so everything else goes to stderr.
set -euo pipefail

input=$(cat)
name=$(jq -r '.name' <<<"$input")
# The main checkout, even when called from inside another worktree.
common_dir=$(git -C "$(jq -r '.cwd' <<<"$input")" rev-parse --path-format=absolute --git-common-dir)
repo=$(dirname "$common_dir")
# Next to the repository, not inside it: cargo merges the .cargo/config.toml of every parent
# directory, so a worktree nested in the main checkout builds with its rustflags twice, which
# makes every crate of the reflinked target/ stale.
dest="$(dirname "$repo")/$(basename "$repo")-worktrees/$name"
branch="worktree-$name"
mkdir -p "$(dirname "$dest")"

if git -C "$repo" show-ref --verify --quiet "refs/heads/$branch"; then
    git -C "$repo" worktree add "$dest" "$branch" >&2
else
    git -C "$repo" worktree add -b "$branch" "$dest" HEAD >&2
fi

if [ -d "$repo/target" ] && [ ! -e "$dest/target" ]; then
    # --reflink=always fails instead of silently doing a full copy of target/
    if ! cp -a --reflink=always "$repo/target" "$dest/target" >&2; then
        rm -rf "$dest/target"
        echo "worktree-create: reflink of target/ failed, the worktree will build from scratch" >&2
    fi
fi

echo "$dest"
