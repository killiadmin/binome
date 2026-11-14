# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository layout

- `server/` — Rust (Axum + Tokio + SQLite) : REST API, native WebSocket, timers, and serving
  the compiled front. One binary, one process. **This is the backend.**
- `frontend/` — Vue 3 (Composition API, `<script setup>`) SPA built with Vite.
- `tests/e2e/` — black-box regression bench (Node ≥ 22, no deps) driving the app like the front.
- `data/characters/` — character catalogue batches + images (real data, gitignored).

The app replaced a Laravel 11 + Reverb + MySQL backend in November 2026 (last version: git tag
`laravel-final`); `docs/rust-migration/` records that migration.

Source-of-truth docs — read them before non-trivial changes:
- `docs/rust-migration/` — audit & architecture (01), API contract (02, generated), events (03,
  generated), WebSocket protocol (04), performance (05), deployment (06)
- `server/README.md` — server commands, configuration, module layout
- `frontend/README.md` — Vue structure, `useReverb` WebSocket singleton, session persistence
- `docs/regles-du-jeu.md` — game rules and special behaviours (disconnects, spectators, chat, history)
- `docs/generation-personnages.md` — character generation / import / validation workflow

## Commands

### Production image / local network

```bash
cp .env.example .env                       # BINOME_ADMIN_PASSWORD_HASH (hash-password --admin)
docker compose up -d --build               # one container, http://localhost:8080/binome/
./start.sh                                 # same + ../portail (killiadmin.local:8095) + LAN share link
docker compose -f docker-compose.yml -f docker-compose.https.yml up -d   # VPS, HTTPS (BINOME_DOMAIN)
docker exec binome /binome backup /data/x.sqlite ; docker exec binome /binome check
docker exec binome /binome prune-rooms [--apply]   # dead room memberships (dry run without --apply)
docker exec binome /binome prune-players [--apply] # profiles inactive 24 months → anonymized (server also runs it daily)
docker exec binome /binome list-characters | count-pending | import-characters /import/<lot>.json
```

### Development

```bash
docker compose -f docker-compose.dev.yml up        # Vite HMR :5173 + cargo run :8080
docker compose -f docker-compose.dev.yml restart server   # after a Rust change
server/cargo.sh build | test               # cargo runs in Docker (no host toolchain)
```

### Tests

```bash
server/cargo.sh test                                   # unit tests (rules, JS parity of the matcher…)
tests/e2e/rust-stack.sh prepare && tests/e2e/rust-stack.sh up   # test server on a live-DB copy, :18101
node tests/e2e/run.mjs [--save]                        # full API + WebSocket bench (never the live DB)
node tests/e2e/compare.mjs                             # JSON contract vs frozen Laravel contract
STATIC_DIR=frontend/dist tests/e2e/rust-stack.sh up && node tests/e2e/browser.mjs   # real front in Chromium
node tests/e2e/load.mjs [--rate N]                     # load / resources
```

Wrap long measurements in `caffeinate -dims` (the Mac sleeps lid-closed and skews timings).

## Architecture essentials

Game rules: players are secretly paired into binomes sharing a universe; each has 3 forbidden
words; turns of questions / accusations; the game ends when one intact team (or one survivor)
remains. Details in `docs/regles-du-jeu.md`.

- Every business operation runs in ONE SQLite transaction (`BEGIN IMMEDIATE`, single writer) and
  returns the HTTP body + the events to publish **after commit** (`server/src/services/mod.rs`).
  Sensitive transitions stay conditional updates (`answer IS NULL`, `card_draw_status`, …).
- Real-time: in-memory hub (`server/src/realtime/`), channels `room.{id}` / `game.{id}`
  (presence, deduplicated per player), `player.{id}` (private), `lobby` (public). Authorization
  at subscribe time (room member / game player or spectator / own `player.{id}`). Presence checks (« pseudo
  already connected », « player offline ») read the hub, no HTTP call.
- Front WebSocket access goes entirely through `frontend/src/sockets/useReverb.js` (name and API
  kept from the Echo era). Instantiate it inside a function, and call `resetEcho()` after
  restoring a session or changing player.
- Pseudos are case/accent-insensitive via the SQLite collation `BINOME_CI` registered by the
  binary — the stock `sqlite3` CLI cannot check or query those columns; use `binome check`.
- Personal images are served only through signed URLs (`common::image_sig`, checklist 7 D):
  `?v=…&s=HMAC(profile secret, kind|id|date)` — avatars signed on their last change (a new or
  removed photo kills old URLs), custom emojis on their creation (renaming keeps URLs valid).
  Missing/bad signature → 404. Always build these URLs with `avatar_url` / `reaction_url`.
