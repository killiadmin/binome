# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository layout

- `server/` — Rust (Axum + Tokio + SQLite) : REST API, native WebSocket, timers, and serving
  the compiled front. One binary, one process. **This is the backend.**
- `frontend/` — Vue 3 (Composition API, `<script setup>`) SPA built with Vite.
- `tests/e2e/` — black-box regression bench (Node ≥ 22, no deps) driving the app like the front.
- `backend/` — the former Laravel 11 + Reverb backend, **kept only as the behavioural reference**
  until the migration's phase 7; not run in production, do not develop features there.

Source-of-truth docs — read them before non-trivial changes:
- `docs/rust-migration/` — audit & architecture (01), API contract (02, generated), events (03,
  generated), WebSocket protocol (04), performance (05), deployment (06)
- `server/README.md` — server commands, configuration, module layout
- `frontend/README.md` — Vue structure, `useReverb` WebSocket singleton, session persistence
- `backend/README.md` — game rules (still accurate) and the Laravel implementation

## Commands

### Production image / local network

```bash
cp .env.example .env                       # BINOME_ADMIN_PASSWORD
docker compose up -d --build               # one container, http://localhost:8080/binome/
./start.sh                                 # same + ../portail (killiadmin.local:8095) + LAN share link
docker compose -f docker-compose.yml -f docker-compose.https.yml up -d   # VPS, HTTPS (BINOME_DOMAIN)
docker exec binome /binome backup /data/x.sqlite ; docker exec binome /binome check
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
tests/e2e/ref-stack.sh up                              # disposable Laravel reference (dump copy)
tests/e2e/rust-stack.sh prepare && tests/e2e/rust-stack.sh up   # Rust on a DB copy, :18101
node tests/e2e/run.mjs --target rust [--save]          # full API + WebSocket bench
node tests/e2e/compare.mjs laravel-ref rust            # JSON contract diff
STATIC_DIR=…/dist tests/e2e/rust-stack.sh up && node tests/e2e/browser.mjs   # real front in Chromium
node tests/e2e/load.mjs --target rust --containers <name>                    # load / resources
```

Wrap long measurements in `caffeinate -dims` (the Mac sleeps lid-closed and skews timings).

## Architecture essentials

Game rules: players are secretly paired into binomes sharing a universe; each has 3 forbidden
words; turns of questions / accusations; the game ends when one intact team (or one survivor)
remains. Details in `backend/README.md`.

- Every business operation runs in ONE SQLite transaction (`BEGIN IMMEDIATE`, single writer) and
  returns the HTTP body + the events to publish **after commit** (`server/src/services/mod.rs`).
  Sensitive transitions stay conditional updates (`answer IS NULL`, `card_draw_status`, …).
- Real-time: in-memory hub (`server/src/realtime/`), channels `room.{id}` / `game.{id}`
  (presence, deduplicated per player), `player.{id}` (private), `lobby` (public). Authorization
  at subscribe time, same rules as the old `routes/channels.php`. Presence checks (« pseudo
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
- JSON shapes are a contract (`tests/e2e/contracts/`): e.g. `players[].is_ready` is 0/1, two date
  formats coexist. Run `compare.mjs` after touching a response or an event.
