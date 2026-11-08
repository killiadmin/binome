# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository layout

This is a monorepo with two independent apps, no shared tooling between them:

- `backend/` — Laravel 11 API + Laravel Reverb WebSocket server
- `frontend/` — Vue 3 (Composition API, `<script setup>`) SPA built with Vite

Full architecture docs already exist and are the source of truth — read them before making non-trivial changes:
- `backend/README.md` — game rules, data model, services, controllers, WebSocket events, full game flow
- `frontend/README.md` — Vue structure, `useReverb` WebSocket singleton, session persistence, page behavior, front-end flow

Do not duplicate that documentation here; this file only covers commands and things not already written down.

## Commands

### Docker (primary way to run the whole stack)

The entire stack — MySQL, Laravel (`artisan serve`), Reverb, a queue worker, and the Vite dev server — is orchestrated from the repo root via `docker-compose.yml`, one service per concern (`database`, `backend`, `reverb`, `queue`, `frontend`), sharing a `binome` bridge network. Backend/frontend containers bind-mount their source directories for hot reload; `vendor` and `node_modules` live in named volumes so container-installed dependencies aren't shadowed by the host bind mount.

```bash
./start.sh              # `docker compose up --build` + prints the LAN join URL
docker compose up -d    # equivalent; the frontend proxy makes the LAN IP a non-issue
docker compose logs -f backend
docker compose down
```

Friends on the same Wi-Fi reach everything through the Vite dev server, which serves the whole app under `/binome/` (Vite `base`). They open `http://killiadmin.local:8095/binome/`: port 8095 belongs to the shared nginx portal in `../portail`, which forwards `/binome/` (prefix kept, WebSockets upgraded) to `binome_frontend:5173` over the shared Docker network `meszappli` (only the `frontend` service joins it); `http://localhost:5173/binome/` still works directly. The browser only ever talks to that one origin. `frontend/vite.config.js` proxies `/binome/api` + `/binome/broadcasting` to the `backend` container and the `/binome/app` WebSocket (`ws: true`) to the `reverb` container, by Docker service name, stripping the prefix — the backend and Reverb know nothing about it, except image URLs the backend hands to the browser (`PlayerAvatar::url()`, `CustomReaction::url()`, `Character::imageUrl()`), which carry `config('app.public_prefix')` (`APP_PUBLIC_PREFIX`, default `/binome`). Frontend code must build URLs from `import.meta.env.BASE_URL` (router history, axios `baseURL`, Echo `wsPath` / `authEndpoint`, `lan-url.json`), never from `/`. `server.allowedHosts: ['.local']` lets the portal forward the Bonjour host name. So the host's LAN IP appears in **no** frontend proxy config and switching networks needs no regeneration — `frontend/.env` only carries `VITE_REVERB_APP_KEY`. `scripts/update-lan-ip.sh` (run by `./start.sh`) prints the join URL and writes it to `frontend/public/lan-url.json` (gitignored, machine-specific); `./start.sh` also keeps `update-lan-ip.sh --watch` running in the background (PID in `.lan-watch.pid`) so the file follows Wi-Fi changes. The Navbar's "Partager le lien" button (`frontend/src/services/joinUrl.js`) re-reads that file on every click when the host browses `localhost:5173`, probes the LAN URL (`no-cors` fetch) before sharing it, and uses native share on touch devices / clipboard otherwise. A plain `docker compose up` has no watcher, so the link goes stale after a network change until `./start.sh` or `scripts/update-lan-ip.sh` is rerun. Auto-detection picks the default-route IP; override with `LAN_IP=192.168.64.1 ./start.sh` or `scripts/update-lan-ip.sh <ip>` when the right interface isn't the default route (VM/host-only networks). Running the frontend outside Docker (`npm run dev`) is the one case that needs `VITE_PROXY_BACKEND` / `VITE_PROXY_REVERB` overrides, since the service names won't resolve.

