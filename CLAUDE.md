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
cp .env.example .env                       # BINOME_ADMIN_PASSWORD
docker compose up -d --build               # one container, http://localhost:8080/binome/
./start.sh                                 # same + ../portail (killiadmin.local:8095) + LAN share link
docker compose -f docker-compose.yml -f docker-compose.https.yml up -d   # VPS, HTTPS (BINOME_DOMAIN)
docker exec binome /binome backup /data/x.sqlite ; docker exec binome /binome check
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
- API security invariant: `GET /games/{game}` never returns characters; `GET /games/{game}/me`
  returns a character only to the requesting player; characters otherwise only in `game.ended`,
  to eliminated players via `GET /games/{game}/reveal` (host option), and in finished-game history.
- Identity is an unsigned `player_id` (inherited trust model, unchanged).
- Deleting games (`DELETE /admin/games/{id}`, or a player `?with_games=1`) also requires the
  `X-Delete-Password` header, checked against `BINOME_DELETE_PASSWORD_HASH` (PBKDF2; unset =
  disabled). Only the owner knows that password — never generate or store it yourself.
- Schema changes = new numbered file in `server/migrations/` registered in `MIGRATIONS`
  (`server/src/db.rs`), applied once at startup (`PRAGMA user_version`). Games survive their room
  (`games.room_id` ON DELETE SET NULL since migration 2). Back up before deploying a migration.
- JSON shapes are a contract (`tests/e2e/contracts/`): e.g. `players[].is_ready` is 0/1, two date
  formats coexist. Run `compare.mjs` after touching a response or an event.
