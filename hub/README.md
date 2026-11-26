# Rustcraft hub

The hub is the central service of Rustcraft, served at `https://hub.rustcraft.dev` and
replaceable by any other. It keeps the players' accounts, signs the short-lived certificates
that let a game server recognise a player without calling the hub, and starts hosted worlds. It is a separate Cargo project: its own workspace and `Cargo.lock`, excluded
from the game's workspace, but it builds with the repository's nightly toolchain and
`.cargo/config.toml`, and it uses the `crates/protocol` crate shared with the game.

So far it serves the `/.well-known/rustcraft-hub` document, `/healthz`, the accounts (sign-up,
login and logout, recovery codes, `GET /v1/me`, password change and account deletion), device
certificates, the list of revoked ones, public profiles and hosted worlds, which its realm agent
runs in Docker containers.
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
  certificates. A game server in hub mode reads the revoked certificates every hour: it then
  refuses the account and dismisses it if it is playing. A hosted world reads its members every
  minute, of active accounts only: a suspended member leaves its whitelist. A suspended owner's
  worlds stay as they are, and their members can still start and play them; the owner, whose
  certificates are revoked, cannot join them, nor manage them, until unsuspended.
- Admin commands take the database options of `serve` and write `audit_log` rows as `admin-cli`.
  Like `serve`, they apply the migrations of their build when they connect: on an update or a
  roll back, the admin command of the new or old image migrates the database as the hub would.

```sh
rustcraft-hub admin users
rustcraft-hub admin grant <user> host [--realms N]   # 1 world by default
rustcraft-hub admin grant <user> admin
rustcraft-hub admin revoke <user> <host|admin>       # a host's quota is kept
rustcraft-hub admin suspend <user> --reason "<text>"
rustcraft-hub admin unsuspend <user>
rustcraft-hub admin reset-password <user>            # prints a 16-character password
rustcraft-hub admin realms                           # id, name, owner, state, slot, players, last played
```

## Hosted worlds

- An account with the `host` role creates worlds (`POST /v1/realms`, `{name, settings}`) while it
  owns fewer than its quota, archived ones not counted. A name has 1 to 32 characters and no
  control character; the settings (`seed` up to 4294967295, `world_type`, `game_mode`,
  `difficulty`, all optional) are checked against the names of the game's server
  (`agent-protocol/src/allowlist.rs`, shared with the agent; no `single-biome`).
- A world is seen by its owner and its members only (`GET /v1/realms`); to anyone else every
  route of the world answers 404. The owner adds members by username, as `member` or
  `operator` (`PUT /v1/realms/{id}/members/{username}`), removes them, stops the world
  (`POST …/stop`, 202: it is `stopping` until its server has saved) and archives it
  (`DELETE /v1/realms/{id}`). Any member starts it.
- `POST /v1/realms/{id}/start` takes the lowest free slot, at most `--max-running` worlds at once
  (503 `no_free_slot` beyond), at most 10 starts of a world an hour (429), issues a new realm
  token (32 random bytes, base64url; the hub keeps its SHA-256) and asks the agent to start the
  container with it. It answers `{state: "starting", host: --realm-host, port}`, the port of
  slot `n` being `--realm-base-port + 2n`. A world starting or running already gets its address
  again.
- The world's server reads `GET /v1/realms/{id}/config` with `Authorization: Realm <token>`: the
  owner and the members with their roles. A wrong token, a stopped or archived world and an
  unknown one all get 401. A stop ends the token.
- The monitor probes each starting world's status port every 5 s: the world is `running` when
  it answers, `failed` (and stopped) after 60 s without an answer or when its container ends. It
  looks at each running world every 30 s: 10 minutes at 0 players (or without an answer) stop
  it, and a container that ended makes it `stopped` (exit code 0) or `failed`. A world the agent
  did not stop stays `stopping` and is stopped again. When the hub starts, and every day, it
  takes the agent's view of the worlds it thought had a container, and stops a running
  container of a world it thought had none (a start cut short while the agent carried it out).
