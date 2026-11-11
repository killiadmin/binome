# syntax=docker/dockerfile:1
#
# Image de production de Binome : un binaire Rust statique + le front compilé,
# dans une image vide (`scratch`) : ni shell, ni Node, ni PHP, ni base externe.
#
#   docker compose build                    (voir docker-compose.yml)
#   docker build --build-arg PUBLIC_PREFIX=/ -t binome .   # application à la racine du domaine

ARG PUBLIC_PREFIX=/binome

# ─── 1. Front (Vue) → fichiers statiques, précompressés en gzip ──────────────
FROM node:22-alpine AS front
WORKDIR /front
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY frontend/ ./
ARG PUBLIC_PREFIX
RUN rm -f public/lan-url.json \
 && VITE_BASE="$PUBLIC_PREFIX" npm run build \
 && find dist -type f \( -name '*.js' -o -name '*.css' -o -name '*.html' -o -name '*.svg' -o -name '*.json' -o -name '*.webmanifest' \) \
      -size +1k -exec gzip -9 -k {} \;

# ─── 2. Serveur (Rust) → binaire statique (musl) ─────────────────────────────
FROM rust:1-alpine AS server
RUN apk add --no-cache musl-dev gcc
WORKDIR /src
# Dépendances d'abord : recompilées seulement quand Cargo.lock change
COPY server/Cargo.toml server/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release --locked && rm -rf src
COPY server/src ./src
COPY server/migrations ./migrations
RUN touch src/main.rs && cargo build --release --locked \
 && mkdir -p /out/data && cp target/release/binome /out/binome

# ─── 3. Exécution : image vide ───────────────────────────────────────────────
FROM scratch
COPY --from=server /out/binome /binome
COPY --from=server --chown=10001:10001 /out/data /data
COPY --from=front /front/dist /static
ARG PUBLIC_PREFIX
ENV BINOME_BIND=0.0.0.0:8080 \
    BINOME_DATABASE=/data/binome.sqlite \
    BINOME_STATIC_DIR=/static \
    BINOME_PUBLIC_PREFIX=${PUBLIC_PREFIX}
USER 10001:10001
EXPOSE 8080
VOLUME /data
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 CMD ["/binome", "healthcheck"]
ENTRYPOINT ["/binome"]
CMD ["serve"]
