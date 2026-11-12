#!/usr/bin/env bash
# Serveur Rust de test, sur une COPIE de la base (jamais la base réelle).
#
#   tests/e2e/rust-stack.sh prepare [f.sqlite]  # base de test : copie de f.sqlite, ou à défaut
#                                       # sauvegarde à chaud de la base en service (conteneur binome)
#   tests/e2e/rust-stack.sh up [release]  # compile puis lance sur 127.0.0.1:18101/binome
#     STATIC_DIR=chemin/dist …            # sert aussi le front compilé (comme en production)
#   tests/e2e/rust-stack.sh down
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
data="$here/.data"
mkdir -p "$data"

# Empreintes des mots de passe du serveur de TEST (jetables, coût réduit),
# dans un fichier privé ; le vrai mot de passe admin n'est connu que du propriétaire
env_file() {
  local profile="$1"
  umask 077
  : > "$data/rust.env"
  printf '%s\n' "${E2E_ADMIN_PASSWORD:-e2e-admin-test}" | docker run --rm -i -v binome_cargo_target:/target:ro \
    rust:1-bookworm "/target/$profile/binome" hash-password --admin --iterations 1000 >> "$data/rust.env"
  # Le banc l'envoie pour les suppressions de parties
  printf '%s\n' "${E2E_DELETE_PASSWORD:-e2e-suppression-test}" | docker run --rm -i -v binome_cargo_target:/target:ro \
    rust:1-bookworm "/target/$profile/binome" hash-password --iterations 1000 >> "$data/rust.env"
  # Profil administrateur du banc (scénario profile)
  printf 'BINOME_ADMIN_PSEUDO=%s\n' "${E2E_ADMIN_PSEUDO:-e2eAdminBanc}" >> "$data/rust.env"
}

case "${1:-}" in
  prepare)
    rm -f "$data/base.sqlite"*
    if [[ -n "${2:-}" ]]; then
      cp "$2" "$data/base.sqlite"
    else
      # Copie cohérente de la base en service (VACUUM INTO), sortie du volume puis effacée
      name="e2e-base-$$.sqlite"
      docker exec binome /binome backup "/data/$name"
      docker run --rm -v binome_binome_data:/data -v "$data:/out" alpine sh -c "mv /data/$name /out/base.sqlite && chown $(id -u):$(id -g) /out/base.sqlite"
    fi
    echo "Base de test prête : $data/base.sqlite"
    ;;
  up)
    profile="${2:-debug}"
    flag=""; [[ "$profile" == release ]] && flag="--release"
    "$root/server/cargo.sh" build $flag -q
    [[ -f "$data/base.sqlite" ]] || { echo "Base absente : lance d'abord « $0 prepare »" >&2; exit 1; }
    env_file "$profile"
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
