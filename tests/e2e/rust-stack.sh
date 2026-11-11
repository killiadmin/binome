#!/usr/bin/env bash
# Serveur Rust de test, sur une COPIE de la base (jamais la base réelle).
#
#   tests/e2e/rust-stack.sh prepare     # base SQLite de référence : export de la pile de
#                                       # référence (ref-stack.sh up) puis import Rust
#   tests/e2e/rust-stack.sh up [release]  # compile puis lance sur 127.0.0.1:18101/binome
#     STATIC_DIR=chemin/dist …            # sert aussi le front compilé (comme en production)
#   tests/e2e/rust-stack.sh down
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
data="$here/.data"
mkdir -p "$data"

# Mot de passe admin repris du .env existant, dans un fichier privé (jamais affiché)
env_file() {
  umask 077
  grep '^CHARACTERS_ACCESS_PASSWORD=' "$root/backend/.env" > "$data/rust.env" || true
}

case "${1:-}" in
  prepare)
    rm -rf "$data/export" "$data/base.sqlite"*
    "$root/scripts/export-mysql.sh" "$data/export" binome_ref-database-1
    CARGO_ENV="-v $data:/work -e BINOME_DATABASE=/work/base.sqlite" "$root/server/cargo.sh" run -q -- import-mysql /work/export
    ;;
  up)
    profile="${2:-debug}"
    flag=""; [[ "$profile" == release ]] && flag="--release"
    "$root/server/cargo.sh" build $flag -q
    [[ -f "$data/base.sqlite" ]] || { echo "Base absente : lance d'abord « $0 prepare »" >&2; exit 1; }
    env_file
    docker rm -f binome_rust_test >/dev/null 2>&1 || true
    rm -f "$data/run.sqlite"*
    cp "$data/base.sqlite" "$data/run.sqlite"
    static=()
    [[ -n "${STATIC_DIR:-}" ]] && static=(-v "$STATIC_DIR:/static:ro" -e BINOME_STATIC_DIR=/static)
    docker run -d --name binome_rust_test -p 18101:8080 \
      -v binome_cargo_target:/target:ro -v "$data:/work" ${static[@]+"${static[@]}"} \
      --env-file "$data/rust.env" -e BINOME_DATABASE=/work/run.sqlite -e RUST_LOG="${RUST_LOG:-binome=info}" \
      rust:1-bookworm "/target/$profile/binome" serve >/dev/null
    for _ in $(seq 1 30); do
      curl -fsS http://127.0.0.1:18101/binome/up >/dev/null 2>&1 && { echo "Serveur Rust ($profile) prêt : http://127.0.0.1:18101/binome"; exit 0; }
      sleep 0.5
    done
    docker logs binome_rust_test >&2; exit 1
    ;;
  down)
    docker rm -f binome_rust_test >/dev/null 2>&1 || true
    ;;
  *) echo "usage: $0 prepare | up [release] | down" >&2; exit 2 ;;
esac
