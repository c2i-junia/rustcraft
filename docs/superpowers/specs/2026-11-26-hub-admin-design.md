# Hub web admin — design

Status: approved by the user on 2026-11-26 ("ok c'est bon, vas-y"). Branch `feat/hub`.

## Goal

A complete web interface for administering the hub, so that the user no longer needs
`rustcraft-hub admin …` over SSH. Everything the admin CLI does, plus a read view of sessions,
worlds and the audit log. Built in Svelte 5 with UI SDK (`@ui-sdk/svelte`, https://ui-sdk.dev/).

## Decisions

- **Served by the hub, at `https://hub.rustcraft.dev/admin/`** — same origin as the admin API, so
  the session is an HttpOnly cookie, the hub sends no CORS headers, and Cloudflare (which proxies
  rustcraft.dev) is never between the browser and a password. rustcraft.dev only links to it.
- **Source in `hub/admin/`** (a SvelteKit app, adapter-static, `paths.base = "/admin"`), built
  into the hub image. Ruling: not in `website/` — the site is deployed from `main` by
  `/srv/rustcraft`; the admin ships with the hub image, from the hub's build — cost if wrong: move
  a folder.
- **Login = a hub account with the `admin` role**, by username and password, through the same
  `authenticate` path as the game (same per-account/per-IP failed-login limits, same audit of
  failures). A correct password of a non-admin is refused with 403 `forbidden`
  ("This account is not an admin."), written in the audit log as `admin_login_refused`.

## Sessions of the web admin

- Migration `0003_admin_web.sql`: `ALTER TABLE sessions ADD COLUMN kind text NOT NULL DEFAULT
  'game' CHECK (kind IN ('game', 'admin_web'))`.
- The game's `Session` extractor (Bearer) only accepts `kind = 'game'`. The new `AdminSession`
  extractor only accepts the cookie, `kind = 'admin_web'`, an `active` account, and **the account
  must hold `admin` at the time of each request** (a revoked role ends access at once).
- Idle lifetime **12 hours**, absolute lifetime **24 hours** from `created_at`: each use sets
  `expires_at = LEAST(now() + 12 h, created_at + 24 h)`.
- Cookie `__Host-rustcraft-admin=<base64url token>; Path=/; Secure; HttpOnly; SameSite=Strict;
  Max-Age=86400`. Logout deletes the session row and clears the cookie (`Max-Age=0`).
- Password change, reset, suspension and `revoke_access` already delete every session of the
  account, web ones included.

## CSRF and browser hardening

- Every `/v1/admin/*` request that is not GET/HEAD must carry `X-Rustcraft-Admin: 1`, else 403.
  (A cross-origin page cannot add that header without a CORS preflight, which the hub never
  answers.)
- If `Origin` is present it must equal the origin of `--public-url`; if `Sec-Fetch-Site` is
  present it must be `same-origin` or `none`. Otherwise 403. This applies to all `/v1/admin/*`
  requests, GET included.
- All `/v1/admin/*` responses: `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`.
- `/admin/*` (the static files): `Content-Security-Policy: default-src 'self'; script-src 'self'
  <hashes>; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self';
  frame-ancestors 'none'; base-uri 'none'; form-action 'self'; object-src 'none'` — the inline
  bootstrap script of SvelteKit is covered by `kit.csp` (mode `hash`, a `<meta>` in the page); the
  hub's header adds what a meta cannot (`frame-ancestors`). Plus `X-Frame-Options: DENY`,
  `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`. `index.html` is
  `Cache-Control: no-cache`; `_app/immutable/*` is `public, max-age=31536000, immutable`.
- The static files come from `--admin-dir` / `HUB_ADMIN_DIR` (unset: no `/admin`, 404). Unknown
  paths under `/admin/` that have no file fall back to `index.html` (client routing). No
  directory listing; no path outside the directory.

## Admin API (`/v1/admin`, JSON, errors as the existing `AppError` body)

| Method & path | Does |
|---|---|
| `POST /v1/admin/session` `{username,password}` | Logs in (sets the cookie) → `{username}`; audit `admin_login` |
| `DELETE /v1/admin/session` | Logs out |
| `GET /v1/admin/me` | `{id, username}` of the admin |
| `GET /v1/admin/overview` | counts: accounts by status, admins, hosts, sessions alive (by kind), sign-ups in 24 h and 7 d, failed logins in 24 h, worlds by state, running vs `max_running` |
| `GET /v1/admin/accounts?q=&status=&role=&offset=&limit=` | page of accounts (q = username contains, case-insensitive; limit ≤ 100, default 50), newest first, with `total` |
| `GET /v1/admin/accounts/{id}` | account + roles + quota + status/reason + sessions (kind, device, created, last used, expires — never the token) + active certificate count + worlds it owns + its last 50 audit rows |
| `PUT /v1/admin/accounts/{id}/roles/{admin\|host}` `{realm_quota?}` | grant (quota only with host, default 1, ≥ 0) |
| `DELETE /v1/admin/accounts/{id}/roles/{admin\|host}` | revoke |
| `PUT /v1/admin/accounts/{id}/quota` `{realm_quota}` | set the quota of worlds (0..=100) |
| `POST /v1/admin/accounts/{id}/suspend` `{reason}` | suspend (1..=200 chars), revokes sessions and certificates |
| `POST /v1/admin/accounts/{id}/unsuspend` | |
| `POST /v1/admin/accounts/{id}/reset-password` | → `{password}` (temporary, shown once), ends sessions, revokes certificates |
| `DELETE /v1/admin/accounts/{id}/sessions` | ends every session and revokes certificates |
| `GET /v1/admin/realms?state=` | every world (archived only when asked) with owner, state, slot, players, last played, failed reason |
| `POST /v1/admin/realms/{id}/stop` | stop in the background (202) |
| `DELETE /v1/admin/realms/{id}` | archive (204) |
| `GET /v1/admin/audit?account=&action=&before=&limit=` | audit rows newest first, keyset paging by id (limit ≤ 200) |

- **Self-protection:** an admin cannot revoke their own `admin`, suspend themselves, reset their
  own password or end their own sessions from the web (409 `conflict`-like error, clear message).
  Deleted accounts: every mutation is refused.
- **Audit:** every mutation writes `audit_log` with `actor = 'admin:<admin username>'` and
  `details.admin_id = <uuid>`, same action names as the CLI (`role_granted`, `role_revoked`,
  `suspended`, `unsuspended`, `password_reset`, `realm_quota_set`, `sessions_ended`,
  `realm_stopped`, `realm_archived`, `admin_login`, `admin_logout`).
- The CLI and the web share one implementation: `admin.rs` functions take the actor and return
  data; the CLI formats text from it. CLI behaviour and output stay as they are.
- Body limit 16 KiB like the account routes. Unknown JSON fields refused.

## The interface (`hub/admin/`)

SvelteKit + Svelte 5 + TypeScript, adapter-static (`fallback: "index.html"`, `ssr = false`),
`@ui-sdk/svelte` + `@ui-sdk/core` (Apache-2.0, pinned exact version) for the interactive parts
(Button, Input, Password Input, Field, Dialog, Alert Dialog, Select, Tabs, Toast, Pagination,
Menu, Tooltip, Switch, Number Input); own CSS for layout, tables, cards and badges (UI SDK has
none). Light and dark (prefers-color-scheme), the Pixelify Sans title of the site, otherwise a
system font. No external requests: fonts and everything bundled.

Pages:
- **Login** — username, password, errors from the hub (bad credentials, rate limit with its wait,
  not an admin, suspended).
- **Overview** — the counts as cards, running worlds vs max, last 10 audit rows.
- **Accounts** — search box (debounced), status and role filters, paged table (name, roles as
  badges, quota, status, created). Row → detail.
- **Account detail** — header with status badge; actions: grant/revoke admin and host, quota
  (number input), suspend (dialog with reason), unsuspend, reset password (alert dialog → then a
  dialog showing the temporary password once, with a copy button and "it will not be shown
  again"), end all sessions (alert dialog). Sections: sessions table, worlds owned, audit rows.
  Actions on oneself are disabled with a tooltip saying why.
- **Worlds** — table with state filter (archived hidden by default), stop and archive (alert
  dialog naming the world), owner links to the account.
- **Audit log** — filters (account, action), "load more" paging.
- Header: hub name, nav, the admin's name, log out. A 401 from any call returns to the login page;
  403 shows the hub's message.

Every request: `fetch(url, { credentials: "same-origin", headers: { "X-Rustcraft-Admin": "1",
… } })`. No token is ever stored by the page (cookie only). No `{@html}`.

## Deployment

- `hub/Dockerfile`: a stage `admin-ui` (`oven/bun:1` pinned by version) runs `bun install
  --frozen-lockfile && bun run build` in `hub/admin`; the `hub` target copies the build to
  `/usr/share/rustcraft-hub/admin`; compose sets `HUB_ADMIN_DIR` to it.
- Caddy is unchanged (it proxies all of hub.rustcraft.dev).
- The site (`website/`) gets a discreet "Admin" link to `https://hub.rustcraft.dev/admin/` in its
  footer — only visible after `main` gets this branch.

## Tests

`hub/tests/admin_web.rs` against the test database: login (admin ok, non-admin 403 after a
correct password, wrong password counted by the limiter, suspended), cookie attributes, bearer
game token refused on `/v1/admin`, admin cookie token refused as a game Bearer token, missing
`X-Rustcraft-Admin` → 403, foreign `Origin` → 403, `Sec-Fetch-Site: cross-site` → 403, idle and
absolute expiry, role revoked mid-session → 403, every mutation and its audit row (actor), self
protection, deleted accounts refused, paging bounds, the static files (fallback, no traversal,
headers, 404 without `--admin-dir`). The interface: `svelte-check` clean and a build; then an
end-to-end pass in a browser against a local hub with test accounts.
