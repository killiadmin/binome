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
- Nombre de joueurs : **minimum 4**, pair ou impair
- Au début de chaque partie, chaque joueur reçoit un **personnage secret** tiré d'un univers (ex : Disney, Marvel…)
- Les joueurs sont regroupés en **binomes** de 2 personnes partageant le même univers, **à leur insu**
- Chaque personnage possède **3 mots interdits**

#### L'orphelin (nombre impair)

Si les joueurs sont en nombre impair, le joueur surnuméraire du tirage se retrouve
**seul dans son binome** : c'est l'**orphelin** (`binomes.is_orphan = true`, un seul
`binome_player`). Il reçoit un personnage et un univers comme les autres, dans un
univers qui aurait pu porter un vrai binome — rien ne le distingue.

- **L'orphelin ignore qu'il est orphelin** : aucune route ni aucun event ne le lui dit
  avant la fin. `GET /games/{game}` et l'event `GameStarted` exposent seulement
  `has_orphan: true` — un orphelin existe, sans dire qui.
- Il compte comme **une équipe à part entière** : tant qu'il est en vie son équipe est
  intacte, donc s'il est le dernier en jeu il gagne **et touche le bonus de +5**, comme
  un binome complet. C'est ce qui le distingue d'un rescapé (joueur dont le partenaire
  a été éliminé), qui gagne sans bonus.
- Il est révélé à la fin, dans `GameEnded.all_binomes[].is_orphan`.

### Modes de jeu

L'hôte choisit le mode dans le lobby (`PATCH /rooms/{room}/settings`) :

| Mode (`rooms.game_mode`) | Comportement |
|---|---|
| `random` (défaut) | Chaque binome reçoit un **univers tiré au hasard** parmi les univers jouables. |
| `cosmos` | L'hôte fixe un **cosmos** (`rooms.cosmos_id`) : tous les binomes reçoivent un univers **de ce cosmos**. |

Dans les deux modes : **un univers distinct par binome**, et binome = 2 persos du même univers au
même `level_affectation`. Un univers est « jouable » s'il a ≥ 2 personnages `playable()` partageant
un niveau. Le nombre de personnages par niveau n'est **pas** limité en saisie manuelle (fiche /
édition) pour permettre toute correction d'affectation ; si un niveau en compte plus de 2, le
tirage en prend 2 au hasard et la liste `/admin/binomes` signale l'anomalie. L'orphelin consomme lui aussi un univers : le mode `cosmos` exige donc
**≥ ceil(nb de joueurs / 2) univers jouables** dans le cosmos, sinon le démarrage renvoie une
erreur `422` (message nommant le cosmos et les compteurs).
`GET /cosmos` expose `playable_universe_count` par cosmos pour l'affichage côté lobby.

### Vote des personnages
Avant le round 1, chaque joueur **accepte son personnage** ou **demande à en changer** : un joueur
qui ne connaît pas son personnage donnerait de fausses réponses. Les deux votes sont publics
(compteurs « Prêts : x / n » et « Demandes à changer : y / n ») et **rien ne se passe tant que
tous n'ont pas voté** (`binome_player.character_vote` : `null` | `accepted` | `change`). Ensuite :

- **Au moins une demande de changement** → binomes, univers et personnages sont **retirés pour
  tout le monde** (`games.draw_number` + 1) et chacun revote. Les personnages refusés sont notés
  dans `games.rejected_character_ids` et écartés des tirages suivants — sauf si le catalogue ne
  suffit plus sans eux, auquel cas on tire sur tous les personnages.
- **Tous ont accepté** → `games.characters_confirmed_at` est renseigné et le round 1 est créé.
- Un vote est définitif pour le tirage en cours.
- Tant que le vote dure, il n'existe aucun round : personne ne peut jouer, et l'hôte ne peut
  ni passer un tour ni exclure (`409`). Un joueur déconnecté bloque donc le vote jusqu'à son retour.

### Déroulement
- Les joueurs jouent **chacun leur tour**, dans le même ordre à chaque round
- À son tour, un joueur peut effectuer **une seule action** parmi deux :
    1. **Poser une question** à n'importe quel autre joueur (réponse libre : oui/non)
    2. **Faire une accusation** : désigner un joueur et nommer précisément son personnage

#### « Je ne sais pas » → tirage de carte
- Si la cible répond **« Je ne sais pas »**, le questionneur (A) **tire une carte face cachée**
  parmi les mots interdits de la cible (B) : un mot tiré **au hasard** (le paquet est complet à
  chaque tirage, un mot peut retomber), que **lui seul** voit pendant **5 secondes**
- A décide ensuite de **montrer ce mot à tous les autres joueurs, sauf B** (eux aussi le voient
  5 s), ou de le **garder pour lui**
- B sait qu'une carte a été tirée, mais **jamais laquelle**, ni si elle a été montrée
- Le tour **attend** A : il a **30 s** (`ActionService::CARD_DRAW_SECONDS`) pour tirer et décider,
  sinon la carte est perdue et le tour avance. Une cible sans mot interdit → pas de tirage

