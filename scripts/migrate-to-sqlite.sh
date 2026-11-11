#!/usr/bin/env bash
# Bascule des données : MySQL (ancien backend, LECTURE SEULE) → volume SQLite de
# l'image de production. À lancer une fois, image construite (docker compose build).
#
#   scripts/migrate-to-sqlite.sh [conteneur-mysql] [projet-compose]
#     défauts : binome_db, binome   → volume « binome_binome_data »
#
# Refuse d'écraser une base existante. Pour transférer ensuite sur un VPS :
#   docker run --rm -v binome_binome_data:/data -v "$PWD":/out alpine cp /data/binome.sqlite /out/
#   (puis scp, et la même commande en sens inverse sur le VPS, suivie d'un
#    `chown 10001:10001`), ou plus simplement : `binome backup` puis copie du fichier.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mysql="${1:-binome_db}"
project="${2:-binome}"
volume="${project}_binome_data"
export_dir="$(mktemp -d)"
trap 'rm -rf "$export_dir"' EXIT

docker image inspect binome:latest >/dev/null 2>&1 || { echo "Image binome:latest absente : lance « docker compose build »" >&2; exit 1; }

echo "1/3 Export de ${mysql} (lecture seule)…"
"$root/scripts/export-mysql.sh" "$export_dir" "$mysql"

echo "2/3 Import dans le volume ${volume}…"
docker volume create "$volume" >/dev/null
# Le serveur tourne sous l'utilisateur 10001 (image sans root)
docker run --rm -v "$volume:/data" alpine chown 10001:10001 /data
docker run --rm --user 10001:10001 -v "$export_dir:/export:ro" -v "$volume:/data" binome:latest import-mysql /export

echo "3/3 Contrôle d'intégrité…"
docker run --rm --user 10001:10001 -v "$volume:/data" binome:latest check
echo "Terminé. Comparer « octets d'images » ci-dessus avec la valeur de l'export (1/3)."
