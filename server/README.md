# Serveur Binome (Rust)

Un seul binaire : API REST, WebSocket temps réel, minuteries (expiration des tirages de
carte) et fichiers statiques du front compilé. Base SQLite embarquée.
A remplacé Laravel + Reverb + le worker de queue + MySQL (migration de novembre 2026).

Architecture et choix : `docs/rust-migration/01-audit-et-architecture.md`.
Contrat d'API : `02-contrat-api.md` · events : `03-events.md` · protocole WS : `04-protocole-ws.md`.

## Commandes

Pas de toolchain Rust sur l'hôte : `cargo.sh` lance Cargo dans Docker (cache dans des volumes).

```bash
server/cargo.sh build [--release]
server/cargo.sh test                       # tests unitaires (règles, parité du détecteur JS…)
tests/e2e/rust-stack.sh prepare            # base de test = copie à chaud de la base en service
tests/e2e/rust-stack.sh up [release]       # serveur de test sur 127.0.0.1:18101/binome
node tests/e2e/run.mjs                     # banc E2E complet
node tests/e2e/compare.mjs                 # contrat ↔ contrat figé de l'ancien backend
```

Sous-commandes du binaire :

```bash
binome serve                                   # défaut
binome healthcheck                             # pour Docker (image sans shell)
binome import-characters <f.json> [--images-dir D] [--skip-existing]
binome list-characters                         # catalogue complet, refusés compris
binome count-pending                           # binômes en attente de validation
binome backup <fichier.sqlite>                 # copie cohérente, serveur en marche
binome check                                   # intégrité, version du schéma, volumes (BINOME_DATABASE)
binome hash-password [--iterations N]          # empreinte du mot de passe de suppression (lu sur stdin)
```

La collation `BINOME_CI` (pseudos insensibles à la casse et aux accents) n'existe que dans le
binaire : l'outil `sqlite3` standard lit la base mais ne peut ni la contrôler ni requêter ces
colonnes. Utiliser `binome check`.

Flux de création de personnages : `docs/generation-personnages.md` (page admin « Prompts »).
`import-characters` n'accepte pas d'image par URL : déposer le fichier dans le dossier d'images.

## Configuration (variables d'environnement)

| Variable | Défaut | Rôle |
|---|---|---|
| `BINOME_BIND` | `0.0.0.0:8080` | Adresse d'écoute |
| `BINOME_DATABASE` | `/data/binome.sqlite` | Fichier SQLite (créé avec son schéma s'il n'existe pas) |
| `BINOME_STATIC_DIR` | — | Dossier du front compilé ; absent : API seule (dev avec Vite) |
| `BINOME_PUBLIC_PREFIX` | `/binome` | Préfixe de toutes les routes et des URLs d'images |
| `BINOME_ADMIN_PASSWORD` (ou `CHARACTERS_ACCESS_PASSWORD`) | — | Mot de passe d'administration ; vide = admin fermée |
| `BINOME_DELETE_PASSWORD_HASH` | — | Empreinte du mot de passe exigé pour supprimer des parties ; absente = suppression désactivée |
| `BINOME_LAN_URL_FILE` | — | `lan-url.json` du bouton « Partager le lien » (réseau local) |
| `RUST_LOG` | `binome=info` | Journaux |

## Organisation

```
src/
├── main.rs         sous-commandes, démarrage, arrêt propre
├── config.rs       variables d'environnement
├── error.rs        erreurs → réponses au format du front (422 errors, 409, 403, 404, 429)
├── clock.rs        dates (stockage UTC, formats iso8601 / eloquent du contrat)
├── db.rs           SQLite : 1 écrivain (transactions IMMEDIATE) + lecteurs WAL, collation BINOME_CI
├── state.rs        état partagé
├── ratelimit.rs    limites de débit en mémoire (par joueur)
├── timers.rs       expiration des tirages de carte (+ réarmement au démarrage)
├── tools.rs        commandes du catalogue (import, liste, binômes en attente)
├── http/           routes (mod.rs) et lecture/validation des entrées (input.rs)
├── realtime/       hub en mémoire (canaux, présence) et WebSocket
├── services/       logique métier : rooms, game, play, score, history, catalog, social, admin
└── support/        texte (collation, slugs), images, détecteur de mots interdits
```

Schéma : `migrations/0001_schema.sql` puis migrations numérotées (`0002_…`), appliquées une
fois au démarrage (`PRAGMA user_version`), chacune dans une transaction avec contrôle des clés
étrangères. Ajouter un fichier `000N_*.sql` et l'inscrire dans `MIGRATIONS` (`src/db.rs`).

Chaque opération métier s'exécute dans **une** transaction et renvoie la réponse HTTP + les
events, publiés **après** le commit.
