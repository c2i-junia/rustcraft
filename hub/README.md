# Rustcraft hub

The hub is the central service of Rustcraft, served at `https://hub.rustcraft.dev` and
replaceable by any other. It keeps the players' accounts, signs the short-lived certificates
that let a game server recognise a player without calling the hub, and (in later versions) starts
hosted worlds. It is a separate Cargo project: its own workspace and `Cargo.lock`, excluded
from the game's workspace, but it builds with the repository's nightly toolchain and
`.cargo/config.toml`, and it uses the `crates/protocol` crate shared with the game.

So far it serves the `/.well-known/rustcraft-hub` document, `/healthz`, the accounts (sign-up,
login and logout, recovery codes, `GET /v1/me`, password change and account deletion), device
certificates, the list of revoked ones and public profiles. Its realm agent runs hosted worlds
in Docker containers, which the hub does not drive yet.
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
  is in `--trusted-proxies` (`HUB_TRUSTED_PROXIES`, comma-separated CIDRs, default
  `127.0.0.1/32,::1/128`: Caddy on the same machine). Do not publish the hub's port anywhere else.
- Every hour the hub forgets sign-ups older than 30 days and expired sessions; every day it
  frees the names of accounts deleted 30 days before (they become `deleted-<id>`).
- `audit_log` records sign-ups, logins and failed ones, password changes, recoveries and
  deletions.

## Certificates and administration

- `POST /v1/certificates` signs a certificate of 7 days for a device key (hex), 30 an hour per
  account (429 beyond). `GET /v1/revoked` lists the ids of revoked certificates that have not
  expired; `GET /v1/profiles?name=` gives `{id, username}` of an active account.
- A suspended account gets 403 `forbidden` with the reason, at login (once the password is right)
  and on any route; suspending, like a password change, ends its sessions and revokes its
  certificates.
- Admin commands take the database options of `serve` and write `audit_log` rows as `admin-cli`:

```sh
rustcraft-hub admin users
rustcraft-hub admin grant <user> host [--realms N]   # 1 world by default
rustcraft-hub admin grant <user> admin
rustcraft-hub admin revoke <user> <host|admin>       # a host's quota is kept
rustcraft-hub admin suspend <user> --reason "<text>"
rustcraft-hub admin unsuspend <user>
rustcraft-hub admin reset-password <user>            # prints a 16-character password
```

## Realm agent

Hosted worlds run in Docker containers, which only the realm agent (`realm-agent/`, binary
`rustcraft-realm-agent`) creates: it is the one service with `docker.sock`, and the hub asks it
over a Unix socket (mode 0660), so that a hub taken over can start, stop and archive worlds but
not run a container of its choosing. Its configuration (`--config`, `realm-agent.example.toml`)
fixes the image, the limits, the ports and the hub of the worlds; a request names only the world
(a UUID), its slot (below `slots`), its token and the settings of a new world, which are checked
against the game's names before they reach a command line.

The requests are newline-delimited JSON, a response per line in order (types in
`agent-protocol/`, crate `rustcraft-agent-protocol`):

| Request | Does | Response |
|---|---|---|
| `{"request":"start","realm":…,"slot":n,"token":…,"settings":{…}}` | creates volume and container `rustcraft-realm-<uuid>` and starts it | `done`; `refused` if it runs |
| `{"request":"stop","realm":…}` | SIGTERM, `stop_grace_seconds` (30) to save, then removes the container; the volume stays | `done` |
| `{"request":"status","realm":…}` | reads the state of the container | `status` with `absent`, `running` or `exited` and its code |
| `{"request":"archive","realm":…}` | stops the world, moves its volume to `rustcraft-realm-<uuid>-archived-<yyyymmdd>` | `done` |
| `{"request":"purge"}` | removes the archives older than `archive_days` (30), by the date in their name | `purged` with their names |

A malformed or out-of-bounds request gets `{"result":"refused","reason":…}`, a Docker failure
`{"result":"failed","error":…}`.

