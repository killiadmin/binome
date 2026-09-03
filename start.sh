#!/usr/bin/env bash
# Detects the current LAN IP and starts the full stack (database, backend,
# reverb, queue, frontend) via Docker Compose.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# LAN_IP="192.168.64.1" ./start.sh  → force l'IP annoncée par le bouton « Copier le lien »
"$ROOT_DIR/scripts/update-lan-ip.sh"

cd "$ROOT_DIR"
docker compose up --build "$@"
