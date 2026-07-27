#!/usr/bin/env bash
# Detects this machine's current LAN IP (the one reachable by devices on the
# same Wi-Fi/network) and writes it into frontend/.env so friends' browsers
# can reach the API and the Reverb WebSocket server through the Docker
# containers. Re-run this any time you switch networks.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT_DIR/frontend/.env"

detect_ip() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        local iface
        iface=$(route -n get default 2>/dev/null | awk '/interface: /{print $2}')
        if [[ -n "$iface" ]]; then
            ipconfig getifaddr "$iface" 2>/dev/null && return
        fi
    else
        # Linux: IP of the interface used for the default route.
        ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if ($i=="src") print $(i+1)}' && return
        hostname -I 2>/dev/null | awk '{print $1}' && return
    fi
}

IP="$(detect_ip || true)"

if [[ -z "$IP" ]]; then
    echo "Impossible de détecter automatiquement l'IP LAN." >&2
    echo "Renseigne-la manuellement dans frontend/.env (VITE_API_URL, VITE_BACKEND_URL, VITE_REVERB_HOST)." >&2
    exit 1
fi

echo "IP LAN détectée : $IP"

touch "$ENV_FILE"

update_var() {
    local key="$1" value="$2"
    if grep -q "^${key}=" "$ENV_FILE" 2>/dev/null; then
        sed -i.bak "s|^${key}=.*|${key}=${value}|" "$ENV_FILE"
        rm -f "${ENV_FILE}.bak"
    else
        printf '%s=%s\n' "$key" "$value" >> "$ENV_FILE"
    fi
}

update_var "VITE_API_URL" "http://${IP}:8001/api"
update_var "VITE_BACKEND_URL" "http://${IP}:8001"
update_var "VITE_REVERB_HOST" "${IP}"

echo "frontend/.env mis à jour :"
echo "  VITE_API_URL=http://${IP}:8001/api"
echo "  VITE_BACKEND_URL=http://${IP}:8001"
echo "  VITE_REVERB_HOST=${IP}"
echo
echo "Tes amis pourront rejoindre la partie via : http://${IP}:5173"