- An archived world leaves the lists at once and its token ends; the agent then moves its volume
  aside, and the world keeps its slot until it has. Deleting an account archives the worlds it
  owns and takes it out of the others. Every day the hub archives again what the agent did not,
  and asks it to purge the archives older than its `archive_days`.
- The hub probes the worlds over UDP on the status port of their slot, at `--realm-probe-host`
  (`--realm-host` by default). In Compose, set it to the gateway of the hub's network, so that
  the probes reach the published ports without going out through the public address.
- The config of a world lists its active members only: a suspended member stays in the owner's
  list, to be removed, but not in the world's whitelist.

## Realm agent

Hosted worlds run in Docker containers, which only the realm agent (`realm-agent/`, binary
`rustcraft-realm-agent`) creates: it is the one service with `docker.sock`, and the hub asks it
over a Unix socket (mode 0660), so that a hub taken over can start, stop and archive worlds but
not run a container of its choosing. Its configuration (`--config`, `realm-agent.example.toml`)
fixes the image, the limits, the ports and the hub of the worlds (its `base_port` and `slots` must
be the hub's `--realm-base-port` and `--realm-slots`); a request names only the world
(a UUID), its slot (below `slots`), its token and the settings of a new world, which are checked
against the game's names before they reach a command line.

The requests are newline-delimited JSON, a response per line in order (types in
`agent-protocol/`, crate `rustcraft-agent-protocol`):

| Request | Does | Response |
|---|---|---|
| `{"request":"start","realm":…,"slot":n,"token":…,"settings":{…}}` | creates volume and container `rustcraft-realm-<uuid>` and starts it | `done`; `refused` if it runs |
| `{"request":"stop","realm":…}` | SIGTERM, `stop_grace_seconds` (30) to save, then removes the container; the volume stays | `done` |
| `{"request":"status","realm":…}` | reads the state of the container | `status` with `absent`, `running` or `exited` and its code |
| `{"request":"archive","realm":…}` | stops the world, moves its volume to `rustcraft-realm-<uuid>-archived-<yyyymmddHHMMSS>` (UTC) | `done` |
| `{"request":"purge"}` | removes the archives older than `archive_days` (30), by the time in their name | `purged` with their names |

A malformed or out-of-bounds request gets `{"result":"refused","reason":…}`, a Docker failure
`{"result":"failed","error":…}`.

A world's container runs the configured image as user 10001 with a read-only root filesystem,
`/tmp` on a tmpfs, the world on its volume at `/data`, `cap_drop ALL`, `no-new-privileges`,
`memory` (1536m, no swap), `cpus` (1.5) and `pids` (256), on the Docker network `network`
(`bridge`; never `host`, `none` or `container:…`), its logs rotated (json-file, 3 files of
10 MB). Slot `n` publishes UDP `base_port + 2n` (game) and the next port (status) on
`0.0.0.0` and `::`, to the server's 8000 and 8001. The worlds are IPv4 only for now (the game's
server listens on IPv4, and the game joins over IPv4): on a network without IPv6, the binding on
`::` is Docker's userland proxy, which the host's firewall closes. The command is `--world world --port 8000
--auth hub --hub <hub_url> --public-address <public_host>:<game port> --realm <uuid>
--max-render-distance 12 --max-players 10` and the settings, with the token in
`RUSTCRAFT_REALM_TOKEN`.

