# Migration Rust — 01. Audit, architecture cible et plan

> Statut : décisions validées le 2026-11-11 (§10). **Phase 6 terminée** (mesures finales et
> validation : `05-performances.md`). **Phases 4 et 5 terminées** (front natif,
> image de production, `06-deploiement.md`). **Phase 2 terminée** (serveur Rust :
> `server/`, 361 vérifications E2E). **Phase 1 terminée** : contrat API
> (`02-contrat-api.md`), events (`03-events.md`), banc E2E (`tests/e2e/`), mesures de
> référence (`05-performances.md`).
> Audit réalisé le 2026-11-11 sur `main` @ `08b7c8c`, stack Docker en fonctionnement.

---

## 1. Résumé

Binome tourne aujourd'hui sur **5 conteneurs / ~30 processus** (MySQL, `artisan serve`,
Reverb, worker de queue, serveur Vite). Au repos, ils occupent **~1 GiB de RAM**, dont
**609 MiB pour MySQL** et **222 MiB pour le serveur Vite**, qui sert aussi en « production »
sur le LAN.

La cible est **un seul binaire Rust** (Axum + Tokio). Il sert dans le même processus :
l'API REST, le WebSocket, les minuteries (expiration du tirage de carte) et les fichiers
statiques du frontend Vue compilé. Base de données : **SQLite embarquée** (recommandé,
voir §7.3 : décision à valider) ou MySQL conservé.

Le gain principal ne vient pas du passage PHP → Rust (≈190 MiB pour backend + Reverb + queue),
mais de trois suppressions : le serveur Vite en exécution (222 MiB), MySQL (609 MiB, si
SQLite est retenu) et trois images PHP de 932 MB.

---

## 2. Cartographie du périmètre (mesuré dans le code)

| Élément | Volume |
|---|---|
| PHP applicatif (`backend/app`) | ~6 000 lignes uniques : 17 contrôleurs, 9 services, 14 modèles, 23 events, 1 job, 9 FormRequests, 3 classes Support, 1 commande Artisan |
| Routes HTTP | **62 routes API** + `POST /broadcasting/auth` (+ `/welcome`, `/up`, inutilisées) |
| Canaux WebSocket | 4 types : `room.{id}` (presence), `game.{id}` (presence), `player.{id}` (private), `lobby` (public) |
| Events diffusés | 22 actifs + 1 mort (`BinomeDiscovered`, jamais déclenché) |
| Tâches différées | 1 : `ExpireCardDraw` (30 s), seule raison d'être du worker de queue |
| Limiteurs de débit | 4 : `chat` 20/min, `reactions` 30/min, `custom-reactions` 5/min, `avatars` 10/min (indexés par joueur, pas par IP) |
| Tables métier | 17 (+ 9 tables Laravel techniques inutiles une fois migré) |
| Données | ~15 MB, dont 13,5 MB d'images base64 dans `characters.image` |
| Frontend | ~29 000 lignes Vue/JS ; **tout le couplage WebSocket est confiné à `src/sockets/useReverb.js`** |

### 2.1 Fonctionnalités à migrer (liste exhaustive)