`backend/.env`'s `REVERB_HOST=reverb` is unrelated: it's the internal service name the PHP process uses to publish events to the Reverb container over the compose network.

### Backend (`backend/`)

```bash
composer install
cp .env.example .env && php artisan key:generate
php artisan migrate

# Import de personnages en masse (JSON) — voir backend/README.md pour le format
php artisan characters:import database/data/characters.json
php artisan db:seed --class=CharacterSeeder   # même import via database/data/characters.json

# Tests
php artisan test
php artisan test --filter=TestName
vendor/bin/phpunit tests/Feature/SomeTest.php

# Lint / format (Pint)
vendor/bin/pint

# Run without Docker (3 terminals) — DB_HOST/DB_PORT and REVERB_HOST in .env
# must then point at reachable hosts (e.g. 127.0.0.1:9090), not the Docker
# service names committed by default:
php artisan serve --host=0.0.0.0
php artisan reverb:start --host=0.0.0.0 --debug
php artisan queue:listen --tries=1
```

There is no real test suite yet (only the default `ExampleTest` files in `tests/Unit` and `tests/Feature`). `phpunit.xml` has sqlite-in-memory env vars commented out — tests run against whatever `DB_CONNECTION` is active, not an isolated DB.

### Frontend (`frontend/`)

```bash
npm install
npm run dev -- --host   # --host exposes Vite on the LAN, needed for multi-device testing
npm run build
npm run preview
```

Requires a `frontend/.env` with just `VITE_REVERB_APP_KEY` (must match the backend's `REVERB_APP_KEY`); everything else goes through the dev-server proxy (see the Docker section). If env changes aren't picked up, clear Vite's cache: `rm -rf node_modules/.vite`. Note: `docker compose restart frontend` does **not** reload `env_file` changes — use `docker compose up -d --force-recreate frontend`.

## Architecture essentials

The game ("Binome"): players are secretly paired into binomes sharing a universe/character theme; each has forbidden words tied to their character; players take turns asking questions or making accusations; the game ends when only one binome remains undiscovered.

- Backend logic is orchestrated through `app/Services/`: `GameService` (game start, binome/character assignment — all in one `DB::transaction()`), `RoundService` (turn order, deterministic by `player.id`), `ActionService` (question/accusation validation, forbidden-word checks), `BinomeService` (binome discovery, win condition).
- Real-time state is pushed via Laravel Reverb over two presence channels: `room.{roomId}` (lobby) and `game.{gameId}` (in-game). All backend events use `ShouldBroadcastNow` (bypasses the queue) — if an event isn't showing up on the frontend, check it wasn't switched to `ShouldBroadcast`.
- WebSocket auth is **not** Sanctum-based: a custom `X-Player-Id` header is read by `BroadcastAuthController`, which does `auth()->setUser($player)` before calling `Broadcast::auth()`. The default Laravel `broadcasting/auth` route is disabled; the custom one is registered manually in `AppServiceProvider::boot()` (and exempted from CSRF in `bootstrap/app.php`), with `routes/channels.php` required manually too.
- `Player` extends `Authenticatable` specifically so `auth()->setUser()` works for this custom auth flow.
- Frontend WebSocket access goes entirely through `frontend/src/sockets/useReverb.js`, a singleton Echo instance keyed to the current `playerId`. It must be instantiated inside a function (never at the top level of `<script setup>`), and `resetEcho()` must be called after restoring a session/refresh or the singleton will keep stale auth headers and get 403'd.
- Frontend session state (`roomId`, `gameCode`, `playerId`, `hostId`, `isHost`) is persisted to `localStorage` under the `session` key and restored on mount in `RoomPage.vue`.
- API security invariant: `GET /games/{game}` never returns player characters; `GET /games/{game}/me` returns a character only to the requesting player; characters are otherwise only revealed via the `BinomeDiscovered` and `GameEnded` broadcast events — and, when the host enables it, to **eliminated** players via `GET /games/{game}/reveal`.
