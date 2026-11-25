#!/usr/bin/env bash
# Starts or stops the throwaway Postgres the hub tests use.
#   scripts/test-db.sh start   prints the URL to put in HUB_TEST_DATABASE_URL
#   scripts/test-db.sh stop    removes the container
set -euo pipefail

NAME=rustcraft-hub-test-db
PORT=55432
URL="postgres://postgres:postgres@127.0.0.1:$PORT/postgres"

case "${1:-start}" in
start)
    if [ -z "$(docker ps -q -f "name=^$NAME\$")" ]; then
        docker rm -f "$NAME" >/dev/null 2>&1 || true
        docker run -d --name "$NAME" -e POSTGRES_PASSWORD=postgres \
            -p "127.0.0.1:$PORT:5432" postgres:17 >/dev/null
    fi
    for _ in $(seq 60); do
        docker exec "$NAME" pg_isready -U postgres -h 127.0.0.1 >/dev/null 2>&1 && break
        sleep 1
    done
    docker exec "$NAME" pg_isready -U postgres -h 127.0.0.1 >/dev/null
    echo "$URL"
    ;;
stop)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    ;;
*)
    echo "usage: $0 start|stop" >&2
    exit 2
    ;;
esac