### Mots interdits
- Chaque joueur a **ses propres mots interdits** liés à **son personnage**
- Si un joueur pose une question contenant l'un de ses mots interdits, **la question est refusée**
- Le joueur **perd son tour** (l'action est enregistrée comme invalide)
- Exemple : personnage = Tarzan, mot interdit = "jungle" → la question "Vit-il dans la jungle ?" est refusée

> ⚠️ Ce sont les mots interdits du **joueur qui pose la question** (liés à son propre personnage) qui filtrent, pas ceux du joueur interrogé. La vérification est uniquement côté backend — le joueur interrogé répond librement sans contrainte backend.

### Victoire
- Quand une accusation est correcte, le **joueur ciblé est éliminé** (son binome n'est pas révélé)
- On appelle **équipe intacte** un binome dont aucun membre n'a été éliminé : une paire complète,
  ou l'orphelin encore en vie. Un **rescapé** est un joueur encore actif dont le partenaire est tombé.
- La partie se termine quand il ne reste **qu'une seule équipe intacte et aucun rescapé** → cette
  équipe gagne ; **ou** plus aucune équipe intacte et **un seul rescapé** → il gagne seul
- Bonus de score : **+5 pour une victoire avec l'équipe intacte** — donc pour un binome complet
  ou pour l'orphelin — et **+3 pour un rescapé** qui gagne seul (sinon il pouvait finir à égalité
  avec des perdants). Constantes `ScoreService::WIN_BONUS_INTACT` / `WIN_BONUS_SURVIVOR`.

### Joueurs éliminés (options de l'hôte, actives par défaut)
Réglées dans le lobby (`PATCH /rooms/{room}/settings` : `reveal_to_eliminated`, `best_question_vote`) :
- **Vue omnisciente** — `GET /games/{game}/reveal?player_id=` : un joueur **éliminé** voit tous les
  binomes, personnages et mots interdits (`403` sinon). Seule exception à « personnages révélés
  seulement à la fin » : l'éliminé ne joue plus.
- **Meilleure question du round** — `POST /games/{game}/question-votes` (`player_id`, `action_id`) :
  un éliminé vote (et peut changer d'avis) pour une question du round en cours ou du précédent,
  ni la sienne ni celle de son binôme (sinon il voterait toujours pour son partenaire). Votes
  publics (`question_votes` dans `GET /games/{game}`, event `QuestionVoted`). À la fin, l'auteur
  de la question la plus votée de chaque round marque **+1** (ex æquo compris) →
  `game_stats.best_questions`, `recap.best_questions`. Voir `QuestionVoteService`.

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

# 6. Terminal 3 — Queue (expiration du tirage de carte : job ExpireCardDraw)
php artisan queue:listen --tries=1
```

> ⚠️ Tous les events utilisent `ShouldBroadcastNow` (ils ne passent pas par la queue). La queue
> reste **nécessaire** pour `ExpireCardDraw` : sans worker, un tirage de carte abandonné bloque
> le tour jusqu'à ce que l'hôte le passe (joueur déconnecté) ou que le questionneur décide.

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
  vérifiée **à l'import uniquement** : une ligne en trop est signalée en erreur,
  les autres passent (l'édition manuelle, elle, ne bloque jamais).
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
| Refusé | false | true | oui (filtre « Binômes refusés ») | non | non (jamais reproposé automatiquement) |

Un binôme refusé n'est pas perdu : dans la liste, le bouton **« Remettre à
valider »** de son groupe (`POST /characters/validation/restore` → `hidden =
false`, `verif_manual = false`) le renvoie dans la popup de validation, et la
modale d'édition expose un champ **Statut** (`validated` / `pending` /
`rejected`, envoyé en `status` sur `PUT /characters/{id}` ; `validated` exige
une image).

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
  └── belongsToMany → Player  (pivot: character_id, score, is_eliminated)
  └── bool: is_orphan          (binome d'un seul joueur — nombre de joueurs impair)
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
  └── string: answer_corrected_from (nullable — réponse d'origine du joueur si l'hôte l'a corrigée ;
                                     remise à null si la correction revient à l'origine)
  └── datetime: answer_corrected_at (nullable — date de la dernière correction)
  └── string: card_draw_status (nullable — 'pending' | 'drawn' | 'shared' | 'kept' | 'expired')
  └── int: card_count          (nullable — cartes proposées = mots interdits de la cible)
  └── datetime: card_draw_deadline (nullable — au-delà, ExpireCardDraw clôt le tirage)
  └── string: drawn_word       (nullable — SECRET, dans Action::$hidden)
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
| character_vote | string nullable | Vote sur son personnage avant le round 1 : `accepted` \| `change` (`null` = pas encore voté) |
| is_eliminated | boolean | Le joueur a été correctement accusé — il devient spectateur |

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

1. Valide que le nombre de joueurs est ≥ 4 (pair ou impair) et que tous sont `is_ready`
2. Crée la `Game` avec le statut `in_progress`
3. Mélange aléatoirement les joueurs → forme des paires → `resolveUniverses()` tire un `Universe` distinct par paire → assigne 2 `Character` distincts du même univers et du même niveau
   - **Nombre impair** : la dernière « paire » ne contient qu'un joueur → binome `is_orphan = true` avec un seul personnage (voir « L'orphelin » plus haut)
   - **Seuls les personnages `playable()` sont tirés** (`verif_manual = true`, `hidden = false`) ; la sélection d'univers ne retient que ceux ayant au moins un binôme jouable au même niveau. Une proposition non validée n'arrive donc jamais en partie.
   - `resolveUniverses()` respecte `room.game_mode` : en mode `cosmos`, les univers sont filtrés sur `room.cosmos_id` ; s'il n'y en a pas assez, une `Exception` est levée (renvoyée en `422` par `GameController::start`, qui enveloppe désormais l'appel dans un `try/catch`).
4. Broadcast `GameStarted` — **sans round** : `characters_confirmed_at` reste `null`, le vote commence

> Tout est wrappé dans une `DB::transaction()`.

**`voteCharacter(Game, Player, int $characterId, bool $accepted)`** — voir *Vote des personnages*
- Verrouille la ligne `games` (`lockForUpdate`) : le dernier vote peut remplacer tous les
  personnages, les votes simultanés passent l'un après l'autre.
- Tant que des joueurs n'ont pas voté → simple `CharacterVoteCast`.
- `$characterId` = le personnage que le joueur a sous les yeux. S'il ne correspond plus (retirage
  passé entre-temps), le vote est rejeté — sinon il validerait un personnage jamais vu.
- Tous ont voté, au moins un `change` → `redraw()` : supprime les binomes (cascade `binome_player`) et relance `assignBinomes()`
  avec **les joueurs de la partie**, pas ceux du salon (un spectateur arrivé depuis n'entre pas).
- Tous ont voté `accepted` → `characters_confirmed_at = now()` puis `RoundService::createRound(1)`.
- `CharacterVoteCast` / `CharactersRedrawn` sont diffusés **après le commit** : à la réception,
  les clients relisent `/me` et doivent trouver le nouveau tirage.

**`characterVotes(Game)`** → `{accepted, change` (ids des joueurs), `change_character_ids, total}`.

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
   - **sauf** `dont_know` sur une cible qui a des mots interdits : ouvre un tirage de carte
     (`card_draw_status: pending`, échéance à 30 s), dispatche `ExpireCardDraw` avec ce délai,
     et le tour attend le questionneur

**`drawCard(Action, Player, int $card): string`** — le questionneur retourne une carte : un mot
interdit de la cible tiré au hasard (`Arr::random`, l'index de carte est cosmétique). Le mot n'est
renvoyé **que** dans la réponse HTTP ; broadcast public `CardDrawn` (sans le mot).

**`decideCardDraw(Action, Player, bool $share)`** — `shared` → `CardWordShared` sur le channel
privé `player.{id}` de chaque joueur / spectateur **sauf la cible** (et le questionneur) ; `kept` →
rien. Puis `CardDrawFinished` (muet sur l'issue) et `advanceRound()`.

**`expireCardDraw(int $actionId)`** — délai dépassé (`ExpireCardDraw`, ou l'hôte qui passe le
tour, `reason: draw`) : `expired`, `CardDrawFinished`, `advanceRound()`. Toutes les transitions
sont des mises à jour conditionnelles sur `card_draw_status` : tirage, décision et expiration
simultanés → un seul l'emporte.

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
| `POST` | `/api/characters/validation/restore` | `CharacterController@restore` | Repêcher un binôme refusé (`character_ids[]`) → `hidden = false`, `verif_manual = false` |
| `PATCH` | `/api/characters/{character}/forbidden-words` | `CharacterController@updateForbiddenWords` | Modifier uniquement les 3 mots interdits (édition rapide) |
| `POST` | `/api/characters/{character}/image` | `CharacterController@uploadImage` | Importer / remplacer l'image d'un personnage |
| `POST` | `/api/universes/{universe}/characters` | `CharacterController@store` | Créer un personnage (`verif_manual = true`) |
| `PUT` | `/api/characters/{character}` | `CharacterController@update` | Modifier un personnage (`universe_id` optionnel pour le réassigner à un autre univers, `status` optionnel `validated`/`pending`/`rejected`) |
| `DELETE` | `/api/characters/{character}` | `CharacterController@destroy` | Supprimer un personnage |
| `GET` | `/api/chat/messages` | `ChatController@index` | 50 derniers messages du chat commun — **route ouverte** |
| `POST` | `/api/chat/messages` | `ChatController@store` | Poster un message (`body`, `player_id` **ou** `anon_id`) → broadcast `ChatMessageSent` — `throttle:chat` |
| `GET` | `/api/history/games` | `HistoryController@games` | Parties **terminées** uniquement — **route ouverte** |
| `GET` | `/api/history/games/{game}` | `HistoryController@game` | Détail complet d'une partie terminée (`404` si en cours) — **route ouverte** |
| `GET` | `/api/history/leaderboard` | `HistoryController@leaderboard` | Classement cumulé par pseudo — **route ouverte** |
| `GET` | `/api/history/players?pseudo=…` | `HistoryController@player` | Totaux d'un joueur + détail partie par partie — **route ouverte** |
| `GET` | `/api/avatars/{avatar}` | `AvatarController@show` | Image JPEG d'une photo de profil — URL versionnée (`?v=`), cache 1 an — **route ouverte** |
| `GET` | `/api/character-images/{character}` | `CharacterImageController@show` | Image d'un personnage en binaire — URL versionnée (`?v=`) renvoyée par `GET /games/{game}/me` et par toutes les routes `/characters*` (`Character::imageUrl()`), cache 1 an — **route ouverte**. Les listes chargent les personnages via `Character::withoutImage()` (pas de data URI lue en base) |
| `POST` | `/api/players/{player}/avatar` | `AvatarController@update` | Enregistrer sa photo (`player_id` = `{player}`, `image` = data URI JPEG carrée 64–512 px, ≤ 200 ko) → broadcast `PlayerAvatarUpdated` — `throttle:avatars` (10/min) |
| `DELETE` | `/api/players/{player}/avatar` | `AvatarController@destroy` | Retirer sa photo (retour aux initiales) |
| `POST` | `/api/rooms` | `RoomController@store` | Créer un salon — `409` si ce pseudo est **connecté** ailleurs (présence Reverb sur `room.{id}` / `game.{id}`, `PresenceService::connectedRoomFor`) : un pseudo = un joueur, deux appareils mélangeraient leurs sessions. Reverb muet → pas de blocage |
| `POST` | `/api/rooms/join` | `RoomController@join` | Rejoindre avec un code à 6 caractères — même `409` pseudo déjà connecté que `store` |
| `GET` | `/api/rooms/{room}` | `RoomController@show` | État du salon + liste joueurs + `game_mode` / `cosmos_id` |
| `PATCH` | `/api/rooms/{room}/settings` | `RoomController@updateSettings` | Hôte : mode de jeu (`random` / `cosmos` + `cosmos_id`) → broadcast `RoomSettingsUpdated` |
| `PATCH` | `/api/rooms/{room}/ready` | `RoomController@ready` | Toggle prêt/pas prêt |
| `DELETE` | `/api/rooms/{room}/leave` | `RoomController@leave` | Quitter le salon (supprime le joueur en DB) |
| `POST` | `/api/rooms/{room}/start` | `GameController@start` | Lancer la partie (créateur uniquement) |
| `GET` | `/api/games/{game}` | `GameController@show` | État de la partie (sans personnages) |
| `GET` | `/api/games/{game}/me` | `GameController@myCharacter` | Mon personnage secret + mots interdits |
| `POST` | `/api/games/{game}/character-vote` | `GameController@voteCharacter` | Voter sur son personnage : accepter ou demander à changer (`player_id`, `character_id` affiché, `accepted`) — `409` si vote clos, déjà voté ou personnage périmé. `GET /games/{game}` expose `characters_confirmed`, `draw_number`, `accepted_player_ids`, `change_player_ids` |
| `GET` | `/api/games/{game}/reveal` | `GameController@reveal` | Vue omnisciente : tous les personnages — **joueur éliminé** uniquement, si l'hôte l'a activée (`403` sinon) |
| `POST` | `/api/games/{game}/question-votes` | `GameController@voteQuestion` | Un éliminé vote pour la meilleure question du round (`player_id`, `action_id`) — `409` si refusé |
| `GET` | `/api/games/{game}/recap` | `GameController@recap` | Récap de fin de partie (scores, binomes révélés avec image du personnage et photo de chaque joueur, qui a trouvé qui, trophées) — `409` tant que la partie n'est pas terminée |
| `POST` | `/api/games/{game}/reactions` | `GameController@react` | Réaction rapide (`player_id`, `reaction_id` d'une réaction **active** — `404` sinon) → broadcast `ReactionSent` — joueurs **et** spectateurs, `throttle:reactions` (30/min par joueur) |
| `GET` | `/api/reactions` | `CustomReactionController@index` | Réactions actives, ordre de la barre : emojis Unicode puis images (`id`, `name`, `emoji` ou `url`, `created_by`) |
| `POST` | `/api/reactions` | `CustomReactionController@store` | Un joueur crée un emoji image (`player_id`, `name` ≤ 20, `image` = data URI JPEG carrée 64–256 px, ≤ 60 ko) — actif tout de suite, 10 actifs max par pseudo, `throttle:custom-reactions` (5/min par joueur) |
| `GET` | `/api/reactions/{reaction}/image` | `CustomReactionController@image` | Image de l'emoji (URL versionnée, cache long) — servie même masqué |
| `POST` | `/api/games/{game}/skip-turn` | `GameController@skipTurn` | **Hôte** : passe le tour du joueur **déconnecté** qui met la partie en pause (voir *Joueur déconnecté* plus bas) — `409` si refusé |
| `POST` | `/api/games/{game}/actions/{action}/correct-answer` | `GameController@correctAnswer` | **Hôte** : corrige une réponse oui ⇄ non déjà donnée (`{ player_id, answer }`) ; « Je ne sais pas » non corrigeable, le tour n'est pas rejoué — `409` si refusé |
| `POST` | `/api/games/{game}/players/{player}/exclude` | `GameController@excludePlayer` | **Hôte** : exclut un joueur déconnecté — lui et son binôme sont éliminés, il marque 0 point |
| `POST` | `/api/games/{game}/rounds/{round}/question` | `ActionController@question` | Poser une question |
| `POST` | `/api/games/{game}/rounds/{round}/accusation` | `ActionController@accusation` | Faire une accusation |
| `POST` | `/broadcasting/auth` | `BroadcastAuthController@authenticate` | Auth custom PresenceChannel |
| `POST` | `/api/games/{game}/rounds/{round}/actions/{action}/answer` | `ActionController@answer` | Répondre à une question (oui/non/je ne sais pas) |
| `POST` | `/api/games/{game}/rounds/{round}/actions/{action}/draw` | `ActionController@drawCard` | Questionneur : tirer une carte après « Je ne sais pas » (`card` = index) → `{ word, reveal_seconds }` |
| `POST` | `/api/games/{game}/rounds/{round}/actions/{action}/draw/decision` | `ActionController@decideCardDraw` | Questionneur : montrer le mot à tous sauf la cible (`share: true`) ou le garder |
| `GET` | `/api/admin/games` | `AdminGameController@index` | Liste de toutes les parties (en cours + terminées) — 🔒 `characters.access` |
| `GET` | `/api/admin/games/{game}` | `AdminGameController@show` | Détail complet d'une partie (binomes + personnages révélés + journal des actions) — 🔒 `characters.access` |
| `DELETE` | `/api/admin/games/{game}` | `AdminGameController@destroy` | Supprimer une partie (cascade DB : binomes, `binome_player`, rounds, actions, `game_stats`) — 🔒 `characters.access` |
| `GET` | `/api/admin/players` | `AdminPlayerController@index` | Joueurs inscrits (lignes `players`) : pseudo, photo, parties jouées, victoires / points cumulés (`game_stats`), salon actuel, `in_game` — 🔒 `characters.access` |
| `DELETE` | `/api/admin/players/{player}/room` | `AdminPlayerController@removeFromRoom` | Retirer le joueur de son salon (`Room::removePlayer` : hôte transféré, salon vide supprimé, broadcast `PlayerLeft`) — `409` en pleine partie — 🔒 `characters.access` |
| `DELETE` | `/api/admin/players/{player}` | `AdminPlayerController@destroy` | Supprimer le joueur (retiré de ses salons, photo supprimée). S'il a joué : `409` + `games_count`, sauf `?with_games=1` qui supprime aussi ses parties (pour tous leurs joueurs). `409` en pleine partie — 🔒 `characters.access` |
| `GET` | `/api/admin/reactions` | `AdminReactionController@index` | Toutes les réactions (Unicode + images), masquées comprises — 🔒 `characters.access` |
| `POST` | `/api/admin/reactions` | `AdminReactionController@store` | Ajouter une réaction : `{ name, emoji }` (un seul emoji Unicode, unique) ou `{ name, image }` — 🔒 `characters.access` |
| `PATCH` | `/api/admin/reactions/{reaction}` | `AdminReactionController@update` | Renommer (`name`) / masquer-réactiver (`is_enabled`) un emoji — 🔒 `characters.access` |
| `DELETE` | `/api/admin/reactions/{reaction}` | `AdminReactionController@destroy` | Supprimer un emoji — 🔒 `characters.access` |

### Accès à la gestion des personnages

Les routes `/api/universes*`, `/api/characters*`, `/api/admin/*` et
`POST /api/cosmos` sont protégées par le middleware `characters.access`
(`EnsureCharactersAccess`, alias déclaré dans `bootstrap/app.php`) :

- Le mot de passe est lu dans `config('access.characters_password')` →
  `CHARACTERS_ACCESS_PASSWORD` du `.env`. **Vide = accès refusé** (`403`).
- `POST /api/access/characters` vérifie le mot de passe et renvoie un `token`
  stable (`hash_hmac`, non réversible).
- Le client renvoie ce token dans l'en-tête `X-Characters-Access` sur chaque
  requête protégée ; le middleware le compare via `hash_equals`.
- Côté front, le token est persisté dans `localStorage` (`charactersAccess`) ;
  le cadenas de la navbar mène à l'espace d'administration `/admin` (parties,
  binômes, prompts, joueurs, emojis), inaccessible sans token
  (cf. `frontend/README.md`). Un seul mot de passe pour toutes les sections.
- `GET /api/cosmos` reste **ouvert** : le lobby en a besoin pour le mode « cosmos ».

### Sécurité des données

- `GET /games/{game}` → ne retourne **jamais** les personnages des joueurs, ni la composition
  des binomes non découverts : les joueurs arrivent **à plat** dans `players[]`, et seul
  `has_orphan` (booléen) signale qu'un orphelin est en jeu. Renvoyer les binomes trahirait
  l'orphelin, seul groupe à ne compter qu'un joueur.
- `GET /games/{game}/me` → retourne le personnage **uniquement au joueur concerné** (via `player_id` en query param)
  — l'image y est une **URL** (`/api/character-images/{id}?v=…`), pas la data URI : `/me` est relu à chaque retirage. La route d'image est ouverte, mais l'id seul ne dit pas qui joue ce personnage.
- Les personnages ne sont révélés publiquement que dans les events `BinomeDiscovered` et `GameEnded`
- Le mot tiré (`actions.drawn_word`) n'est jamais sérialisé (`Action::$hidden`) : `GET /games`
  n'expose que `card_draw: { status: pending | drawn | done, card_count, deadline }` — `shared`,
  `kept` et `expired` se confondent en `done` pour que la cible ignore si son mot a circulé

---

## Events WebSocket

### Channels utilisés

| Channel | Type | Usage |
|---|---|---|
| `room.{roomId}` | PresenceChannel | Lobby — joueurs qui rejoignent/quittent/sont prêts |
| `game.{gameId}` | PresenceChannel | Partie en cours — actions, rounds, fin de partie |
| `lobby` | Channel **public** | Chat commun du hall — aucune auth, ouvert aux visiteurs sans salon |
| `player.{playerId}` | PrivateChannel | Messages à un seul joueur (mot interdit montré) — seul `X-Player-Id == playerId` s'y abonne. Nécessaire car `game.{id}` atteint aussi la cible |
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
| `ReactionSent` | `game.{id}` | `reaction.sent` | Réaction rapide — `reaction` : `{ id, name, emoji, url, created_by }` (`emoji` Unicode **ou** `url` d'image) ; rien n'est stocké |
| `CharacterVoteCast` | `game.{id}` | `character.vote.cast` | Un joueur a voté (`draw_number`, `accepted_player_ids`, `change_player_ids`) |
| `CharactersRedrawn` | `game.{id}` | `characters.redrawn` | Tous ont voté, au moins une demande de changement : nouveau tirage pour tous (`draw_number`, `change_count`) — chacun relit `/me` |
| `TurnSkipped` | `game.{id}` | `turn.skipped` | L'hôte a passé le tour d'un joueur déconnecté (`reason` : `turn` \| `accusation` \| `draw`) |
| `AnswerGiven` | `game.{id}` | `answer.given` | Réponse donnée — `card_draw` non null si un tirage s'ouvre (« Je ne sais pas ») |
| `AnswerCorrected` | `game.{id}` | `answer.corrected` | L'hôte a corrigé une réponse oui / non (`answer`, `previous_answer`, `answerer`, `host`) |
| `CardDrawn` | `game.{id}` | `card.drawn` | Le questionneur a retourné une carte (`card` = index, **jamais le mot**) |
| `CardDrawFinished` | `game.{id}` | `card.draw.finished` | Tirage clos (montré, gardé ou expiré — l'issue n'est pas dite), le tour reprend |
| `CardWordShared` | `player.{id}` (un par destinataire) | `card.word.shared` | Mot montré : `word`, `reveal_seconds`, `asker`, `target` — tous sauf la cible |
| `PlayerAvatarUpdated` | `room.{id}` + `game.{id}` en cours | `player.avatar.updated` | Un joueur change / retire sa photo (`player_id`, `avatar_url`) |
| `PlayerExcluded` | `game.{id}` | `player.excluded` | L'hôte a exclu un joueur déconnecté (+ `partner` éliminé, `cancelled_action_id`) |
| `ChatMessageSent` | `lobby` | `chat.message` | Un message est posté dans le chat commun |
| `QuestionVoted` | `game.{id}` | `question.voted` | Un éliminé vote pour la meilleure question (`round_id`, tous les `votes` du round) |

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
   → broadcast: GameStarted (sur room.{id} ET game.{id})

2 bis. VOTE DES PERSONNAGES
   POST /api/games/{game}/character-vote
   body: { player_id, character_id, accepted }
     ├── des joueurs n'ont pas voté → broadcast: CharacterVoteCast (compteurs)
     └── tous ont voté
          ├── ≥ 1 demande de changement → nouveau tirage pour tous → broadcast: CharactersRedrawn
          └── tous acceptés → Création du Round n°1 → broadcast: CharacterVoteCast + RoundStarted

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
                     └── « Je ne sais pas » → tirage de carte par le questionneur
                         POST .../actions/{action}/draw            → mot (à lui seul), CardDrawn
                         POST .../actions/{action}/draw/decision   → CardWordShared (sauf cible),
                                                                     CardDrawFinished, advanceRound()
                         (ou 30 s écoulées → ExpireCardDraw → CardDrawFinished, advanceRound())

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
`DELETE /rooms/{room}/leave` détache le pivot. **`409` si le joueur est dans la partie en cours** : il emporterait son personnage (binome vidé, vote bloqué, fin faussée) — s'il est parti pour de bon, l'hôte l'exclut. Le joueur n'est supprimé en base que s'il n'a jamais joué (ses parties passées pointent encore sur lui). Si la room est vide → supprime la room. Si l'hôte part → transfère `created_by` au prochain joueur.

### Coups concurrents et erreurs de règle
- Question / accusation : le round est verrouillé (`lockForUpdate`) le temps de valider et d'enregistrer — un double envoi ne crée qu'une action. La cible doit être un autre joueur de la partie, non éliminé.
- `RoundService::nextTurn($round, $fromPlayerId)` n'avance que si le tour est encore à `$fromPlayerId` (mise à jour conditionnelle) : une action ne fait jamais avancer le tour deux fois ni recréer un round. `/answer` n'accepte qu'une question, `/confirm` qu'une accusation.
- Fin de partie : `endGame()` clôt le round en cours ; rien n'est diffusé après `GameEnded`.
- Le lancement verrouille le salon : un double clic sur « Lancer » ne crée qu'une partie.
- Les refus de règle lèvent `App\Exceptions\GameRuleException`, rendue en **`409`** avec son message (`bootstrap/app.php`), jamais en 500.
- Les routes `rounds/{round}/…` utilisent `scopeBindings()` : round d'une autre partie → `404`.
- `checkGameOver()` ignore un binome sans joueur (sinon il compterait comme équipe intacte pour toujours).

### Joueur déconnecté : pause, l'hôte décide

La partie est **en pause** quand le joueur qu'elle attend est hors ligne (déterminé par
`ActionService::findBlocker()` sur le round courant). Un autre joueur déconnecté ne bloque rien
tant que ce n'est pas à lui. **Seul l'hôte** (`rooms.created_by`) peut débloquer — si c'est lui
qui est déconnecté, la partie attend son retour.

**Passer son tour** — `POST /games/{game}/skip-turn` (`skipBlockedTurn()`) :

| Ce qu'on attend | `reason` | Effet |
|---|---|---|
| Le joueur dont c'est le tour n'a rien joué | `turn` | Le tour passe, **aucune action** enregistrée |
| La cible d'une accusation n'a pas confirmé | `accusation` | Le **serveur tranche** : nom proposé == vrai personnage (insensible casse / accents / espaces) — sinon un accusé n'aurait qu'à se déconnecter pour s'en tirer |
| La cible d'une question n'a pas répondu | — | **Refusé (`409`)** : une question attend toujours sa réponse. Attendre le retour du joueur, ou l'exclure |
| Le questionneur doit tirer sa carte (« Je ne sais pas ») | `draw` | La carte est **perdue**, le tour avance (de toute façon automatique au bout de 30 s via `ExpireCardDraw`) |

**Exclure** — `POST /games/{game}/players/{player}/exclude` (`excludePlayer()`) :

- L'exclu **et son binôme** sont éliminés (`binome_player.is_eliminated`) ; l'exclu est marqué
  `is_excluded` et marque **0 point** (`ScoreService`). `eliminated_round` sert à leurs rounds
  survécus. Un orphelin exclu n'entraîne personne. Révéler le partenaire est inhérent à la règle.
- Une question / accusation en attente impliquant un joueur éliminé est **supprimée** : si elle
  venait du joueur courant (toujours en jeu), il rejoue ; si le joueur courant est lui-même
  éliminé, le tour avance (`RoundService::nextTurn()` prend le premier joueur actif d'id supérieur,
  le joueur courant pouvant ne plus être dans la liste).
- Puis `checkGameOver()` : l'exclusion peut terminer la partie.
- Broadcast `PlayerExcluded` ; `game_stats.is_excluded` et `recap.exclusions` pour l'affichage.

Dans les deux cas :

- `PresenceService` interroge Reverb (`GET /channels/presence-game.{id}/users`, API Pusher) :
  si le joueur visé est connecté → `409`. Un 404 de Reverb = channel vide = personne en ligne.
  Si Reverb ne répond pas, on laisse l'hôte décider (mieux vaut débloquer que geler). Le délai
  de 30 s avant de pouvoir décider est appliqué côté front.
- `recordAnswer()` / `resolveAccusation()` font une mise à jour **conditionnelle**
  (`whereNull('answer')`, `whereNull('accusation_confirmed')`) : si la cible répond pile au
  moment où l'hôte passe son tour, un seul des deux l'emporte et le round n'avance qu'une fois.

### Photos de profil (`player_avatars`)

- Rattachées au **pseudo**, pas à la ligne `players` : `leave` supprime le joueur, alors que le
  pseudo est l'identité durable (classement, historique). Un joueur retrouve sa photo à chaque
  partie ; même collation que `players.pseudo` (« Killian » = « killian »).
- Stockée en data URI base64 comme les images de personnages. Le **navigateur** recadre et
  réencode en JPEG 256 px (~5–20 ko) : GD n'est pas chargé dans le conteneur, le serveur ne fait
  que vérifier (vrai JPEG via `getimagesizefromstring`, carré, 64–512 px, ≤ 200 ko). Le
  réencodage canvas supprime les métadonnées EXIF.
- **Jamais d'image dans le JSON ni les events** (Reverb limite un message à 10 ko) : les listes de
  joueurs portent `avatar_url` (`PlayerAvatar::urlsFor()`, une requête), servie par
  `GET /api/avatars/{id}?v={updated_at}`. `Room::playersPayload()` centralise la liste du lobby
  (réponses de `RoomController` + events `PlayerJoined` / `PlayerReady` / `PlayerLeft`) ;
  `GET /games/{game}` l'inclut dans `players[]`.
- L'historique ne connaît que des pseudos (`game_stats`, joueurs parfois supprimés) :
  `PlayerAvatar::urlsForPseudos()` alimente `avatar_url` dans `GameHistoryService::detail()`
  (joueurs, scores, journal) et `leaderboard()`.
- Même modèle de confiance que le reste de l'API : `player_id` doit correspondre à `{player}`.

### Spectateurs

Un joueur qui rejoint un salon dont la partie est déjà lancée n'a pas de personnage.
`GET /rooms/{room}` et `POST /rooms/join` renvoient `current_game_id` : le front lui propose de
suivre la partie. Le PresenceChannel `game.{id}` accepte déjà les membres du salon
(`routes/channels.php`), et `Game::canBeWatchedBy()` applique la même règle aux réactions.

Les réactions sont toutes en base (`custom_reactions`) : les 8 emojis Unicode d'origine y ont été repris, l'admin peut en ajouter ou en supprimer. La colonne `emoji` est en collation `utf8mb4_bin` : en `utf8mb4_0900_ai_ci`, tous les emojis sont égaux entre eux.

### Chat commun du hall (`lobby`)
Le chat de la page `/rooms` diffuse sur un **channel public** `lobby`, pas sur une `PresenceChannel` : il doit rester lisible par un visiteur qui n'a ni créé ni rejoint de salon, donc sans `Player` à présenter à `BroadcastAuthController`.

Conséquences sur `ChatController` :
- **Le nom d'auteur est toujours dérivé côté serveur.** Si `player_id` est fourni et valide → `pseudo` du joueur ; sinon → `Anonyme#{anon_id}` (identifiant tiré au sort et stocké dans le `localStorage` du navigateur). Un client ne peut donc pas usurper le pseudo d'un joueur en le postant lui-même.
- `chat_messages.player_id` est `nullOnDelete` et `author_name` est **dénormalisé** : `RoomController@leave` supprime le `Player`, le message doit survivre avec son auteur d'origine. `is_anonymous` se déduit du préfixe de `author_key` (`p:` / `a:`), immuable, et non de `player_id`.
- `author_key` (`p:{id}` ou `a:{anonId}`) sert au front à reconnaître ses propres messages.
- Les codes de partie cités dans un message sont extraits par `ChatMessage::mentionedRoomCodes()` et **confrontés à la table `rooms`** — sans cela n'importe quel mot de 6 caractères (« COUCOU ») passerait pour un code copiable.
- **Durée de vie d'une heure** (`ChatController::RETENTION_MINUTES`) : `index()` filtre sur `created_at`, et la purge en base tourne à chaque lecture *et* chaque écriture — pas de tâche planifiée à prévoir. L'historique renvoyé est limité aux 50 derniers. `LobbyChat.vue` applique la même limite en local (balayage toutes les 30 s) pour qu'un onglet resté ouvert voie les messages disparaître sans rechargement.
- **Anti-spam indexé sur l'auteur, pas sur l'IP** (limiteur nommé `chat`, défini dans `AppServiceProvider`).
  Le proxy du serveur Vite ne transmet pas l'IP du client (pas de `xfwd` dans `vite.config.js`, pas de
  `TrustProxies`) : tous les joueurs du réseau local arrivent avec l'IP du conteneur frontend. Un
  `throttle:20,1` classique leur ferait donc partager un seul compteur.

### Historique public et classement
`GameHistoryService` centralise la lecture de l'historique. Le **même** formatage sert à la vue admin et à l'historique public : c'est le controller qui décide de ce qu'il autorise, pas le service. `AdminGameController` est passé de ~180 à ~60 lignes en le réutilisant.

**Invariant de sécurité** — `HistoryController` n'expose que les parties `finished` :
- `listGames(finishedOnly: true)` pour la liste ;
- `404` explicite sur le détail d'une partie non terminée ;
- `leaderboard()` et `playerBreakdown()` **joignent `games` et filtrent sur le statut**. En pratique `ScoreService` n'écrit dans `game_stats` qu'à la clôture (`ActionService::endGame`), mais ces endpoints étant publics, ils imposent l'invariant au lieu de le supposer.

Publier le détail d'une partie en cours révélerait les personnages à ses propres joueurs : il suffirait d'ouvrir un second onglet pour tricher. La vue admin, elle, voit toujours tout.

**Clé d'identité du classement : le pseudo.** `players.pseudo` est unique et `Player::firstOrCreate(['pseudo' => …])` réutilise donc la même personne d'une partie à l'autre. L'agrégation porte sur `game_stats.player_pseudo`, dénormalisé, qui **survit à la suppression du joueur** (`RoomController@leave` supprime la ligne `players`). Les points d'un même pseudo s'additionnent globalement tout en restant consultables partie par partie via `playerBreakdown()`.

La collation de la base est `utf8mb4_0900_ai_ci` : « Killian », « killian » et « Killián » sont regroupés en une seule ligne de classement — exactement comme la contrainte d'unicité sur `players.pseudo` les considère déjà comme un seul joueur.

Les ex æquo partagent leur rang (1, 2, 2, 4). Supprimer une partie côté admin la retire du classement par cascade DB sur `game_stats`.

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
