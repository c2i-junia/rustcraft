#!/usr/bin/env bash
# WorktreeCreate hook: creates the worktree, then reflinks (or, without reflinks, hard links and copies) the main checkout's
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

# Whether the filesystem shares the blocks of copied files (btrfs, xfs), tried on one small file.
supports_reflink() {
    local probe="$dest/.reflink-probe" status=0
    cp --reflink=always "$dest/Cargo.toml" "$probe" 2>/dev/null || status=1
    rm -f "$probe"
    return $status
}

# Without reflinks (ext4): the built dependencies of the dev profile only, as our own crates
# have another path in the worktree and are rebuilt anyway. The libraries, two thirds of the
# size, are hard linked: rustc replaces them by new files and never writes into them. The rest
# is copied, as cargo does write into its fingerprints.
copy_dependencies() {
    local from="$repo/target/debug/build" to="$dest/target/debug/build"
    local own=(--exclude=/client --exclude=/server --exclude=/shared)
    [ -d "$from" ] || return 0
    mkdir -p "$to"
    rsync -a "${own[@]}" --exclude='*.rlib' --exclude='*.rmeta' "$from/" "$to/" >&2 &&
        rsync -a "${own[@]}" --include='*/' --include='*.rlib' --include='*.rmeta' --exclude='*' \
            --link-dest="$from" "$from/" "$to/" >&2
}

if [ -d "$repo/target" ] && [ ! -e "$dest/target" ]; then
    if supports_reflink; then
        # --reflink=always fails instead of silently doing a full copy of target/
        cp -a --reflink=always "$repo/target" "$dest/target" >&2 || failed=1
    else
        copy_dependencies || failed=1
    fi
    if [ -n "${failed:-}" ]; then
        rm -rf "$dest/target"
        echo "worktree-create: target/ could not be reused, the worktree will build from scratch" >&2
    fi
fi

echo "$dest"
