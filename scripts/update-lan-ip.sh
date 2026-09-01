#!/usr/bin/env bash
# Prints the URL friends on the same Wi-Fi should open to join a game.
#
# The frontend no longer stores the LAN IP anywhere: the browser only talks to
# the Vite dev server's own origin, which proxies /api, /broadcasting and the
# /app WebSocket to the backend & Reverb containers. So there is nothing to
# regenerate when the network changes — this script is purely informational.
set -euo pipefail

detect_ip() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        local iface
        iface=$(route -n get default 2>/dev/null | awk '/interface: /{print $2}')
        if [[ -n "$iface" ]]; then
            ipconfig getifaddr "$iface" 2>/dev/null && return
        fi
    else
        ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if ($i=="src") print $(i+1)}' && return
        hostname -I 2>/dev/null | awk '{print $1}' && return
    fi
}

IP="$(detect_ip || true)"

if [[ -z "$IP" ]]; then
    echo "Impossible de détecter l'IP LAN automatiquement." >&2
    echo "Trouve-la à la main (Préférences Réseau) — tes amis ouvriront http://<IP>:5173" >&2
    exit 0
fi

echo "IP LAN détectée : $IP"
echo "Tes amis rejoignent la partie via : http://${IP}:5173"