Docker cannot rename a volume, so `archive` copies it: it creates the archive volume, named
after the UTC second of the archive, runs a throwaway container of the configured image (`cp
-a`, user 10001, no network, the world's volume read-only, the limits of a world, 15 minutes at
most) and removes the world's volume once the copy has succeeded. The image must therefore have
`cp`, as the game's (Debian) does. Archiving a world that has no volume does nothing; an archive
of the same second is refused. Only `purge` ever removes an archive: a world archived, started
again and archived again keeps both archives, and a copy that fails or is interrupted leaves the
world as it was and a partial archive, which `purge` removes in its time. A copy container left
over by an interrupted archive is removed before the next one.

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
| `--realm-host` | `HUB_REALM_HOST` | `play.rustcraft.dev` (where players join hosted worlds) |
| `--realm-probe-host` | `HUB_REALM_PROBE_HOST` | `--realm-host` (where the hub probes the worlds' status ports) |
| `--realm-base-port` | `HUB_REALM_BASE_PORT` | `25000` (the agent's `base_port`) |
| `--realm-slots` | `HUB_REALM_SLOTS` | `20` (the agent's `slots`) |
| `--max-running` | `HUB_MAX_RUNNING` | `3` (hosted worlds running at once) |
| `--agent-socket` | `HUB_AGENT_SOCKET` | `/run/realm-agent/agent.sock` |
| `--admin-dir` | `HUB_ADMIN_DIR` | (none: no web admin; the built `hub/admin`, served under `/admin/`) |

## Deploy

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
hub's (`nonroot`), which the agent's socket gets, so that the hub can connect.

The steps below are those of the production machine: Debian 13, Docker with Compose, 4 cores,
7.8 GB of memory and no swap, UFW allowing only 22, 80 and 443, and Caddy from apt serving
`rustcraft.dev` from `/etc/caddy/Caddyfile`. Run them as root, in order.

### 1. DNS

Four records, on Cloudflare, all **DNS only** (grey cloud):

| Name | Type | Value |
|---|---|---|
| `hub.rustcraft.dev` | A, AAAA | the machine's IPv4 and IPv6 addresses |
| `play.rustcraft.dev` | A, AAAA | the same |

Hosted worlds and game servers are IPv4 only for now (dual-stack is future work): the game
resolves `play` to its IPv4 address alone, and its AAAA record is unused until then.

`play` cannot be proxied: the worlds speak UDP, which Cloudflare's proxy does not carry. `hub`
could be, as Caddy serves it like the site, but then every request would reach Caddy from a
Cloudflare address, and the hub's limits per address (sign-ups, failed logins) would need
Caddy to be told Cloudflare's ranges (`trusted_proxies` in its `servers` options) to see the
player's. DNS only keeps Cloudflare out of the authentication path, and the client's address is
the one Caddy sees. (Caddy sets `X-Forwarded-For` itself, discarding any the client sent, as no proxy
is trusted on the way; the hub believes the last address of it.)

### 2. Get the code

Either from the public repository, once the branch is published there:

```sh
git clone --branch feat/hub https://github.com/c2i-junia/rustcraft /srv/rustcraft-hub
```

or, while it is only on a local checkout, by pushing it over SSH into a repository that updates
its working tree. On the machine:

```sh
git init /srv/rustcraft-hub
git -C /srv/rustcraft-hub config receive.denyCurrentBranch updateInstead
```

then, from the local checkout, and on the machine again:

```sh
git push root@<machine>:/srv/rustcraft-hub feat/hub     # locally
git -C /srv/rustcraft-hub checkout feat/hub             # on the machine
```

The rest runs in `/srv/rustcraft-hub`. Updating is another `git push` (the working tree
follows it as long as nothing is edited on the machine) or a `git pull`.

### 3. The image of the worlds

The agent never pulls an image: it runs the one named in its configuration, which must be on the
machine. Build the game's server from this checkout (the first build is long, about the time of
a release build of the game with 4 cores; nothing else needs to run meanwhile):

```sh
cd /srv/rustcraft-hub
docker build -t rustcraft-server:hub .
```

A machine with 8 GB of memory and no swap cannot build it: one rustc takes 5 GB and is killed.
Build the images on another machine from the same commit, then copy them over SSH. In step 7,
run `docker compose up -d` without `--build`:

```sh
# on the build machine, in the checkout
docker build -t rustcraft-server:hub .
(cd hub && docker compose build hub realm-agent)
docker save rustcraft-server:hub rustcraft-hub rustcraft-realm-agent | gzip -1 \
  | ssh root@vps 'gunzip | docker load'
```

### 4. The worlds' network

Worlds run on a Docker network of their own, where containers cannot talk to one another
(`enable_icc=false`): a world reaches the internet, but not the other worlds, and the hub's
containers are on other networks.
Compose does not make it (nothing it starts is on it), so it is made once. Check that the
subnet is free first (`docker network ls`, `ip route`); the hub's own is `172.30.80.0/24`. The
network is IPv4 only, as the worlds are for now.

```sh
docker network create --driver bridge \
    --subnet 172.30.81.0/24 \
    --opt com.docker.network.bridge.enable_icc=false \
    rustcraft-realms
```

A `rustcraft-realms` made with `--ipv6` by an earlier version of this guide is made again IPv4
only, while no world runs: `docker network rm rustcraft-realms`, then the command above. The
`accept_ra` file that version wrote (`/etc/sysctl.d/60-rustcraft-accept-ra.conf`) can stay.

### 5. The agent's configuration

`realm-agent.toml` (git-ignored, mounted by Compose) starts from the example. The edits: the
image of step 3, the network of step 4 and one CPU per world. Three worlds run at once
(`HUB_MAX_RUNNING`, in `compose.yaml`), each with 1.5 GiB of memory (no swap, so 4.5 GiB at most)
and 1 CPU (3 of the 4), which leaves 3 GiB and a core for Postgres, the hub, Caddy and the
machine. `public_host`, `base_port` and `slots` are already those of the hub's
`HUB_REALM_HOST` (`play.rustcraft.dev`), `--realm-base-port` and `--realm-slots`.

```sh
cd /srv/rustcraft-hub/hub
cp realm-agent.example.toml realm-agent.toml
sed -i -e 's|^image = .*|image = "rustcraft-server:hub"|' \
       -e 's|^cpus = .*|cpus = 1.0|' \
       -e 's|^network = .*|network = "rustcraft-realms"|' realm-agent.toml
grep -E '^(image|public_host|memory|cpus|network|base_port|slots) ' realm-agent.toml
```

### 6. The secrets

`secrets.example/README.txt` lists them. Build the hub image first, which makes the signing key:

```sh
cd /srv/rustcraft-hub/hub
docker compose build hub
umask 077
mkdir secrets && chmod 700 secrets && cd secrets
openssl rand -hex 24 > db_password
printf 'postgres://hub:%s@db:5432/hub\n' "$(cat db_password)" > database_url
openssl rand -hex 32 > ip-secret
docker run --rm --user "$(id -u)" --entrypoint rustcraft-hub -v "$PWD:/out" \
    rustcraft-hub generate-key /out/signing.key
cd .. && chmod 444 secrets/*
```

Compose mounts each secret file as it is: its `uid`, `gid` and `mode` options are ignored outside
Swarm, so the hub (uid 65532) can read a file only if it is world-readable. The `0700` folder
keeps the other users of the machine away from the files; do not loosen it. **Copy
`secrets/signing.key` somewhere safe, off the machine**: a hub with another key has to be
announced again to every game server.

### 7. Start it

```sh
cd /srv/rustcraft-hub/hub
docker compose up -d --build
docker compose ps
docker compose logs realm-agent hub | tail
curl -fsS http://127.0.0.1:8080/healthz
curl -fsS http://127.0.0.1:8080/.well-known/rustcraft-hub
```

`realm-agent` logs "the realm agent is up"; the three services are `running` and `db` `healthy`.
Behind Docker's port publishing the hub does not see Caddy as `127.0.0.1`: every connection comes
from the gateway of the Compose network the port is published on. `compose.yaml` therefore pins
that network to `172.30.80.0/24` and sets `HUB_TRUSTED_PROXIES: 172.30.80.1/32`; change both
together if the subnet clashes with yours. Without this every player would share one sign-up
limit. The same gateway is `HUB_REALM_PROBE_HOST`, where the hub probes the worlds. `HUB_PORT`
changes the published port, for a machine where 8080 is taken.

### 8. Caddy

```sh
cat >> /etc/caddy/Caddyfile <<'CADDY'

hub.rustcraft.dev {
    reverse_proxy 127.0.0.1:8080
}
CADDY
caddy validate --config /etc/caddy/Caddyfile
systemctl reload caddy
curl -fsS https://hub.rustcraft.dev/.well-known/rustcraft-hub
```

Caddy gets the certificate of `hub.rustcraft.dev` by itself once the DNS record is there.

### 9. The firewall

UFW does not see the ports Docker publishes: Docker's NAT rules take the packets before UFW's, so
a published port is open to the internet whatever UFW says. Only the worlds' ports must be
(UDP 25000 to 25039: slot `n` has the game on `25000 + 2n` and its status on the next one).
`deploy/rustcraft-docker-firewall.sh` puts the rest of the policy in Docker's `DOCKER-USER`
chain: for what comes in on the WAN interface, it lets through the replies
of connections (`ESTABLISHED,RELATED`, so the containers' own traffic out and back works),
the UDP packets that were addressed to a port from 25000 to 25039, and drops every other new
connection to a container. The ports are matched with `--ctorigdstport`, the destination before
NAT: in `DOCKER-USER` a `--dport` rule would see the container's port (8000, 8001), not the
published one. The systemd unit makes the rules again at each boot and each time Docker
restarts. (Between Docker starting and the unit running, a moment, the rules are not yet
there.) It writes the same rules for IPv6, where they guard a Docker network with IPv6 if one is
ever made; the worlds' network has none, and IPv6 packets to the worlds' ports reach Docker's
userland proxy through the host's `INPUT` chain, where UFW refuses them.

```sh
cd /srv/rustcraft-hub/hub
ip -br link show eth0            # the WAN interface; if it is not eth0, `systemctl edit` the unit
install -m 755 deploy/rustcraft-docker-firewall.sh /usr/local/sbin/
install -m 644 deploy/rustcraft-docker-firewall.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now rustcraft-docker-firewall.service
iptables -S DOCKER-USER; iptables -S RUSTCRAFT-WAN
ip6tables -S DOCKER-USER; ip6tables -S RUSTCRAFT-WAN
```

`DOCKER-USER` must start with `-A DOCKER-USER -i eth0 -j RUSTCRAFT-WAN`; `ip6tables` may have no
`DOCKER-USER` chain, which the script says and skips.

To remove the rules, `systemctl disable --now rustcraft-docker-firewall.service`: its stop
undoes the jump and empties the chain. `systemctl enable --now` puts them back.

### 10. The first accounts

There is no sign-up on the command line: make the account in the game (Account dialog, from the
name button of the main menu, with `--hub` left to its default), keep its recovery code, then
give it its roles. Administration runs in a one-off container with the same secrets:

```sh
cd /srv/rustcraft-hub/hub
docker compose run --rm --entrypoint rustcraft-hub hub admin users
docker compose run --rm --entrypoint rustcraft-hub hub admin grant <username> admin
docker compose run --rm --entrypoint rustcraft-hub hub admin grant <username> host --realms 2
```

The other commands are in "Certificates and administration".

### 11. Smoke checks

From your own computer, not the machine:

```sh
curl -fsS https://hub.rustcraft.dev/.well-known/rustcraft-hub      # the hub's document
curl -m 5 http://<ipv4 of the machine>:8080/healthz                 # must time out or be refused
```

Then, as the account of step 10, create a world in the Hosted worlds tab and join it: the first
start can take a minute, and the tab says when the world is `running`. **Do not announce hosted
worlds before a world has reached `running` in the tab**: a world the hub cannot probe is
`failed` after 60 s. Meanwhile, on the machine, probe the world from inside the hub's container
(its network namespace, the way the hub does it: to the gateway of its network), and the world
is in slot 0, so on 25000 and 25001, if it is the first:

```sh
docker ps --format '{{.Names}}\t{{.Status}}\t{{.Ports}}'   # rustcraft-realm-<uuid>, 25000-25001/udp
cd /srv/rustcraft-hub/hub
nsenter -n -t "$(docker inspect -f '{{.State.Pid}}' "$(docker compose ps -q hub)")" \
    deploy/probe-status.sh 172.30.80.1 25001     # 0 of 10 players
```

and from your computer (copy `deploy/probe-status.sh` there), the way the players reach it:

```sh
probe-status.sh play.rustcraft.dev 25001
```

Each answers with its players and fails after 3 s without a reply (the script needs `python3`:
`apt install python3` if the machine or your computer has none). Last, the network of the worlds:

```sh
docker network inspect rustcraft-realms --format '{{.Options}}'   # map[com.docker.network.bridge.enable_icc:false]
```

### 12. Backups

A nightly dump of the database, kept 14 days (it holds password hashes: the folder is private),
and the signing key, copied once as step 6 says. A dump cut short stays as a `.part` file,
removed in its turn after 14 days:

```sh
install -d -m 700 /var/backups/rustcraft-hub
cat > /etc/cron.d/rustcraft-hub-backup <<'CRON'
17 3 * * * root umask 077 && cd /srv/rustcraft-hub/hub && d=/var/backups/rustcraft-hub/hub-$(date +\%F).dump && docker compose exec -T db pg_dump -U hub -Fc hub > $d.part && mv $d.part $d && find /var/backups/rustcraft-hub \( -name 'hub-*.dump' -o -name 'hub-*.dump.part' \) -mtime +14 -delete
CRON
```

To restore one, with the hub stopped:

```sh
cd /srv/rustcraft-hub/hub
docker compose stop hub
docker compose exec -T db pg_restore -U hub -d hub --clean --if-exists < /var/backups/rustcraft-hub/hub-2026-11-25.dump
docker compose start hub
```

The worlds themselves are in Docker volumes (`rustcraft-realm-<uuid>`, archived ones suffixed
`-archived-<date>`), which the hub's dump does not hold: copy them with `docker run --rm -v
<volume>:/data:ro -v "$PWD:/out" debian:bookworm-slim tar czf /out/<volume>.tar.gz -C /data .`.

### Update and roll back

```sh
cd /srv/rustcraft-hub
docker compose -f hub/compose.yaml exec -T db pg_dump -U hub -Fc hub > ~/hub-before-update.dump
git rev-parse HEAD > ~/hub-before-update.sha
git pull --ff-only origin feat/hub            # or, from the local checkout: git push root@<machine>:/srv/rustcraft-hub feat/hub
cd hub && docker compose run --rm --entrypoint rustcraft-hub hub admin realms   # which worlds run
```

Building the game image takes most of the memory of a machine with no swap, and a world may be
started meanwhile, so do it only when the update changes the game's server and no world is
running (the Dockerfile has no knob to cap the build's parallelism):

```sh
cd /srv/rustcraft-hub && docker build -t rustcraft-server:hub .
cd hub && docker compose up -d --build
```

Migrations are applied at start. The worlds take their image at each start: a running world keeps
the old one until it stops (10 minutes at 0 players, or from the game), and the next start runs
the new one.

To roll back, check out the previous commit and rebuild both images:

```sh
cd /srv/rustcraft-hub && git checkout "$(cat ~/hub-before-update.sha)"
docker build -t rustcraft-server:hub .
cd hub && docker compose up -d --build
```

A migration is not undone by this. If the update applied one the old hub cannot work with, also
restore the dump taken before the update (as in step 12; the accounts made since are lost), and
`git checkout feat/hub` again once fixed (with a push as in step 2, `git checkout feat/hub`
follows the branch back).

`HUB_PUBLIC_URL`, `HUB_NAME` and `HUB_TRUSTED_RANGES` are set in the `environment` of `hub` in
`compose.yaml`.

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
