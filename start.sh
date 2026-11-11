#!/usr/bin/env bash
# Réseau local : détecte l'IP LAN (lien « Partager le lien ») puis lance Binome
# (un seul conteneur : serveur Rust + front compilé) derrière le portail ../portail.
#   ./start.sh [-d]     →  http://killiadmin.local:8095/binome/  (et localhost:8080/binome/)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WATCH_PID_FILE="$ROOT_DIR/.lan-watch.pid"

# LAN_IP="192.168.64.1" ./start.sh  → force l'IP annoncée par le bouton « Partager le lien »
"$ROOT_DIR/scripts/update-lan-ip.sh"

# Un seul watcher à la fois : on arrête celui d'un lancement précédent.
if [[ -f "$WATCH_PID_FILE" ]]; then
    kill "$(cat "$WATCH_PID_FILE")" 2>/dev/null || true
    rm -f "$WATCH_PID_FILE"
fi

# Sans IP forcée, on surveille le réseau pendant toute la session : si l'hôte
# change de Wi-Fi, lan-url.json est réécrit et le bouton de partage suit.
if [[ -z "${LAN_IP:-}" ]]; then
    DETACHED=false
    for arg in "$@"; do
        [[ "$arg" == "-d" || "$arg" == "--detach" ]] && DETACHED=true
    done

    if $DETACHED; then
        # « ./start.sh -d » rend la main tout de suite : le watcher survit en
        # arrière-plan (arrêt : kill $(cat .lan-watch.pid)).
        nohup "$ROOT_DIR/scripts/update-lan-ip.sh" --watch > /dev/null 2>&1 &
        echo $! > "$WATCH_PID_FILE"
    else
        "$ROOT_DIR/scripts/update-lan-ip.sh" --watch &
        echo $! > "$WATCH_PID_FILE"
        trap 'kill "$(cat "$WATCH_PID_FILE")" 2>/dev/null || true; rm -f "$WATCH_PID_FILE"' EXIT
    fi
fi

cd "$ROOT_DIR"
docker compose -f docker-compose.yml -f docker-compose.portail.yml up --build "$@"
