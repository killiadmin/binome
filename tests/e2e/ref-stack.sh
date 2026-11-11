#!/usr/bin/env bash
# Pile Laravel de référence jetable (voir docker-compose.ref.yml).
#   tests/e2e/ref-stack.sh up [dump.sql]   # défaut : dernier dump de ~/Meszappli/backups/binome
#   tests/e2e/ref-stack.sh down
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

case "${1:-}" in
  up)
    dump="${2:-$(ls -1d "$HOME"/Meszappli/backups/binome/*/binome.sql 2>/dev/null | sort | tail -1)}"
    [[ -f "$dump" ]] || { echo "Dump introuvable : '$dump'" >&2; exit 1; }
    echo "Référence restaurée depuis : $dump"
    REF_DUMP="$dump" docker compose -f docker-compose.ref.yml up -d --wait
    # Laravel répond-il ? (la première requête compile les routes)
    for _ in $(seq 1 30); do
      curl -fsS http://127.0.0.1:18001/api/cosmos >/dev/null 2>&1 && { echo "API de référence prête : http://127.0.0.1:18001"; exit 0; }
      sleep 1
    done
    echo "L'API de référence ne répond pas" >&2; exit 1
    ;;
  down)
    REF_DUMP=/dev/null docker compose -f docker-compose.ref.yml down --remove-orphans
    ;;
  *)
    echo "usage: $0 up [dump.sql] | down" >&2; exit 2
    ;;
esac