- API security invariant: `GET /games/{game}` never returns characters; `GET /games/{game}/me`
  returns a character only to the requesting player; characters otherwise only in `game.ended`,
  to eliminated players via `GET /games/{game}/reveal` (host option), and in finished-game history.
- Identity = signed profile token (`X-Profile-Token`, `services/profile.rs`, migration 3):
  `id.HMAC(server secret in app_secrets, id|players.token_version)`, issued by `rooms/store`,
  `rooms/join` and `/profile/login`, revoked by bumping `token_version` (password set/changed).
  Since checklist point 5 (2026-11-14) a request's `player_id` is only trusted if the token
  designates that player: middleware `identify` (`http/mod.rs`) → `input::Caller`, enforced by
  `V::player(…, "player_id")` and `Caller::require` (401 no/invalid token, 403 other player).
  Any new endpoint acting as a player must go through one of them. The WebSocket takes
  `?token=` and derives the player from it. A public (password-less) profile can still be
  opened by pseudo while its player is offline (inherited model). Players are no longer
  deleted when leaving a room.
- Admin access: only a PBKDF2 hash is configured (`BINOME_ADMIN_PASSWORD_HASH`, generated by the
  owner with `binome hash-password --admin`); the `X-Characters-Access` token is
  HMAC(hash, constant). The owner's profile pseudo is `BINOME_ADMIN_PSEUDO`: it opens only with
  the admin password, returns the admin token, and only that profile sees the navbar lock. `POST /access/characters` allows 2 attempts/minute, global bucket
  (successes refunded). The E2E test server uses throwaway passwords (`rust-stack.sh`).
- Designated admins (migration 7, `players.admin_granted_at`): only the owner (`AdminRole::Owner`,
  via `X-Characters-Access`) grants/revokes them (`PATCH /admin/players/{id}/admin`), and only
  password-protected profiles. A designated admin reaches every admin route with its profile
  token (`require_admin` → `AdminRole::Delegate`, rechecked per request) but cannot grant admins
  or delete an admin, and cannot remove its own password. JSON: `admin_role` = owner/admin/null
  (`is_admin` still means the owner).
- Admin controls (migration 6, `services/controls.rs`, tab /admin/games): enabled game modes
  (`app_settings.game_modes`), `cosmos.is_enabled` (new cosmos are born disabled), disabled
  binomes (`disabled_binomes`, universe + level) — the draw uses `controls::PLAYABLE_CHARACTER`.
  Profiles are validated by hand (`players.validated_at`, tab /admin/players): history routes and
  image emojis require a validated profile (`controls::require_validated`); admin always is.
- Universe sheets (migration 10, `catalog::cosmos_sheet_*`, tab /admin/univers): a short
  presentation per cosmos (`cosmos.description`, `cosmos.image` data URI 16/9, versioned by
  `sheet_updated_at`), public in `GET /cosmos` and `GET /cosmos-images/{id}`; shown in the lobby
  (selected cosmos + « Voir les fiches ») and in the rules (« Univers jouables », enabled cosmos).
- Erasure (GDPR, migration 9, `profile::anonymize`): a player who played cannot be deleted
  without destroying others' games (`rounds.current_player_id` / `actions.player_id` have no
  ON DELETE), so they are anonymized: pseudo → `Joueur supprimé #000123` (23 chars, untypeable),
  password/validation/admin role cleared, `token_version` bumped, `deleted_at` set; avatar,
  custom emojis, lobby messages and idea votes deleted; `game_stats` / `ideas.author_name`
  renamed. A player with no game rows is deleted outright. Used by `DELETE /profile` (self,
  password required if set), `DELETE /admin/players/{id}` (default) and the daily purge of
  profiles inactive for 24 months (`players.last_seen_at`, written at most once a day per player
  by the `identify` middleware and at login). Keep `LegalPage.vue` in line with this behaviour.
- Deleting games (`DELETE /admin/games/{id}`, or a player `?with_games=1`) also requires the
  `X-Delete-Password` header, checked against `BINOME_DELETE_PASSWORD_HASH` (PBKDF2; unset =
  disabled). Only the owner knows these passwords (admin and delete) — never generate or store them yourself.
- Schema changes = new numbered file in `server/migrations/` registered in `MIGRATIONS`
  (`server/src/db.rs`), applied once at startup (`PRAGMA user_version`). Games survive their room
  (`games.room_id` ON DELETE SET NULL since migration 2). Back up before deploying a migration.
- JSON shapes are a contract (`tests/e2e/contracts/`): e.g. `players[].is_ready` is 0/1, two date
  formats coexist. Run `compare.mjs` after touching a response or an event.
