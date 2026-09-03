# Backend — Binome

## Table des matières

- [Règles du jeu](#règles-du-jeu)
- [Stack technique](#stack-technique)
- [Architecture du projet](#architecture-du-projet)
- [Modèle de données](#modèle-de-données)
- [Services](#services)
- [Controllers & Endpoints](#controllers--endpoints)
- [Events WebSocket](#events-websocket)
- [Flux complet d'une partie](#flux-complet-dune-partie)
- [Points d'attention importants](#points-dattention-importants)

---

## Règles du jeu

### Objectif
Deviner le personnage secret des autres joueurs avant que son propre binome soit découvert.

### Mise en place
- Nombre de joueurs : **pair, minimum 4** (ex : 6, 8, 10…)
- Au début de chaque partie, chaque joueur reçoit un **personnage secret** tiré d'un univers (ex : Disney, Marvel…)
- Les joueurs sont regroupés en **binomes** de 2 personnes partageant le même univers, **à leur insu**
- Chaque personnage possède **3 mots interdits**

### Modes de jeu

L'hôte choisit le mode dans le lobby (`PATCH /rooms/{room}/settings`) :

| Mode (`rooms.game_mode`) | Comportement |
|---|---|
| `random` (défaut) | Chaque binome reçoit un **univers tiré au hasard** parmi les univers jouables. |
| `cosmos` | L'hôte fixe un **cosmos** (`rooms.cosmos_id`) : tous les binomes reçoivent un univers **de ce cosmos**. |

Dans les deux modes : **un univers distinct par binome**, et binome = 2 persos du même univers au
même `level_affectation`. Un univers est « jouable » s'il a ≥ 2 personnages `playable()` partageant
un niveau. Le mode `cosmos` exige donc **≥ (nb de joueurs / 2) univers jouables** dans le cosmos,
sinon le démarrage renvoie une erreur `422` (message nommant le cosmos et les compteurs).
`GET /cosmos` expose `playable_universe_count` par cosmos pour l'affichage côté lobby.

### Déroulement
- Les joueurs jouent **chacun leur tour**, dans le même ordre à chaque round
- À son tour, un joueur peut effectuer **une seule action** parmi deux :
    1. **Poser une question** à n'importe quel autre joueur (réponse libre : oui/non)
    2. **Faire une accusation** : désigner un joueur et nommer précisément son personnage

### Mots interdits
- Chaque joueur a **ses propres mots interdits** liés à **son personnage**
- Si un joueur pose une question contenant l'un de ses mots interdits, **la question est refusée**
- Le joueur **perd son tour** (l'action est enregistrée comme invalide)
- Exemple : personnage = Tarzan, mot interdit = "jungle" → la question "Vit-il dans la jungle ?" est refusée

> ⚠️ Ce sont les mots interdits du **joueur qui pose la question** (liés à son propre personnage) qui filtrent, pas ceux du joueur interrogé. La vérification est uniquement côté backend — le joueur interrogé répond librement sans contrainte backend.

### Victoire
- Quand une accusation est correcte, le **binome du joueur ciblé est découvert**
- La partie se termine quand il ne reste **plus qu'un seul binome non découvert**
- Ce dernier binome **gagne la partie** (les autres ont tous été découverts avant)

---

## Stack technique

| Composant | Technologie |
|---|---|
| Framework backend | Laravel 11 |
| Auth API | Laravel Sanctum |
| WebSocket | Laravel Reverb (`php artisan install:broadcasting`) |
| Base de données | MySQL |
| PHP | ^8.2 |
| Conteneurisation | Docker Compose (voir README racine) |

---

## Lancer le projet

### Avec Docker (recommandé)

Toute la stack (MySQL, backend, Reverb, queue, frontend) est orchestrée depuis la racine du repo via `docker-compose.yml`. Voir le README à la racine pour le détail — en résumé :

```bash
# Depuis la racine du repo
./start.sh
```

Le backend est alors servi sur `http://localhost:8000`, Reverb sur le port `8080`, MySQL exposé sur `9090`. Les migrations tournent automatiquement au démarrage du container `backend`.

### Sans Docker (legacy)

```bash
# 1. Installer les dépendances
composer install

# 2. Copier le .env
cp .env.example .env
php artisan key:generate

# 3. Lancer les migrations
php artisan migrate

# 4. Terminal 1 — Serveur Laravel
php artisan serve --host=0.0.0.0

# 5. Terminal 2 — Reverb WebSocket
php artisan reverb:start --host=0.0.0.0 --debug

# 6. Terminal 3 — Queue (si ShouldBroadcast utilisé)
php artisan queue:listen --tries=1
```

> ⚠️ Tous les events utilisent `ShouldBroadcastNow` (pas de queue requise en dev).

---

## Import de personnages en masse

Plutôt que de créer les personnages un par un dans l'interface, on peut les
importer depuis un fichier JSON.

```bash
# Import ponctuel depuis n'importe quel fichier
php artisan characters:import chemin/vers/personnages.json

# Options
php artisan characters:import fichier.json --images-dir=/chemin/images
php artisan characters:import fichier.json --skip-existing   # ne touche pas aux persos déjà en base

# Via le seeder (utilise database/data/characters.json)
php artisan db:seed --class=CharacterSeeder
php artisan db:seed                                          # inclut CharacterSeeder
```

Format du fichier — un tableau d'objets :

```json
[
  {
    "name": "Pikachu",
    "universe": "Pokémon",
    "cosmos": "Animé Japonais",
    "forbidden_words": ["souris", "éclair", "jaune"],
    "level_affectation": 1,
    "image": "pikachu.png"
  }
]
```

- **Idempotent** : un personnage est identifié par `(univers, nom)`. Relancer
  l'import met à jour les personnages existants au lieu de les dupliquer. Un
  personnage déjà **refusé** (`hidden = true`) est ignoré, jamais recréé.
- Les personnages importés sont créés avec **`verif_manual = false`** : ce sont
  des propositions à valider à la main (voir *Validation manuelle* ci-dessous).
  Ils apparaissent dans `/characters` avec un badge « à valider » mais ne sont
  **pas jouables** tant qu'ils ne sont pas validés.
- L'**univers** et le **cosmos** (facultatif) sont créés automatiquement s'ils
  n'existent pas ; un univers déjà présent sans cosmos se voit rattacher celui du
  fichier.
- La règle « 2 personnages max par niveau d'affectation dans un univers » est
  vérifiée : une ligne en trop est signalée en erreur, les autres passent.
- **Images** (`app/Support/CharacterImporter.php`) — le champ `image` accepte :
  un nom de fichier présent dans `database/data/images/` (ou `--images-dir`), un
  chemin absolu, une URL `http(s)` (téléchargée à l'import), ou une data URI déjà
  encodée. Si le champ est absent, l'import cherche par **convention** un fichier
  `<slug-du-nom>.(png|jpg|jpeg|webp|gif)` dans le dossier d'images. Formats
  jpeg/png/gif/webp, 4 Mo max ; l'image est encodée en base64 comme l'upload
  manuel (pas de recadrage automatique — fournir des images ~carrées).

### Validation manuelle des propositions

Tout personnage avec `verif_manual = false` (import, génération) doit être relu
avant d'entrer en jeu. Le champ `hidden` sert à archiver les refus.

| État | `verif_manual` | `hidden` | Visible `/characters` | Jouable | Dans `/characters/pending` |
|---|---|---|---|---|---|
| Proposé | false | false | oui (badge « à valider ») | non | oui |
| Validé | true | false | oui | oui | non |
| Refusé | false | true | non | non | non (jamais reproposé) |

Front : à l'ouverture de `/characters`, si des binômes sont en attente, une popup
les présente un par un (univers, cosmos, noms, mots interdits, niveau). Les **3
mots interdits de chaque personnage sont éditables directement dans la popup**
(champ + bouton « vider ») ; les changements sont enregistrés via
`PATCH /characters/{id}/forbidden-words` au moment du clic sur *Accepter*.
**Refuser** → `hidden = true`. **Accepter** → (sauvegarde des mots interdits
modifiés puis) import d'image obligatoire pour les 2 personnages, puis
`verif_manual = true`. Fermer la popup en cours de route laisse le binôme en
attente (il réapparaîtra ; les mots interdits déjà sauvegardés sont conservés).

Import d'image (partout dans l'app : fiche, édition, popup de validation) : même
composant `ImageDropzone` (glisser-déposer **ou** clic) → `ImageCropperModal`
(recadrage carré imposé) → sortie normalisée **512×512 JPEG q0.9**. Toutes les
images du jeu ont donc le même format.

Un personnage créé via la fiche de perso classique est directement `verif_manual = true`.

---

## Architecture du projet

```
app/
├── Enums/
│   ├── GameStatus.php          # waiting | in_progress | finished
│   ├── GameMode.php            # random | cosmos
│   └── ActionType.php          # question | accusation
│
├── Events/
│   ├── GameStarted.php
│   ├── RoundStarted.php
│   ├── ActionPlayed.php
│   ├── BinomeDiscovered.php
│   ├── GameEnded.php
│   ├── PlayerJoined.php        # broadcast quand un joueur rejoint le lobby
│   ├── PlayerReady.php         # broadcast quand un joueur toggle son statut prêt
│   ├── PlayerLeft.php          # broadcast quand un joueur quitte le lobby
│   └── RoomSettingsUpdated.php # broadcast quand l'hôte change le mode de jeu / le cosmos
│
├── Console/
│   └── Commands/
│       └── ImportCharacters.php         # `php artisan characters:import`
│
├── Http/
│   ├── Controllers/
│   │   ├── BroadcastAuthController.php  # auth custom PresenceChannel (sans Sanctum)
│   │   ├── CosmosController.php
│   │   ├── UniverseController.php
│   │   ├── CharacterController.php
│   │   ├── RoomController.php
│   │   ├── GameController.php
│   │   └── ActionController.php
│   └── Requests/
│       ├── StoreCosmosRequest.php / UpdateUniverseRequest.php / StoreUniverseRequest.php
│       ├── StoreCharacterRequest.php
│       ├── StoreRoomRequest.php
│       ├── JoinRoomRequest.php
│       ├── PlayQuestionRequest.php
│       └── PlayAccusationRequest.php
│
├── Models/
│   ├── Cosmos.php              # sur-catégorie regroupant plusieurs univers (table `cosmos`, invariable)
│   ├── Universe.php
│   ├── Character.php
│   ├── Room.php
│   ├── Player.php              # étend Authenticatable (requis pour auth broadcasting)
│   ├── Game.php
│   ├── Binome.php
│   ├── Round.php
│   └── Action.php
│
├── Services/
│   ├── GameService.php         # orchestration démarrage de partie
│   ├── RoundService.php        # gestion des tours
│   ├── ActionService.php       # validation questions + accusations
│   └── BinomeService.php       # découverte binome + fin de partie
│
└── Support/
    └── CharacterImporter.php   # import JSON en masse (partagé commande + seeder)

database/
├── data/
│   ├── characters.json         # jeu de personnages importé par CharacterSeeder
│   └── images/                 # images référencées à l'import
├── seeders/
│   ├── DatabaseSeeder.php
│   └── CharacterSeeder.php
└── migrations/
    ├── create_cosmos_table.php
    ├── create_universes_table.php   # + add_cosmos_id_to_universes_table
    ├── create_characters_table.php
    ├── create_rooms_table.php       # + add_game_mode_to_rooms_table (game_mode, cosmos_id)
    ├── create_players_table.php
    ├── create_room_player_table.php
    ├── create_games_table.php
    ├── create_binomes_table.php
    ├── create_binome_player_table.php
    ├── create_rounds_table.php
    └── create_actions_table.php

routes/
├── api.php                     # endpoints REST
├── channels.php                # autorisation WebSocket PresenceChannel (via X-Player-Id)
└── web.php
```

---

## Modèle de données

### Vue d'ensemble des relations

```
Cosmos                          ← sur-catégorie (ex : « Disney », « Animé Japonais »)
  └── hasMany → Universe

Universe
  └── belongsTo → Cosmos       (cosmos_id, nullable)
  └── hasMany → Character
  └── hasMany → Binome

Character
  └── belongsTo → Universe
  └── json: forbidden_words[]
  └── bool: verif_manual   (relu/confirmé par un humain — défaut false ; true si créé via la fiche)
  └── bool: hidden         (proposition refusée — conservée mais jamais affichée ni jouée)
  (le cosmos d'un personnage est celui de son univers : Character → Universe → Cosmos)
  scopes : Character::playable()  (verif_manual && !hidden) / Character::pendingValidation() (!verif_manual && !hidden)

Room
  └── belongsToMany → Player  (pivot: is_ready)
  └── hasMany → Game
  └── created_by → Player

Player                          ← étend Authenticatable
  └── belongsToMany → Room    (pivot: is_ready)
  └── belongsToMany → Binome  (pivot: character_id, score)

Game
  └── belongsTo → Room
  └── hasMany → Binome
  └── hasMany → Round
  └── enum status: waiting | in_progress | finished

Binome
  └── belongsTo → Game
  └── belongsTo → Universe
  └── belongsToMany → Player  (pivot: character_id, score)
  └── discovered_by_player_id → Player (nullable)

Round
  └── belongsTo → Game
  └── current_player_id → Player (qui joue ce tour)
  └── bool: is_finished

Action
  └── belongsTo → Round
  └── belongsTo → Player      (qui joue)
  └── belongsTo → Player      (target_player_id, nullable)
  └── enum type: question | accusation
  └── text: content            (contenu de la question ou nom du personnage accusé)
  └── bool: is_valid           (false si mot interdit)
  └── bool: accusation_correct (nullable, uniquement pour les accusations)
  └── string: answer           (nullable — 'yes' | 'no' | 'dont_know')
```

### Tables pivots

**`room_player`**
| Colonne | Type | Description |
|---|---|---|
| room_id | FK | |
| player_id | FK | |
| is_ready | boolean | Toggle — le joueur peut activer/désactiver son statut prêt |

**`binome_player`**
| Colonne | Type | Description |
|---|---|---|
| binome_id | FK | |
| player_id | FK | |
| character_id | FK | Personnage secret assigné à ce joueur |
| score | integer | Score du joueur dans cette partie |

### Champs importants sur `rooms`
```php
$table->foreignId('created_by')->constrained('players');       // seul l'hôte peut start
$table->string('game_mode')->default('random');                // random | cosmos (App\Enums\GameMode)
$table->foreignId('cosmos_id')->nullable()->constrained('cosmos')->nullOnDelete(); // mode cosmos uniquement
```

---

## Services

### `GameService`

Point d'entrée : `start(Room $room): Game`

1. Valide que le nombre de joueurs est pair et ≥ 4, et que tous sont `is_ready`
2. Crée la `Game` avec le statut `in_progress`
3. Mélange aléatoirement les joueurs → forme des paires → `resolveUniverses()` tire un `Universe` distinct par paire → assigne 2 `Character` distincts du même univers et du même niveau
   - **Seuls les personnages `playable()` sont tirés** (`verif_manual = true`, `hidden = false`) ; la sélection d'univers ne retient que ceux ayant au moins un binôme jouable au même niveau. Une proposition non validée n'arrive donc jamais en partie.
   - `resolveUniverses()` respecte `room.game_mode` : en mode `cosmos`, les univers sont filtrés sur `room.cosmos_id` ; s'il n'y en a pas assez, une `Exception` est levée (renvoyée en `422` par `GameController::start`, qui enveloppe désormais l'appel dans un `try/catch`).
4. Délègue la création du premier round à `RoundService::createRound()`
5. Broadcast `GameStarted`

> Tout est wrappé dans une `DB::transaction()`.

---

### `RoundService`

**`createRound(Game $game, int $roundNumber): Round`**
- Récupère les joueurs triés par `id` (ordre déterministe et constant)
- Crée le round avec `current_player_id` = premier joueur
- Broadcast `RoundStarted`

**`nextTurn(Round $round): bool`**
- Passe au joueur suivant dans la liste ordonnée
- Retourne `true` si tous les joueurs ont joué (round terminé)
- Si terminé, met `is_finished = true` sur le round

---

### `ActionService`

**`playQuestion(Round, Player, Player $target, string $question): Action`**
1. Vérifie que c'est le tour du joueur (`current_player_id`)
2. Vérifie que le joueur n'a pas déjà joué ce round (action valide existante)
3. Récupère le personnage du joueur et vérifie les mots interdits
4. Si mot interdit → enregistre `is_valid: false`, broadcast, **tour perdu**, appelle `advanceRound()`
5. Sinon → enregistre `is_valid: true`, broadcast — **attend la réponse avant d'avancer**

**`playAnswer(Action, Player, string $answer): Action`**
1. Vérifie que c'est bien le joueur ciblé qui répond
2. Vérifie que la question n'a pas déjà une réponse
3. Enregistre la réponse (`yes` | `no` | `dont_know`)
4. Broadcast `AnswerGiven`
5. Appelle `advanceRound()` — c'est ici que le tour avance

**`playAccusation(Round, Player, Player $target, Character $guessed): Action`**
1. Mêmes validations que la question
2. Compare le personnage accusé avec le vrai personnage du joueur ciblé
3. Enregistre l'action avec `accusation_correct: true/false`
4. Si correct → `handleCorrectAccusation()` → `BinomeService::discoverBinome()`
5. Vérifie si c'était le dernier binome → `endGame()` si oui
6. Appelle `advanceRound()`

**`advanceRound(Round, Game)`**
- Appelle `RoundService::nextTurn()`
- Si le round est terminé et la partie toujours `in_progress` → crée le round suivant

---

### `BinomeService`

**`discoverBinome(Game, Player $target, Player $discoveredBy): Binome`**
- Trouve le binome du joueur ciblé dans la partie
- Met `is_discovered: true` et `discovered_by_player_id`
- Broadcast `BinomeDiscovered` (révèle les personnages du binome)

**`getRemainingBinomes(Game): Collection`**
- Retourne les binomes encore non découverts (utile pour le front)

---

## Controllers & Endpoints

### Tableau complet des routes

| Méthode | Route | Controller | Description |
|---|---|---|---|
| `POST` | `/api/access/characters` | `AccessController@characters` | Échange le mot de passe d'accès personnages (`password`) contre un `token` — `422` si incorrect |
| `GET` | `/api/cosmos` | `CosmosController@index` | Liste des cosmos (+ nb d'univers, + `playable_universe_count`) — **route ouverte** (lobby) |
| `POST` | `/api/cosmos` | `CosmosController@store` | Créer un cosmos — 🔒 `characters.access` |
| `GET` | `/api/universes` | `UniverseController@index` | Liste des univers (+ cosmos rattaché) |
| `POST` | `/api/universes` | `UniverseController@store` | Créer un univers (`cosmos_id` optionnel) |
| `PATCH` | `/api/universes/{universe}` | `UniverseController@update` | Modifier le cosmos d'un univers |
| `GET` | `/api/characters` | `CharacterController@index` | Liste des personnages non masqués (+ univers, cosmos, `verif_manual`) |
| `GET` | `/api/characters/pending` | `CharacterController@pending` | Binômes proposés en attente de validation (groupés par univers + niveau) |
| `POST` | `/api/characters/validation/accept` | `CharacterController@accept` | Valider un binôme (`character_ids[]`) — refuse si un perso n'a pas d'image |
| `POST` | `/api/characters/validation/reject` | `CharacterController@reject` | Refuser un binôme (`character_ids[]`) → `hidden = true` |
| `PATCH` | `/api/characters/{character}/forbidden-words` | `CharacterController@updateForbiddenWords` | Modifier uniquement les 3 mots interdits (édition rapide) |
| `POST` | `/api/characters/{character}/image` | `CharacterController@uploadImage` | Importer / remplacer l'image d'un personnage |
| `POST` | `/api/universes/{universe}/characters` | `CharacterController@store` | Créer un personnage (`verif_manual = true`) |
| `PUT` | `/api/characters/{character}` | `CharacterController@update` | Modifier un personnage (`universe_id` optionnel pour le réassigner à un autre univers) |
| `DELETE` | `/api/characters/{character}` | `CharacterController@destroy` | Supprimer un personnage |
| `POST` | `/api/rooms` | `RoomController@store` | Créer un salon |
| `POST` | `/api/rooms/join` | `RoomController@join` | Rejoindre avec un code à 6 caractères |
| `GET` | `/api/rooms/{room}` | `RoomController@show` | État du salon + liste joueurs + `game_mode` / `cosmos_id` |
| `PATCH` | `/api/rooms/{room}/settings` | `RoomController@updateSettings` | Hôte : mode de jeu (`random` / `cosmos` + `cosmos_id`) → broadcast `RoomSettingsUpdated` |
| `PATCH` | `/api/rooms/{room}/ready` | `RoomController@ready` | Toggle prêt/pas prêt |
| `DELETE` | `/api/rooms/{room}/leave` | `RoomController@leave` | Quitter le salon (supprime le joueur en DB) |
| `POST` | `/api/rooms/{room}/start` | `GameController@start` | Lancer la partie (créateur uniquement) |
| `GET` | `/api/games/{game}` | `GameController@show` | État de la partie (sans personnages) |
| `GET` | `/api/games/{game}/me` | `GameController@myCharacter` | Mon personnage secret + mots interdits |
| `POST` | `/api/games/{game}/rounds/{round}/question` | `ActionController@question` | Poser une question |
| `POST` | `/api/games/{game}/rounds/{round}/accusation` | `ActionController@accusation` | Faire une accusation |
| `POST` | `/broadcasting/auth` | `BroadcastAuthController@authenticate` | Auth custom PresenceChannel |
| `POST` | `/api/games/{game}/rounds/{round}/actions/{action}/answer` | `ActionController@answer` | Répondre à une question (oui/non/je ne sais pas) |

### Accès à la gestion des personnages

Les routes `/api/universes*`, `/api/characters*` et `POST /api/cosmos` sont
protégées par le middleware `characters.access` (`EnsureCharactersAccess`,
alias déclaré dans `bootstrap/app.php`) :

- Le mot de passe est lu dans `config('access.characters_password')` →
  `CHARACTERS_ACCESS_PASSWORD` du `.env`. **Vide = accès refusé** (`403`).
- `POST /api/access/characters` vérifie le mot de passe et renvoie un `token`
  stable (`hash_hmac`, non réversible).
- Le client renvoie ce token dans l'en-tête `X-Characters-Access` sur chaque
  requête protégée ; le middleware le compare via `hash_equals`.
- Côté front, le token est persisté dans `localStorage` (`charactersAccess`),
  les onglets « Personnages » / « Créer » et leurs routes ne sont visibles
  qu'une fois déverrouillé (cf. `frontend/README.md`).
- `GET /api/cosmos` reste **ouvert** : le lobby en a besoin pour le mode « cosmos ».

### Sécurité des données

- `GET /games/{game}` → ne retourne **jamais** les personnages des joueurs
- `GET /games/{game}/me` → retourne le personnage **uniquement au joueur concerné** (via `player_id` en query param)
- Les personnages ne sont révélés publiquement que dans les events `BinomeDiscovered` et `GameEnded`

---

## Events WebSocket

### Channels utilisés

| Channel | Type | Usage |
|---|---|---|
| `room.{roomId}` | PresenceChannel | Lobby — joueurs qui rejoignent/quittent/sont prêts |
| `game.{gameId}` | PresenceChannel | Partie en cours — actions, rounds, fin de partie |
| `AnswerGiven` | `game.{id}` | `answer.given` | Le joueur ciblé répond à une question |

### Auth PresenceChannel

L'authentification ne passe **pas** par Sanctum. Elle utilise un header custom `X-Player-Id` :

```
POST /broadcasting/auth
Header: X-Player-Id: {playerId}
```

Le `BroadcastAuthController` récupère le joueur via ce header et appelle `auth()->setUser($player)` avant `Broadcast::auth($request)`.

La route par défaut de Laravel Broadcasting est désactivée — seule la route custom est enregistrée via `AppServiceProvider`.

### Récapitulatif des events

| Event | Channel | Nom broadcast | Déclencheur |
|---|---|---|---|
| `PlayerJoined` | `room.{id}` | `player.joined` | Quelqu'un rejoint le lobby |
| `PlayerReady` | `room.{id}` | `player.ready` | Toggle prêt/pas prêt |
| `PlayerLeft` | `room.{id}` | `player.left` | Quelqu'un quitte le lobby |
| `RoomSettingsUpdated` | `room.{id}` | `room.settings.updated` | L'hôte change le mode de jeu / le cosmos |
| `GameStarted` | `room.{id}` + `game.{id}` | `game.started` | Hôte lance la partie |
| `RoundStarted` | `game.{id}` | `round.started` | Nouveau round créé |
| `ActionPlayed` | `game.{id}` | `action.played` | Un joueur joue son tour |
| `BinomeDiscovered` | `game.{id}` | `binome.discovered` | Accusation correcte |
| `GameEnded` | `game.{id}` | `game.ended` | Dernier binome découvert |

### `ActionPlayed` — logique de masquage

```
type = question
  ├── is_valid: false  → question: null, refused_reason: "Mot interdit détecté."
  └── is_valid: true   → question: "Le contenu de la question"

type = accusation
  ├── accusation_correct: false → character_name: null
  └── accusation_correct: true  → character_name: "Nom du personnage"
```

---

## Flux complet d'une partie

```
1. LOBBY
   POST /api/rooms                      → créer salon (code 6 chars généré auto)
   POST /api/rooms/join                 → rejoindre avec le code
   → broadcast: PlayerJoined           → tous voient le nouveau joueur
   PATCH /api/rooms/{room}/settings     → hôte : mode de jeu (random / cosmos)
   → broadcast: RoomSettingsUpdated    → tous voient le mode choisi
   PATCH /api/rooms/{room}/ready        → toggle prêt/pas prêt
   → broadcast: PlayerReady            → tous voient le statut mis à jour
   DELETE /api/rooms/{room}/leave       → quitter (supprime le joueur en DB)
   → broadcast: PlayerLeft             → tous voient la liste mise à jour
   POST /api/rooms/{room}/start         → créateur lance la partie

2. DÉMARRAGE (GameService)
   - Mélange aléatoire des joueurs
   - Formation des binomes par paires
   - Assignation d'un univers par binome
   - Assignation d'un personnage distinct par joueur
   - Création du Round n°1
   → broadcast: GameStarted (sur room.{id} ET game.{id})
   → broadcast: RoundStarted

3. TOUR D'UN JOUEUR
   GET /api/games/{game}/me             → récupère son personnage + mots interdits

   [Option A] Question
    POST .../rounds/{round}/question
    body: { player_id, target_player_id, question }
    ├── mot interdit → is_valid: false, tour perdu, advanceRound()
    └── valide       → is_valid: true, broadcast ActionPlayed
                     → joueur ciblé répond via POST .../actions/{action}/answer
                     → broadcast AnswerGiven
                     → advanceRound() déclenché après la réponse

   [Option B] Accusation
   POST .../rounds/{round}/accusation
   body: { player_id, target_player_id, character_id }
     ├── fausse  → accusation_correct: false
     └── correcte → BinomeService::discoverBinome()
                    → broadcast: BinomeDiscovered
                    └── dernier binome ? → endGame()
                                          → broadcast: GameEnded

4. FIN DE TOUR
   RoundService::nextTurn()
     ├── joueur suivant → mise à jour current_player_id
     └── tous ont joué  → is_finished: true
                          → RoundService::createRound(n+1)
                          → broadcast: RoundStarted

5. FIN DE PARTIE
   - Status Game → finished
   - Le dernier binome non découvert = gagnant
   → broadcast: GameEnded (avec winning_binomes + révélation complète)
```

---

## Points d'attention importants

### `ShouldBroadcastNow` sur tous les events
Tous les events utilisent `ShouldBroadcastNow` (pas `ShouldBroadcast`) pour bypasser la queue en développement. En production, repasser sur `ShouldBroadcast` avec une queue Redis.

### `Player` étend `Authenticatable`
```php
use Illuminate\Foundation\Auth\User as Authenticatable;
class Player extends Authenticatable { ... }
```
Requis pour que `auth()->setUser($player)` fonctionne dans `BroadcastAuthController`.

### Route broadcasting/auth custom
La route par défaut Laravel est désactivée en retirant `channels:` de `withRouting()` dans `bootstrap/app.php`. Les channels sont chargés manuellement dans `AppServiceProvider::boot()`.

```php
// AppServiceProvider.php
public function boot(): void
{
    Route::middleware('web')
        ->post('/broadcasting/auth', [BroadcastAuthController::class, 'authenticate']);
    require base_path('routes/channels.php');
}
```

### CSRF désactivé sur `/broadcasting/auth`
```php
// bootstrap/app.php
$middleware->validateCsrfTokens(except: ['broadcasting/auth']);
```

### Toggle ready
`PATCH /rooms/{room}/ready` **inverse** le statut actuel du joueur (pas un simple `true`).

### Leave room
`DELETE /rooms/{room}/leave` supprime le joueur en base + détache le pivot. Si la room est vide → supprime la room. Si l'hôte part → transfère `created_by` au prochain joueur.

### Enums natifs Laravel 11
```php
enum GameStatus: string {
    case Waiting    = 'waiting';
    case InProgress = 'in_progress';
    case Finished   = 'finished';
}
protected $casts = ['status' => GameStatus::class];
```

### Ordre des joueurs
Trié par `player.id` — déterministe et identique à chaque round.

### Transaction sur le démarrage
`GameService::start()` est entièrement dans une `DB::transaction()`.

### `is_new_round` dans `RoundStarted`
L'event `RoundStarted` est broadcasté dans deux cas :
- Changement de joueur en cours de round (`is_new_round: false`)
- Création d'un nouveau round (`is_new_round: true`)

Le front utilise ce flag pour ne déclencher l'animation de transition que lors d'un vrai changement de round.
