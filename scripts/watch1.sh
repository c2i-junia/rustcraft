#!/usr/bin/env sh

cargo watch \
    --watch client/src \
    --watch server/src \
    --watch shared/src \
    -- bash -c "./run1.sh"
