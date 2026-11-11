# Banc E2E

Banc de non-régression **boîte noire** : il pilote le jeu comme le front (HTTP + WebSocket)
et vérifie les règles, les events reçus par chaque joueur et la forme exacte des JSON.
Construit pendant la migration Laravel → Rust : il a validé que le serveur Rust reproduit
l'ancien backend, dont le contrat est figé dans `contracts/laravel-ref.json`. Il n'utilise que
Node ≥ 22 (`fetch` et `WebSocket` intégrés), sans dépendance npm.

> ⚠ Le banc crée, modifie et supprime des données. Il ne vise **jamais** la base en service :
> uniquement un serveur de test sur une copie (port 18101).

## Serveur de test

```bash
tests/e2e/rust-stack.sh prepare            # copie à chaud de la base en service (ou : prepare f.sqlite)
tests/e2e/rust-stack.sh up [release]       # 127.0.0.1:18101/binome
STATIC_DIR=frontend/dist tests/e2e/rust-stack.sh up    # + front compilé, pour browser.mjs
tests/e2e/rust-stack.sh down
```

## Lancer

```bash
node tests/e2e/run.mjs                       # tout (≈ 35 s)
node tests/e2e/run.mjs --only game8 --verbose
node tests/e2e/run.mjs --save                # + contrat observé (contracts/rust.json) et latences
node tests/e2e/compare.mjs                   # contrat observé ↔ contrat figé de Laravel
node tests/e2e/render-contract.mjs           # régénère docs/rust-migration/02 et 03
node tests/e2e/browser.mjs                   # vrai front dans Chromium (4 joueurs)
node tests/e2e/load.mjs --games 6 --duration 60 --ws 1000            # charge maximale
node tests/e2e/load.mjs --games 6 --duration 60 --rate 120 --ws 1000 # charge fixe
node tests/e2e/gen-forbidden-vectors.mjs     # vecteurs de parité du détecteur de mots interdits
```

Mesures : toujours sous `caffeinate -dims` (un Mac capot fermé se met en veille et fausse les temps).

| Scénario | Contenu |
|---|---|
| `lobby` | transfert d'hôte, salon vidé, lancement à 3, partie supprimée par l'admin, chat (anonyme, joueur, codes, anti-spam 429), emojis joueurs |
| `catalog` | accès admin, cosmos / univers / personnages (multipart, `_method=PUT`), validation, emojis admin, joueurs admin |
| `game8` | partie complète à 8 + spectateur : refus de règles, retirage, questions, corrections de l'hôte, tirages de carte (montré / gardé / expiré à 30 s), accusations, vue omnisciente, vote des éliminés, réactions, déconnexions (passage de tour, accusé hors ligne, exclusion), fin, scores, récap, historique, classement |
| `game7` | partie à 7 (orphelin) en accusations directes |

Le nettoyage se fait **par ids exacts** de ce que le banc a créé, jamais par motif.

## Fichiers

| Chemin | Rôle |
|---|---|
| `lib/realtime.mjs` | client du protocole WebSocket natif |
| `lib/http.mjs` | client HTTP, enregistre la forme de chaque réponse par gabarit de route |
| `lib/shapes.mjs` | formes JSON (types, nullabilité, formats de date) et comparaison de contrats |
| `contracts/laravel-ref.json` | contrat de l'ancien backend, **figé** le 2026-11-11 |
| `contracts/rust.json` | contrat observé sur le serveur Rust (cumulé sur les exécutions `--save`) |
| `results/` | rapports de charge et latences (dont les mesures avant / après de la migration) |
| `fixtures/` | images synthétiques (motifs générés, aucune image réelle) |
