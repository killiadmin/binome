#!/usr/bin/env bash
# Détecte l'IP LAN de l'hôte, l'écrit dans frontend/public/lan-url.json et
# affiche l'URL que les amis sur le même Wi-Fi doivent ouvrir pour rejoindre.
#
# Le navigateur ne parle qu'à l'origine du serveur Vite, qui proxifie /api,
# /broadcasting et le WebSocket /app vers les conteneurs backend & Reverb : rien
# à régénérer côté proxy quand le réseau change. Ce fichier lan-url.json sert
# uniquement au bouton « Copier le lien » de la navbar, pour que l'hôte (qui
# ouvre souvent http://localhost:5173) puisse partager l'URL LAN réelle.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAN_FILE="$ROOT_DIR/frontend/public/lan-url.json"
PORT=5173

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

# Override manuel : « LAN_IP=192.168.64.1 ./start.sh » ou « scripts/update-lan-ip.sh 192.168.64.1 »
# utile quand la bonne interface n'est pas la route par défaut (VM, réseau hôte…).
IP="${1:-${LAN_IP:-$(detect_ip || true)}}"

if [[ -z "$IP" ]]; then
    rm -f "$LAN_FILE"
    echo "Impossible de détecter l'IP LAN automatiquement." >&2
    echo "Trouve-la à la main (Préférences Réseau) — tes amis ouvriront http://<IP>:${PORT}" >&2
    exit 0
fi

URL="http://${IP}:${PORT}/"
mkdir -p "$(dirname "$LAN_FILE")"
printf '{\n  "url": "%s",\n  "ip": "%s",\n  "port": %s,\n  "detectedAt": "%s"\n}\n' \
    "$URL" "$IP" "$PORT" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$LAN_FILE"

echo "IP LAN détectée : $IP"
echo "Tes amis rejoignent la partie via : $URL"