**Salon** : créer / rejoindre par code (refus `409` si le pseudo est déjà connecté ailleurs,
vérifié via la présence), prêt (bascule), réglages de l'hôte (mode `random`/`cosmos`,
`reveal_to_eliminated`, `best_question_vote`), quitter (transfert d'hôte, salon supprimé
s'il est vide, joueur supprimé seulement s'il n'a jamais joué), spectateurs (`current_game_id`).

**Partie** : lancement (verrou du salon, ≥ 4 joueurs, tous prêts), formation des binômes
(mélange, orphelin si nombre impair, un univers distinct par binôme, 2 personnages même
`level_affectation`, uniquement `playable()`, filtre cosmos), vote des personnages (retirage
global, personnages refusés exclus si le catalogue le permet), rounds à ordre déterministe
par `player.id`, question → réponse (oui / non / je ne sais pas), tirage de carte (30 s,
mot secret renvoyé en HTTP au seul questionneur, partage via `player.{id}` à tous sauf la
cible), accusation → confirmation par la cible, élimination, condition de victoire
(équipe intacte / rescapé), fin de partie et scores (+5 / +3, rounds survécus, éliminations,
meilleures questions, exclu = 0), vote « meilleure question » des éliminés, vue omnisciente
des éliminés, réactions rapides, correction de réponse par l'hôte, passer le tour d'un
joueur déconnecté (`turn` / `accusation` tranchée par le serveur / `draw`), exclusion
(lui et son binôme, annulation de l'action en attente), récapitulatif + 7 trophées
(dont la détection des mots interdits `ForbiddenWordMatcher`, à porter à l'identique
de `frontend/src/services/forbiddenWords.js`).

**Hors partie** : chat public du hall (rétention 1 h, purge à la lecture et à l'écriture,
50 derniers messages, codes de salon reconnus dans le texte), historique public (parties
terminées uniquement), classement (rangs ex æquo, regroupement par pseudo insensible à la
casse et aux accents), fiche joueur, photos de profil (JPEG carré 64–512 px ≤ 200 ko,
indexées par pseudo, URL versionnée, cache 1 an), emojis de réaction (Unicode + images
JPEG 64–256 px ≤ 60 ko, 10 par pseudo).

**Administration** (mot de passe → jeton HMAC `X-Characters-Access`) : CRUD cosmos /
univers / personnages (upload multipart, slug unique), validation des propositions
(accepter / refuser / remettre à valider), parties (liste, détail, suppression en
cascade), joueurs (liste, retrait du salon, suppression avec ou sans parties), emojis.

**Outils** : `php artisan characters:import fichier.json [--images-dir] [--skip-existing]`
et `CharacterSeeder` (images locales, chemin absolu, URL http(s), data URI, convention
`<slug>.(png|jpg|…)`).

### 2.2 Écarts entre la documentation et le code

Ces points sont à corriger dans la doc ; le comportement **réel** est celui qui sera reproduit.

1. La route `POST /games/{g}/rounds/{r}/actions/{a}/confirm` (confirmation d'accusation)
   est absente du tableau des routes de `backend/README.md`.
2. Les events `AccusationConfirmed` et `PlayerEliminated` sont diffusés et écoutés par le
   front, mais absents du récapitulatif des events.
3. `BinomeDiscovered` est du code mort : jamais déclenché. C'est d'ailleurs le seul event en
   `ShouldBroadcast` (queue), contrairement à ce qu'affirme CLAUDE.md. Conséquence :
   `binomes.is_discovered` reste toujours `false`, donc `binomes_discovered_count` vaut
   toujours 0 dans l'historique.
4. La collation effective de la base est **`utf8mb4_unicode_ci`**, pas `utf8mb4_0900_ai_ci`
   (les deux ignorent casse et accents ; le comportement décrit reste juste).
5. `frontend/README.md` indique que l'accusation envoie `character_id` ; le code envoie
   `character_name`.
6. Laravel Sanctum est installé et son middleware est actif, mais il ne sert à rien (aucun
   jeton émis).
7. La colonne `actions.character_id` n'est jamais lue ni écrite.
8. La table `failed_jobs` contient 31 échecs, tous du 2026-07-14 entre 21:20:30 et 21:20:34
   (des events sérialisés dont le modèle avait été supprimé). Ce sont des vestiges, pas un
   bug actuel.
9. **Découvert par le banc E2E (phase 1)** :
   - `POST /admin/reactions` accepte « 🦩🦩 » malgré le contrôle « un seul emoji » (`^\X$`) :
     bug existant. Rust appliquera la règle documentée (un seul graphème) ; **c'est un
     changement de comportement, à confirmer**.
   - `APP_DEBUG=true` dans `backend/.env` : les erreurs `404` / `429` renvoient la pile
     d'appels PHP complète (fichiers, lignes, classes) à n'importe quel client. Rust ne
     renverra que `{ message }`.
   - Le front modifie un personnage par `POST /characters/{id}` + `_method=PUT` (method
     spoofing Laravel) : Rust doit accepter cette forme.
   - `players[].is_ready` sort en `0` / `1`, pas en booléen (pivot non casté).
10. **Découvert en phase 2 — perte de l'historique** : `games.room_id` est en `ON DELETE
    CASCADE`. Quand le dernier joueur quitte un salon, le salon est supprimé **et ses parties
    avec lui** (binômes, actions, `game_stats`) : la partie disparaît de l'historique et du
    classement. Vérifié par le banc sur les deux backends ; la base réelle ne contient plus
    que 4 parties pour des ids allant jusqu'à 113. Rust reproduit ce comportement **en
    attendant une décision**.
11. **Écarts assumés en phase 2** (sans effet sur le front) :
    - la réponse de `/confirm` ne contient plus le graphe accidentel de la partie (binômes,
      rounds) que Laravel y joignait via ses relations chargées ;
    - `import-characters` ne télécharge plus d'images par URL (jamais utilisé : aucun fichier
      d'import n'en contient) ; image locale, data URI ou convention `<slug>.png` ;
    - un emoji admin doit être exactement un graphème (correction validée).

### 2.3 Particularités à reproduire fidèlement

- **Formats JSON.** Les dates viennent soit de `toIso8601String()` (`2026-11-11T10:00:00+00:00`),
  soit de la sérialisation Eloquent (`2026-11-11T10:00:00.000000Z`), selon la route. Certains
  booléens de pivot non castés sortent en `0`/`1` (ex. `is_ready` dans `playersPayload()`).
  Les réponses de `/answer` et `/confirm` sont des `toArray()` Eloquent bruts (le front n'en
  lit rien, mais elles seront reproduites).
  → On capture les réponses réelles de Laravel comme **fichiers de référence** et on compare (§9).
- **Courses concurrentes.** `lockForUpdate` sur `rounds` / `games` / `rooms`, et mises à jour
  conditionnelles (`whereNull('answer')`, `card_draw_status`, `current_player_id = from`).
  Ce sont elles qui garantissent qu'un round n'avance qu'une fois.
- **Insensibilité casse / accents** pour `players.pseudo` (unicité), `player_avatars.pseudo`,
  `game_stats.player_pseudo` (regroupement du classement) et les recherches par pseudo.
- **Secrets.** `actions.drawn_word` n'est jamais sérialisé ; les personnages ne sont jamais
  dans `GET /games/{g}` ; les binômes ne sont jamais exposés tant qu'ils ne sont pas
  découverts (protection de l'orphelin).
- **Préfixe `/binome`** dans les URL d'images (`APP_PUBLIC_PREFIX`).

---

## 3. Mesures de référence (état actuel)

Mesuré le 2026-11-11 avec `docker stats --no-stream`, au repos, aucune partie en cours.

| Conteneur | CPU | RAM | PIDs |
|---|---|---|---|
| `binome_db` (MySQL 8) | 2,87 % | **608,8 MiB** | 42 |
| `binome_frontend` (Vite dev) | 0,08 % | **221,7 MiB** | 34 |
| `binome_backend` (`artisan serve`, 4 workers) | 0,07 % | 81,3 MiB | 7 |
| `binome_queue` (`queue:listen`) | 0,60 % | 65,2 MiB | 2 |
| `binome_reverb` | 0,00 % | 45,1 MiB | 1 |
| **Total** | **~3,6 %** | **~1 022 MiB** | **86** |

| Image | Taille |
|---|---|
| `binome-backend` / `binome-reverb` / `binome-queue` | 932 MB chacune (même image, construite 3 fois) |
| `binome-frontend` | 264 MB |
| `mysql:8` | 793 MB |

**Encore à mesurer en phase 1** (aucun chiffre ne sera annoncé sans mesure) : RAM et CPU
en charge, latence API (p50/p95), nombre de connexions WebSocket simultanées tenues,
temps de démarrage. Protocole : le même script de charge (§9), sur la même machine, contre
chacune des deux piles, sur une copie de la base.

> Note : `queue:listen` relance un processus PHP à chaque job, d'où le CPU de repos non nul
> (0,6 %). MySQL consomme 2,9 % de CPU au repos, sans aucun trafic.

---

## 4. Dépendances

### 4.1 Backend : tout PHP disparaît

| Supprimé | Raison |
|---|---|
| `laravel/framework`, `laravel/tinker` | remplacés par Axum + le code métier porté |
| `laravel/reverb` (+ `pusher/pusher-php-server` transitif) | WebSocket intégré au binaire |
| `laravel/sanctum` | inutilisé |
| dev : `pint`, `pail`, `sail`, `collision`, `faker`, `mockery`, `phpunit` | remplacés par `cargo fmt` / `clippy` / `cargo test` |
| `backend/package.json` (vite, laravel-vite-plugin, axios) | squelette Laravel jamais utilisé |
| Composer, PHP 8.2, extensions `pdo_mysql`, `mbstring`, `bcmath`, `zip`, `pcntl` | plus de PHP |
| Tables `users`, `password_reset_tokens`, `personal_access_tokens`, `sessions`, `cache`, `cache_locks`, `jobs`, `job_batches`, `failed_jobs` | sessions inutilisées ; cache et limiteurs passent en mémoire ; queue remplacée par une minuterie Tokio |

### 4.2 Crates Rust proposées (minimum nécessaire)

| Crate | Usage | Justification |
|---|---|---|
| `tokio` | runtime async | requis par Axum ; minuteries des tirages de carte |
| `axum` (features `ws`, `multipart`, `json`, `query`) | HTTP + WebSocket | voir §7.1 |
| `tower-http` (`fs`, `limit`, `trace`) | fichiers statiques du front, limite de taille des requêtes | même écosystème qu'Axum |
| `serde`, `serde_json` | JSON | standard |
| `rusqlite` (`bundled`) **ou** `sqlx` (`mysql`) | accès BDD | selon la décision §7.3 |
| `time` | dates ISO 8601 | plus léger que `chrono` |
| `rand` | mélange, codes de salon, tirage | standard |
| `hmac`, `sha2`, `subtle` | jeton d'accès admin (même valeur qu'aujourd'hui), comparaison à temps constant | RustCrypto |
| `base64` | images en data URI | standard |
| `imagesize` | dimensions et type JPEG (remplace `getimagesizefromstring`) | lit l'en-tête seulement, pas de décodage |
| `deunicode` | équivalent de `Str::ascii` (slugs, comparaison de pseudos, détecteur de mots interdits) | sans dépendance |
| `unicode-segmentation` | contrôle « un seul emoji » (`^\X$`) | standard |
| `tracing`, `tracing-subscriber` | journaux | standard avec Axum |
| `ureq` (feature optionnelle, outil d'import seulement) | téléchargement d'images par URL à l'import | client HTTP synchrone minimal ; hors du chemin serveur |

Volontairement **absents** : ORM (SeaORM, Diesel), framework de validation, Redis,
broker de messages, `chrono`, `anyhow` côté serveur (une seule énumération d'erreurs
écrite à la main suffit).

### 4.3 Frontend

| Paquet | Décision | Raison |
|---|---|---|
| `laravel-echo`, `pusher-js` | **supprimés** (si protocole natif, §7.2) | remplacés par ~150 lignes de `WebSocket` natif dans `useReverb.js` |
| `socket.io`, `socket.io-client`, `express` | **supprimés** | jamais importés ; le script `npm run server` pointe vers un fichier inexistant |
| `pinia` | **supprimé** | instancié dans `main.js` mais aucun store défini |
| `vue`, `vue-router` | conservés | — |
| `axios` | conservé (phase 1) | utilisé par tous les services avec intercepteurs ; le remplacer par `fetch` toucherait 12 fichiers pour ~14 ko gzip côté client, sans effet sur le serveur. Optionnel plus tard |
| `bootstrap`, `bootstrap-vue-next` | conservés | UI (`BModal` ×24, `BFormInput` ×16…). **Optimisation sans changement visuel** : `main.js` enregistre globalement *tous* les composants ; n'importer que les 10 utilisés réduit le bundle |
| `@fortawesome/fontawesome-free` | conservé | visuel ; sous-ensemble possible plus tard |
| `cropperjs`, `qrcode` | conservés | recadrage d'images, QR code de salon |
| `vite`, `@vitejs/plugin-vue` | conservés **en dev et au build uniquement** | Node absent de l'image de production |

---

## 5. Risques techniques et incompatibilités

| Risque | Gravité | Parade |
|---|---|---|
| Divergence subtile de règle métier (ordre des tours, scores, fin de partie, orphelin) | Haute | Tests unitaires Rust table par table sur `checkGameOver`, `nextTurn`, `computeAndStore`, le matcher ; scénario E2E à 8 joueurs rejoué contre les deux backends, comparaison des états |
| Différences de forme JSON (dates, `0`/`1`, clés absentes ou `null`) | Moyenne | Réponses de référence capturées sur Laravel, comparées automatiquement à celles de Rust |
| Pseudo insensible casse/accents sans collation MySQL (si SQLite) | Moyenne | Colonne normalisée `pseudo_key` (NFKD → ASCII → minuscules) avec index `UNIQUE`, calculée côté Rust ; on reproduit `utf8mb4_unicode_ci` sur les cas réels (accents, casse). Les cas exotiques (ligatures, ß) seront listés |
| Migration des **données réelles** (personnages et images uniques, sans autre copie) | **Haute** | Sauvegarde `mysqldump` horodatée avant tout ; outil de copie MySQL → SQLite en lecture seule sur MySQL ; contrôle de comptes et de sommes de contrôle table par table ; MySQL conservé intact jusqu'à validation |
| Concurrence SQLite (un seul écrivain) | Faible à l'échelle de Binome | Mode WAL ; transactions `BEGIN IMMEDIATE` (sérialisation plus forte que les `lockForUpdate` actuels) ; mises à jour conditionnelles conservées |
| Un seul processus = point de défaillance unique | Faible | Aujourd'hui, Reverb seul est déjà un point unique. `restart: unless-stopped` ; les tirages de carte en cours sont réarmés au démarrage depuis `card_draw_deadline` |
| Reconnexion WebSocket (téléphones verrouillés) | Moyenne | Ping serveur + détection côté client, reconnexion avec backoff, réabonnement automatique, émission de `state_change` → `connected` pour que la logique `resync()` existante de `RoundPage.vue` reste inchangée |
| Modèle de confiance faible (`player_id` / `X-Player-Id` non signés) | Existant | **Reproduit tel quel** (pas de changement de comportement). Amélioration possible plus tard : jeton signé délivré à la création ou à l'arrivée dans un salon (§10, hors périmètre) |
| Portail `../portail` qui vise `binome_frontend:5173` | Faible | Alias réseau Docker `binome_frontend` sur le nouveau conteneur, port 5173 conservé en interne : **aucune modification du portail** |
| Pas de toolchain Rust sur l'hôte | Faible | Compilation dans une étape Docker `rust:*-alpine` ; `rustup` sur l'hôte facultatif (plus confortable pour itérer) |

**Changement de comportement assumé (amélioration)** : aujourd'hui, plusieurs events partent
**pendant** la transaction (`GameStarted`, `GameEnded`, `PlayerExcluded`…). D'où le `409`
transitoire sur `/recap` que le front contourne par des reprises. En Rust, les events sont
collectés pendant la transaction et publiés **après le commit**. Aucun client n'est cassé ;
les reprises du front deviennent simplement inutiles.

---

## 6. Gains attendus (estimations, à confirmer par mesure)

Ces chiffres sont des **ordres de grandeur attendus, pas des résultats**. Le tableau
avant/après de la phase 6 ne contiendra que des mesures.

| Indicateur | Actuel (mesuré) | Cible attendue | Origine du gain |
|---|---|---|---|
| RAM au repos | ~1 022 MiB | ~10–30 MiB avec SQLite ; ~620 MiB si MySQL est conservé | MySQL (609) + Vite (222) + PHP (~190) |
| Processus | 86 PIDs / 5 conteneurs | 1 processus / 1 conteneur (SQLite) | — |
| CPU au repos | ~3,6 % | ≈ 0 % | plus de `queue:listen`, plus de MySQL |
| Taille des images | 3 × 932 MB + 264 MB + 793 MB | ~10–20 MB (binaire statique + `dist/`) | image `scratch` / distroless |
| Latence API | à mesurer | à mesurer | pas de bootstrap PHP par requête ; présence vérifiée en mémoire au lieu d'un appel HTTP à Reverb |

**Limites du gain** : en charge, le coût réel de Binome est dominé par la taille des réponses
(images base64 de 50–160 ko lues en base) plus que par le langage. Côté navigateur, le gain
est modeste (bundle légèrement réduit). L'essentiel de la RAM gagnée vient des services
**supprimés**, pas de Rust lui-même.

---

## 7. Architecture cible

```
                         navigateur (LAN, via ../portail → alias binome_frontend:5173)
                                         │  un seul port, une seule origine
                                         ▼
┌────────────────────────── binome (1 binaire Rust, 1 processus) ─────────────────────────┐
│  /binome/            → fichiers statiques Vue (dist/), repli SPA vers index.html        │
│  /binome/api/*       → routeur Axum (handlers → services métier)                        │
│  /binome/ws          → WebSocket natif (hub de canaux + présence en mémoire)            │
│  minuteries Tokio    → expiration des tirages de carte (remplace queue + ExpireCardDraw) │
│  limiteurs en mémoire→ chat / réactions / emojis / avatars                              │
│                                         │                                              │
│                              SQLite (fichier, WAL)  ── ou ── MySQL (décision §7.3)      │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.1 Framework HTTP : **Axum**

- Axum vs Actix Web : performances équivalentes à l'échelle de Binome (quelques dizaines de
  joueurs ; les deux tiennent des dizaines de milliers de req/s).
- Axum est maintenu par l'équipe Tokio et s'appuie sur `tower` / `hyper`, que l'on utilise de
  toute façon (fichiers statiques via `tower-http`). Son WebSocket est intégré (`axum::extract::ws`),
  sans crate supplémentaire.
- Il donne moins de dépendances au total et des handlers qui sont de simples fonctions async.
- Actix apporterait son propre runtime d'acteurs, inutile ici.

### 7.2 WebSocket : **protocole natif léger** (recommandé) plutôt que compatibilité Pusher

Le front n'utilise de Pusher que : `join` (presence) avec `here` / `joining` / `leaving`,
`private`, `channel` public, `listen('.event')`, `leave`, et l'état de connexion
(`state_change` → `connected`). Aucun event client (`whisper`), aucun `socketId`/`toOthers`.

| | Compatibilité Pusher | Protocole natif |
|---|---|---|
| Changements front | aucun | **`useReverb.js` seulement** (API publique identique ; pages inchangées) |
| Dépendances front | garde `laravel-echo` + `pusher-js` | les deux supprimées |
| Serveur | protocole Pusher complet (signatures HMAC d'auth, `pusher:subscribe`, `pusher_internal:member_added`, codes d'erreur…) + route `/broadcasting/auth` | ~5 messages JSON, autorisation **dans** l'abonnement (pas d'aller-retour HTTP par canal) |
| Risque | quirks du protocole à reproduire | plus simple à tester |

Protocole proposé (documenté en détail en phase 3) :

```
connexion : GET /binome/ws?player_id=123        (sans player_id : visiteur, seul « lobby » autorisé)
client → { "op": "sub",   "ch": "game.12" }   |  { "op": "unsub", "ch": "game.12" }
serveur → { "op": "sub_ok", "ch": "game.12", "members": [{ "id": 1, "pseudo": "…" }] }  // here
          { "op": "sub_err", "ch": "game.12", "status": 403 }
          { "op": "joining" | "leaving", "ch": "game.12", "member": { … } }
          { "op": "event", "ch": "game.12", "event": "round.started", "data": { … } }   // payloads identiques
ping/pong WebSocket natifs toutes les 25 s ; le client se reconnecte (backoff 1→10 s) et se réabonne.
```

Les règles d'autorisation de `routes/channels.php` sont reprises à l'identique (joueur de la
partie ou membre du salon pour `game.{id}`, membre du salon pour `room.{id}`,
`player_id == id` pour `player.{id}`). La présence est dédupliquée par joueur (plusieurs
onglets = un membre ; `leaving` au dernier onglet fermé), comme Reverb.
`PresenceService` devient une lecture en mémoire du hub : plus de cas « Reverb muet ».

### 7.3 Base de données : **SQLite recommandé** — décision à valider

| | SQLite embarqué | MySQL conservé |
|---|---|---|
| RAM | dans le processus (~quelques Mo de cache) | **609 MiB** au repos |
| Conteneurs | 0 | 1 |
| Volume de données | 15 MB : largement dans le confort de SQLite | — |
| Concurrence | un écrivain à la fois, en microsecondes : sans effet pour quelques parties simultanées | verrous de ligne |
| Sauvegarde | copie d'un fichier (`VACUUM INTO`) | `mysqldump` |
| Travail en plus | migration unique des données ; colonne `pseudo_key` pour remplacer la collation | aucun |
| Risque | migration des données réelles (voir §5) | aucun |

Recommandation : **SQLite**, puisque l'objectif n°1 est la RAM et que MySQL représente 60 %
de la consommation actuelle. Le code d'accès sera isolé dans un module `db/` pour que le
choix reste réversible. Si tu préfères limiter le risque, on peut faire la migration en
deux temps : Rust sur MySQL d'abord, bascule SQLite ensuite.

### 7.4 Organisation du code Rust (`server/`)

```
server/
├── Cargo.toml
├── migrations/0001_schema.sql        # schéma métier unique (17 tables, sans les tables Laravel)
└── src/
    ├── main.rs          # CLI : `serve` (défaut), `import-characters`, `migrate`
    ├── config.rs        # variables d'environnement, centralisées
    ├── error.rs         # AppError → 403/404/409/422 (formats {message, errors} identiques)
    ├── db/              # pool/connexion, transactions, requêtes par domaine
    ├── models.rs        # structs + enums (GameStatus, GameMode, ActionType, CardDrawStatus)
    ├── services/        # game, round, action, binome, score, recap, history, question_vote
    ├── support/         # forbidden_words (port exact), slug, image (jpeg/data URI), ascii
    ├── http/            # routes.rs + 1 module par domaine (rooms, games, actions, catalog, admin, chat, media, history)
    ├── realtime/        # hub (canaux, présence), protocole ws, autorisation, events typés
    ├── timers.rs        # expiration des tirages + réarmement au démarrage
    └── ratelimit.rs     # fenêtres glissantes en mémoire
```

Un module par domaine, pas de couches « repository / use-case / port » superflues :
les services reçoivent une transaction et renvoient le résultat **et** la liste des events
à publier après le commit.

### 7.5 Frontend

- `src/sockets/useReverb.js` : réécrit sur `WebSocket` natif, **mêmes exports et mêmes
  callbacks** (`joinRoom`, `joinGame`, `joinPlayerChannel`, `joinLobbyChat`, `leave*`,
  `onConnectionChange` avec `{previous, current}`, `resetEcho`, `hasEcho`, `onEchoReset`).
  Aucune page ni composant modifié.
- `main.js` : retrait de Pinia ; enregistrement des seuls composants BootstrapVueNext utilisés.
- `package.json` : retrait des 6 paquets inutiles ; `VITE_REVERB_APP_KEY` disparaît.
- `vite.config.js` : en dev, proxy `/binome/api` et `/binome/ws` vers le binaire Rust.
- Aucune modification visuelle.

### 7.6 Docker

**Production** — `Dockerfile` multi-étapes à la racine :
1. `node:22-alpine` → `npm ci && npm run build` → `dist/`
2. `rust:alpine` → `cargo build --release` (cible musl, binaire statique, `strip`, `lto`, `opt-level`)
3. `scratch` (ou `gcr.io/distroless/static`) ← binaire + `dist/`

`docker-compose.yml` (prod) : **un service** `binome` + un volume pour `binome.sqlite`, sur le
réseau `meszappli` avec l'alias `binome_frontend` (port 5173). Healthcheck intégré au
binaire (`binome healthcheck`) puisqu'une image `scratch` n'a ni `curl` ni shell.

**Développement** — `docker-compose.dev.yml` : le binaire Rust (recompilé par `cargo watch`,
dépendances en cache dans un volume) + le serveur Vite pour le HMR, qui proxifie vers Rust.
Node n'existe que dans ce fichier.

`start.sh` / `scripts/update-lan-ip.sh` : inchangés dans leur rôle. `lan-url.json` est lu
depuis le dossier statique servi par Rust.

---

## 8. Plan de migration et fichiers concernés

| Phase | Contenu | Fichiers créés / modifiés |
|---|---|---|
| **1. Préparation** | Contrats API figés (OpenAPI léger ou tableau Markdown), catalogue des events avec payloads, réponses de référence capturées sur Laravel ; banc E2E rejouable à N joueurs (HTTP + WebSocket) **versionné dans le repo** cette fois ; mesures de charge Laravel ; sauvegarde de la base | `docs/rust-migration/02-contrat-api.md`, `03-events.md`, `tests/e2e/` (scénario 8 joueurs, charge, fichiers de référence), `scripts/backup-db.sh` |
| **2. Backend Rust** | Projet Cargo, schéma, config, erreurs, accès BDD, tous les services et les 62 routes, auth admin, limiteurs, images, commande `import-characters`, outil de migration des données | `server/**`, `tools/mysql-to-sqlite/` (si SQLite, jamais embarqué en prod) |
| **3. WebSocket** | Hub, présence, autorisation, ping/reconnexion, minuteries ; tests à plusieurs clients simultanés | `server/src/realtime/**`, `server/src/timers.rs`, `docs/rust-migration/04-protocole-ws.md` |
| **4. Frontend** | `useReverb.js` natif, nettoyage des dépendances, proxy dev | `frontend/src/sockets/useReverb.js`, `frontend/src/main.js`, `frontend/package.json`, `frontend/vite.config.js`, `frontend/.env.example` |
| **5. Docker** | Images prod / dev, compose simplifié | `Dockerfile`, `docker-compose.yml`, `docker-compose.dev.yml`, `start.sh`, `.dockerignore` |
| **6. Validation** | Suite E2E + contrats contre Rust, charge, tableau avant/après mesuré, essai réel sur le LAN | `docs/rust-migration/05-performances.md` |
| **7. Nettoyage** (après ta validation explicite) | Suppression de `backend/` (Laravel), mise à jour de `CLAUDE.md` et des README | `backend/**` supprimé, `CLAUDE.md`, `README` |

Pendant les phases 2 à 6, **Laravel reste en place et fonctionnel**. La pile Rust tourne à
côté, sur une **copie** de la base, jamais sur la base réelle avant la bascule.

---

## 9. Stratégie de test

- **Unitaires (Rust)** : condition de victoire (paires, orphelin, rescapé, binôme vidé),
  ordre des tours avec joueur exclu, scores et bonus, meilleures questions ex æquo,
  détecteur de mots interdits (jeux d'essai générés depuis la version JS pour garantir la
  parité), slug, normalisation des pseudos, validation JPEG.
- **Intégration (Rust, BDD en mémoire)** : chaque route, cas nominal + chaque refus
  (`403` / `404` / `409` / `422` avec message), courses concurrentes (double envoi,
  réponse contre passage de tour simultanés, tirage contre expiration).
- **Contrat** : réponses de référence Laravel ↔ Rust comparées (dates et ids normalisés).
- **WebSocket** : N clients simultanés, présence (multi-onglets, `leaving` différé),
  autorisations refusées, coupure puis reconnexion avec réabonnement, `card.word.shared`
  absent chez la cible.
- **E2E** : partie complète à 8 joueurs (orphelin à 7 joueurs, exclusion, tirage de carte,
  vote, fin de partie), rejouée à l'identique contre les deux backends.
- Jamais d'opération destructive sur la base réelle : fixtures dédiées, nettoyage par ids exacts.

---

## 10. Décisions validées (2026-11-11)

1. **SQLite.** Sauvegarde avant migration : `~/Meszappli/backups/binome/2026-11-11/binome.sql`.
   Restauration testée dans un conteneur jetable : comptes de lignes et taille totale des
   images identiques à la base réelle.
2. **Protocole WebSocket natif**, à condition que le jeu reste fluide et en temps réel (push
   instantané inchangé, voir §7.2).
3. **Events publiés après le commit.**
4. **Pas de code mort porté.**

## 10 bis. Questions d'origine

1. **Base de données** : SQLite (recommandé) ou MySQL conservé (ou les deux en séquence).
2. **WebSocket** : protocole natif + `useReverb.js` réécrit (recommandé), ou compatibilité Pusher sans toucher au front.
3. **Changement de comportement** : publication des events après commit (recommandé, §5).
4. **Code mort** : ne pas porter `BinomeDiscovered`, `is_discovered`, `discovered_by_player_id`,
   `actions.character_id` ni les tables Laravel techniques (recommandé). Le listener front
   `binome.discovered` devient inerte, comme aujourd'hui.
5. **Hors périmètre, à décider plus tard** : authentification par jeton signé au lieu de
   `player_id` en clair ; remplacement d'`axios` par `fetch` ; sous-ensemble de Font Awesome.
