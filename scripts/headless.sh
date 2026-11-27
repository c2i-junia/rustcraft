#!/usr/bin/env bash
# Runs a command of the game headless, under gamescope, for at most SECONDS, then kills it
# and everything it started (gamescope, cargo, the client), so that a run never outlives its
# time. Never opens a window on the desktop. Exits with the command's status, or 124 when the
# time ran out.
#
#   scripts/headless.sh SECONDS command [args...]
#   scripts/headless.sh 120 cargo run -p client -- --game-folder-path /tmp/game
set -u

seconds="$1"
shift

# A session of its own, so that the whole group can be killed at once.
setsid gamescope --backend headless -W 1280 -H 720 -- env -u WAYLAND_DISPLAY "$@" &
leader=$!
expired="$(mktemp)"
rm -f "$expired"

(
    sleep "$seconds"
    touch "$expired"
    kill -TERM -- "-$leader" 2>/dev/null
    sleep 5
    kill -KILL -- "-$leader" 2>/dev/null
) &
reaper=$!

wait "$leader"
status=$?
kill "$reaper" 2>/dev/null
# Say so when the time ran out, as timeout(1) does.
if [ -e "$expired" ]; then
    rm -f "$expired"
    status=124
fi
# Whatever the leader left behind goes too.
kill -KILL -- "-$leader" 2>/dev/null
exit "$status"
