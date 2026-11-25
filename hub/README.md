# Rustcraft hub

The hub is the central service of Rustcraft, served at `https://hub.rustcraft.dev` and
replaceable by any other. It keeps the players' accounts, signs the short-lived certificates
that let a game server recognise a player without calling the hub, and (in later versions) starts
hosted worlds. It is a separate Cargo project: its own workspace and `Cargo.lock`, excluded
from the game's workspace, but it builds with the repository's nightly toolchain and
`.cargo/config.toml`, and it uses the `crates/protocol` crate shared with the game.

This is the skeleton: configuration, the database with the account tables, the
`/.well-known/rustcraft-hub` document and `/healthz`. The design is in
`docs/superpowers/specs/2026-11-25-hub-design.md`.

## Running it locally

You need Postgres 17 (the extension `citext` comes with it). The test database script starts
one in Docker:

```sh
cd hub
export HUB_DATABASE_URL=$(scripts/test-db.sh start)
cargo run -- generate-key signing.key          # a hex seed, mode 0600
openssl rand -hex 32 > ip-secret               # keys the sign-up address hashes
cargo run -- serve --public-url http://127.0.0.1:8080 \
    --signing-key-file signing.key --ip-secret-file ip-secret
curl http://127.0.0.1:8080/.well-known/rustcraft-hub
```

Migrations (`migrations/`) are applied at start. Every option has an environment variable:

| Flag | Variable | Default |
|---|---|---|
| `--listen` | `HUB_LISTEN` | `127.0.0.1:8080` |
| `--database-url` | `HUB_DATABASE_URL` | |
| `--database-url-file` | `HUB_DATABASE_URL_FILE` | (a file holding the URL, instead of the flag) |
| `--public-url` | `HUB_PUBLIC_URL` | |
| `--signing-key-file` | `HUB_SIGNING_KEY_FILE` | |
| `--ip-secret-file` | `HUB_IP_SECRET_FILE` | |
| `--name` | `HUB_NAME` | `Rustcraft` |

## Testing

Tests that touch the database create a database of their own in the Postgres that
`HUB_TEST_DATABASE_URL` names, and drop it at the end:

```sh
cd hub
export HUB_TEST_DATABASE_URL=$(scripts/test-db.sh start)   # postgres:17 on 127.0.0.1:55432
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/test-db.sh stop
```

Queries are plain `sqlx::query` calls checked at run time, so building needs no database.
