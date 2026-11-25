# Rustcraft hub

The hub is the central service of Rustcraft, served at `https://hub.rustcraft.dev` and
replaceable by any other. It keeps the players' accounts, signs the short-lived certificates
that let a game server recognise a player without calling the hub, and (in later versions) starts
hosted worlds. It is a separate Cargo project: its own workspace and `Cargo.lock`, excluded
from the game's workspace, but it builds with the repository's nightly toolchain and
`.cargo/config.toml`, and it uses the `crates/protocol` crate shared with the game.

So far it serves the `/.well-known/rustcraft-hub` document, `/healthz` and the accounts:
sign-up, login and logout, recovery codes, `GET /v1/me`, password change and account deletion.
The design, with the routes and their limits, is in
`docs/superpowers/specs/2026-11-25-hub-design.md`.

## Accounts

- Passwords and recovery codes are hashed with Argon2id (`m = 19456 KiB, t = 2, p = 1`), on
  blocking threads, at most half the cores at a time.
- A session is a bearer token (`Authorization: Bearer <token>`) of which the hub keeps the
  SHA-256 only. It lasts 90 days after its last use.
- Changing the password or using the recovery code ends every other session and revokes every
  certificate of the account. A deleted account keeps its row and, for 30 days, its name.
- Sign-ups are counted in the `signups` table by a keyed hash of the IPv4 address or IPv6 /64
  (never the address itself): 10 an hour and 30 a day per address, 60 an hour from a trusted
  range, 200 an hour on the whole hub. Failed logins are counted in memory: 5 per name and
  address and 30 per address in 15 minutes.
- The client's address is the TCP peer, or the last address of `X-Forwarded-For` when the peer
  is the loopback (Caddy on the same machine). Do not publish the hub's port anywhere else.
- Every hour the hub forgets sign-ups older than 30 days and expired sessions; every day it
  frees the names of accounts deleted 30 days before (they become `deleted-<id>`).
- `audit_log` records sign-ups, logins and failed ones, password changes, recoveries and
  deletions.

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

`*.key`, `ip-secret` and `secrets/` in `hub/` are git-ignored: keep secrets there and nowhere else.

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
| `--trusted-ranges` | `HUB_TRUSTED_RANGES` | (none; comma-separated CIDRs allowed 60 sign-ups an hour per address) |

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