A world's container runs the configured image as user 10001 with a read-only root filesystem,
`/tmp` on a tmpfs, the world on its volume at `/data`, `cap_drop ALL`, `no-new-privileges`,
`memory` (1536m, no swap), `cpus` (1.5) and `pids` (256), on the Docker network `network`
(`bridge`; never `host`, `none` or `container:…`), its logs rotated (json-file, 3 files of
10 MB). Slot `n` publishes UDP `base_port + 2n` (game) and the next port (status) on
`0.0.0.0` and `::`, to the server's 8000 and 8001. The command is `--world world --port 8000
--auth hub --hub <hub_url> --public-address <public_host>:<game port> --realm <uuid>
--max-render-distance 12 --max-players 10` and the settings, with the token in
`RUSTCRAFT_REALM_TOKEN`.

Docker cannot rename a volume, so `archive` copies it: it creates the archive volume, runs a
throwaway container of the configured image (`cp -a`, user 10001, no network, the world's
volume read-only, the limits of a world, 15 minutes at most) and removes the world's volume once
the copy has succeeded; on a failure the archive is removed and the world stays as it was. The
image must therefore have `cp`, as the game's (Debian) does. Archiving a world that has no
volume does nothing. An archive of today beside the world's volume, or a copy container left
over, are the remains of an interrupted archive: they are removed and the archive redone.

A start that fails (a port taken, say) removes the container, and the volume if the start
created it. `stop_grace_seconds` is at most 100, below the 120 s after which the agent's Docker
client gives up on a request.

The agent does not pull: `docker pull` the image on the host. As a world takes the image at each
start, a new image upgrades the worlds as they restart.

The test against Docker is ignored by default. It builds a small stand-in image of the server,
uses ports 47310 to 47313 and a random world, and removes all it made:

```sh
cargo test -p rustcraft-realm-agent --test docker -- --ignored
```

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
| `--trusted-proxies` | `HUB_TRUSTED_PROXIES` | `127.0.0.1/32,::1/128` (peers whose `X-Forwarded-For` is believed) |
| `--trusted-ranges` | `HUB_TRUSTED_RANGES` | (none; comma-separated CIDRs allowed 60 sign-ups an hour per address) |

## Deploying

The hub runs with Docker Compose (`hub/compose.yaml`): a `postgres:17` service on an internal
network with a named volume, the hub, published on `127.0.0.1:8080` only, for Caddy, and the
realm agent. All restart unless stopped; the hub waits for the database to be healthy. The
images are built from the repository root (`hub/Dockerfile`, targets `hub` and `realm-agent`,
distroless images).

The realm agent has no network (`network_mode: none`), a read-only root filesystem and no
capability; it mounts `/var/run/docker.sock` and shares its socket with the hub in the
`agent-socket` volume (`/run/realm-agent/agent.sock`). It runs as root with the group 65532:
root because the host's `docker.sock` is `root:docker 0660` and the gid of `docker` differs from
one host to the next (holding that socket is as good as root anyway), and the group 65532 is the
hub's (`nonroot`), which the agent's socket gets, so that the hub can connect. Its configuration
is `hub/realm-agent.toml` (git-ignored), mounted as a Compose config:

```sh
cp realm-agent.example.toml realm-agent.toml   # set image and public_host
docker pull ghcr.io/c2i-junia/rustcraft/rustcraft-server:latest
```

On the server, from `hub/`, build the image, then create the secrets
(`secrets.example/README.txt` lists them):

```sh
docker compose build hub
mkdir secrets && cd secrets
openssl rand -hex 24 > db_password
printf 'postgres://hub:%s@db:5432/hub\n' "$(cat db_password)" > database_url
openssl rand -hex 32 > ip-secret
docker run --rm --user "$(id -u)" --entrypoint rustcraft-hub -v "$PWD:/out" \
    rustcraft-hub generate-key /out/signing.key
cd .. && chmod 700 secrets && chmod 444 secrets/*
```

Compose mounts each secret file as it is: its `uid`, `gid` and `mode` options are ignored outside
Swarm, so the hub (uid 65532) can read a file only if it is world-readable. The `0700` folder
keeps the other users of the machine away from the files; do not loosen it. Back up
`signing.key`: a hub with another key has to be announced again to every game server.

Then start it:

```sh
docker compose up -d
curl http://127.0.0.1:8080/.well-known/rustcraft-hub
```

Behind Docker's port publishing the hub does not see Caddy as `127.0.0.1`: every connection
comes from the gateway of the Compose network the port is published on. `compose.yaml` therefore
pins that network to `172.30.80.0/24` and sets `HUB_TRUSTED_PROXIES: 172.30.80.1/32`; change both
together if the subnet clashes with yours. Without this every player would share one sign-up
limit. To check, post two sign-ups with different `X-Forwarded-For` headers and look at
`select encode(ip_key, 'hex') from signups` in the database: the keys must differ.

`HUB_PUBLIC_URL`, `HUB_NAME` and `HUB_TRUSTED_RANGES` are set in the `environment` of `hub` in
`compose.yaml`. To update, `git pull` and `docker compose up -d --build`; migrations are applied
at start. `HUB_PORT` changes the published port, for a machine where 8080 is taken.

Caddy (on the host) terminates TLS:

```
hub.rustcraft.dev {
    reverse_proxy 127.0.0.1:8080
}
```

Administration runs in a one-off container with the same secrets:

```sh
docker compose run --rm --entrypoint rustcraft-hub hub admin users
docker compose run --rm --entrypoint rustcraft-hub hub admin grant <user> host
```

Backup (nightly, from cron) and restore:

```sh
docker compose exec -T db pg_dump -U hub -Fc hub > hub-$(date +%F).dump
docker compose exec -T db pg_restore -U hub -d hub --clean --if-exists < hub-2026-11-25.dump
```

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
