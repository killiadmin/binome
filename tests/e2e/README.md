# Banc E2E — migration Rust

Banc de non-régression **boîte noire** : il pilote le jeu comme le front (HTTP + WebSocket)
et vérifie les règles, les events reçus par chaque joueur et la forme exacte des JSON.
Le même banc tourne contre Laravel (référence) puis contre Rust. Il n'utilise que Node ≥ 22
(`fetch` et `WebSocket` intégrés), sans dépendance npm.

> ⚠ Le banc crée, modifie et supprime des données. Il ne vise **jamais** la pile en service
> (ports 8001 / 5173) : uniquement des cibles jetables restaurées depuis un dump.

## Pile de référence (Laravel)

```bash
tests/e2e/ref-stack.sh up        # dernier dump de ~/Meszappli/backups/binome/*/binome.sql
tests/e2e/ref-stack.sh up chemin/dump.sql
tests/e2e/ref-stack.sh down      # tout disparaît (MySQL en tmpfs)
```

API sur `127.0.0.1:18001`, Reverb sur `127.0.0.1:18080`. Réseau Docker isolé : la pile en
service n'est pas touchée.

## Lancer

```bash
node tests/e2e/run.mjs --target laravel-ref                 # tout (≈ 40 s)
node tests/e2e/run.mjs --target laravel-ref --only game8 --verbose
node tests/e2e/run.mjs --target laravel-ref --save          # + contrat et latences
node tests/e2e/render-contract.mjs                          # régénère docs/rust-migration/02 et 03
node tests/e2e/load.mjs --target laravel-ref --games 6 --duration 60 --ws 1000             # charge maximale
node tests/e2e/load.mjs --target rust --containers binome --games 6 --rate 120 --ws 1000   # charge égale
STATIC_DIR=…/dist tests/e2e/rust-stack.sh up && node tests/e2e/browser.mjs                # vrai front, Chromium
```

Mesures : toujours sous `caffeinate -dims` (un Mac capot fermé se met en veille et fausse les
temps), chaque pile seule. Cibles : `laravel-ref` (`ref-stack.sh`), `rust` (port 18101 :
`rust-stack.sh` ou l'image de production avec `BINOME_PORT=18101`).

| Scénario | Contenu |
|---|---|
| `lobby` | transfert d'hôte, salon vidé, lancement à 3, chat (anonyme, joueur, codes, anti-spam 429), emojis joueurs |
| `catalog` | accès admin, cosmos / univers / personnages (multipart, `_method=PUT`), validation, emojis admin, joueurs admin |
| `game8` | partie complète à 8 + spectateur : refus de règles, retirage, questions, corrections de l'hôte, tirages de carte (montré / gardé / expiré à 30 s), accusations, vue omnisciente, vote des éliminés, réactions, déconnexions (passage de tour, accusé hors ligne, exclusion), fin, scores, récap, historique, classement |
| `game7` | partie à 7 (orphelin) en accusations directes |

Le nettoyage se fait **par ids exacts** de ce que le banc a créé, jamais par motif.

## Fichiers

| Chemin | Rôle |
|---|---|
| `lib/realtime.mjs` | client temps réel ; **seul fichier qui connaît le protocole** (Pusher aujourd'hui, natif Rust ensuite) |
| `lib/http.mjs` | client HTTP, enregistre la forme de chaque réponse par gabarit de route |
| `lib/shapes.mjs` | formes JSON (types, nullabilité, formats de date) et comparaison de contrats |
| `contracts/laravel-ref.json` | **contrat de référence** observé sur Laravel |
| `results/` | rapports de charge et latences |
| `fixtures/` | images synthétiques (motifs générés, aucune image réelle) |

## Écarts connus de la référence

Signalés `⚠` et non comptés comme échecs sur `laravel-ref`, vérifiés normalement sur Rust :

- `POST /admin/reactions` accepte deux emojis (« 🦩🦩 ») malgré la règle « un seul emoji ».
