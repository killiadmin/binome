#!/usr/bin/env bash
# Cargo dans Docker (pas de toolchain Rust sur l'hôte) : registre et dossier
# target dans des volumes nommés, donc compilations incrémentales.
#   server/cargo.sh build | test | clippy | run -- serve ...
# Variables : CARGO_PUBLISH="-p 18101:8080" pour exposer le serveur lancé par `run`.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
exec docker run --rm ${CARGO_INTERACTIVE:+-it} ${CARGO_PUBLISH:-} ${CARGO_NAME:+--name "$CARGO_NAME"} \
  -v "$PWD":/app -w /app \
  -v binome_cargo_registry:/usr/local/cargo/registry \
  -v binome_cargo_target:/app/target \
  ${CARGO_ENV:-} \
  rust:1-bookworm cargo "$@"
