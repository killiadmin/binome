# Migration Rust — 07. Bilan

> Migration réalisée le 2026-11-11. Ancienne version : étiquette git `laravel-final`.

## Résultat

Binome est passé de **5 conteneurs** (MySQL, Laravel, Reverb, worker de queue, serveur Vite)
à **un seul conteneur de 9,3 Mo** : un binaire Rust qui sert l'API, le temps réel, les
minuteries et le front compilé, avec une base SQLite embarquée. Mesures détaillées :
`05-performances.md`.

| | Avant | Après |
|---|---|---|
| RAM au repos | 857 MiB (1 022 MiB avec Vite) | 3 MiB |
| CPU à charge égale (120 parties/min) | 262 % | 14,5 % |
| Latence API (p50) | 20 ms | 1,9 ms |
| Images Docker | ≈ 3,9 Go | 9,3 Mo |
| Processus à faire tourner | PHP ×3, MySQL, Node | 1 binaire |

Fonctionnellement, rien n'a disparu : les 63 routes et les 23 events reproduisent le contrat de
l'ancien backend (comparaison automatique : identique), le front n'a changé que dans
`useReverb.js` et la page admin « Prompts » (nouvelles commandes).

## Livrables

| Livrable | Où |
|---|---|
| Backend Rust (API, logique métier, accès SQLite, erreurs, configuration) | `server/` |
| Serveur WebSocket Rust (canaux, présence, autorisations, reconnexion) | `server/src/realtime/` |
| Front Vue 3 adapté (WebSocket natif, dépendances allégées) | `frontend/` |
| Docker simplifié (production, HTTPS, réseau local, développement) | `Dockerfile`, `docker-compose*.yml`, `deploy/` |
| Documentation technique | `01-audit-et-architecture.md`, `server/README.md`, `frontend/README.md`, `CLAUDE.md` |
| Documentation de l'API | `02-contrat-api.md` (générée depuis les mesures) |
| Documentation des événements WebSocket | `03-events.md`, `04-protocole-ws.md` |
| Règles du jeu et comportements | `docs/regles-du-jeu.md` |
| Lancement en développement, compilation et déploiement | `06-deploiement.md`, `server/README.md` |
| Tests automatisés | `server/` (unitaires), `tests/e2e/` (API + WebSocket, navigateur, charge, contrat) |
| Comparatif de performances | `05-performances.md` + rapports bruts `tests/e2e/results/` |
| Dépendances supprimées et conservées | ci-dessous |

## Dépendances

**Supprimées**

| Domaine | Dépendances |
|---|---|
| Exécution serveur | PHP 8.2 + extensions `pdo_mysql`, `mbstring`, `bcmath`, `zip`, `pcntl` ; Composer |
| Paquets PHP | 90 paquets Composer de production, dont `laravel/framework`, `laravel/reverb`, `laravel/sanctum` (inutilisé), `laravel/tinker` ; 35 de développement (`phpunit`, `pint`, `pail`, `sail`, `collision`, `faker`, `mockery`) |
| Base de données | MySQL 8 (serveur, conteneur, 696 MiB au repos) |
| Temps réel | Laravel Reverb (serveur WebSocket séparé, protocole Pusher) ; worker `queue:listen` |
| Front (npm) | `laravel-echo`, `pusher-js`, `socket.io`, `socket.io-client`, `express`, `pinia` (6 paquets directs ; 240 → 141 paquets dans le lockfile) |
| Production | Node.js (le serveur Vite servait l'application) |
| Tables | `users`, `sessions`, `cache`, `cache_locks`, `jobs`, `job_batches`, `failed_jobs`, `password_reset_tokens`, `personal_access_tokens`, `migrations` ; colonnes mortes `actions.character_id`, `binomes.is_discovered`, `binomes.discovered_by_player_id` |

**Conservées (front)** : `vue`, `vue-router`, `axios`, `bootstrap`, `bootstrap-vue-next`
(seuls les composants utilisés sont enregistrés), `@fortawesome/fontawesome-free`, `cropperjs`,
`qrcode` ; au build seulement : `vite`, `@vitejs/plugin-vue`.

**Ajoutées (serveur)** : 19 crates Rust directes (150 avec les dépendances transitives), toutes
compilées dans le binaire, aucune à installer : `axum`, `tokio`, `tower-http` (fichiers
statiques), `serde`, `serde_json`, `rusqlite` (SQLite embarqué), `time`, `rand`, `hmac`,
`sha2`, `subtle`, `hex`, `base64`, `imagesize`, `deunicode`, `unicode-segmentation`,
`futures-util`, `tracing`, `tracing-subscriber`. Volontairement absents : ORM, Redis, file de
messages. Optionnel en production : Caddy (HTTPS automatique), un conteneur de plus.

## Changements de comportement (validés)

- Un emoji d'administration doit être **un seul** graphème (Laravel acceptait « 🦩🦩 »).
- Les erreurs ne renvoient plus la pile d'appels PHP (`APP_DEBUG=true` l'exposait).
- Les events sont diffusés **après** l'enregistrement en base (plus de `409` transitoire sur `/recap`).
- La réponse de `/confirm` ne contient plus le graphe accidentel de la partie.
- L'import de personnages n'accepte plus d'image par URL (jamais utilisé).
- Code mort non repris : event `binome.discovered` (jamais émis), découverte de binôme
  (champs conservés dans l'API avec leur seule valeur observée : `false` / `0` / `null`).
- Présence vérifiée en mémoire (plus d'appel HTTP à Reverb), minuterie du tirage de carte
  réarmée au redémarrage (plus de dépendance au worker de queue).

## Évolutions après la migration (2026-11-12)

- **Historique conservé** : supprimer un salon ne supprime plus ses parties (migration de schéma
  n° 2 ; avant, le départ du dernier joueur effaçait la partie de l'historique et du classement).
- **Suppression de parties protégée** par un mot de passe dédié, connu du seul propriétaire
  (empreinte PBKDF2-SHA256, 5 échecs par minute au plus).

## Points ouverts

- **Identité** : `player_id` non signé (modèle de confiance d'origine, inchangé) ; un jeton
  signé à l'arrivée dans un salon fermerait l'usurpation d'un joueur par son identifiant.
- Pistes d'allègement du front sans effet visuel : `fetch` à la place d'`axios`, sous-ensemble
  d'icônes Font Awesome.
