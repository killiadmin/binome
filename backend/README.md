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
| Base de données | MySQL / SQLite |
| PHP | ^8.2 |

---

## Architecture du projet

```
app/
├── Enums/
│   ├── GameStatus.php          # waiting | in_progress | finished
│   └── ActionType.php          # question | accusation
│
├── Events/
│   ├── GameStarted.php
│   ├── RoundStarted.php
│   ├── ActionPlayed.php
│   ├── BinomeDiscovered.php
│   └── GameEnded.php
│
├── Http/
│   ├── Controllers/
│   │   ├── RoomController.php
│   │   ├── GameController.php
│   │   └── ActionController.php
│   └── Requests/
│       ├── StoreRoomRequest.php
│       ├── JoinRoomRequest.php
│       ├── PlayQuestionRequest.php
│       └── PlayAccusationRequest.php
│
├── Models/
│   ├── Universe.php
│   ├── Character.php
│   ├── Room.php
│   ├── Player.php
│   ├── Game.php
│   ├── Binome.php
│   ├── Round.php
│   └── Action.php
│
└── Services/
    ├── GameService.php         # orchestration démarrage de partie
    ├── RoundService.php        # gestion des tours
    ├── ActionService.php       # validation questions + accusations
    └── BinomeService.php       # découverte binome + fin de partie

database/
└── migrations/
    ├── create_universes_table.php
    ├── create_characters_table.php
    ├── create_rooms_table.php
    ├── create_players_table.php
    ├── create_room_player_table.php
    ├── create_games_table.php
    ├── create_binomes_table.php
    ├── create_binome_player_table.php
    ├── create_rounds_table.php
    └── create_actions_table.php

routes/
├── api.php                     # endpoints REST
├── channels.php                # autorisation WebSocket PresenceChannel
└── web.php
```

---

## Modèle de données

### Vue d'ensemble des relations

```
Universe
  └── hasMany → Character
  └── hasMany → Binome

Character
  └── belongsTo → Universe
  └── json: forbidden_words[]

Room
  └── belongsToMany → Player  (pivot: is_ready)
  └── hasMany → Game
  └── created_by → Player

Player
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
  └── bool: is_valid           (false si mot interdit)
  └── bool: accusation_correct (nullable, uniquement pour les accusations)
```

### Tables pivots

**`room_player`**
| Colonne | Type | Description |
|---|---|---|
| room_id | FK | |
| player_id | FK | |
| is_ready | boolean | Le joueur est prêt à démarrer |

**`binome_player`**
| Colonne | Type | Description |
|---|---|---|
| binome_id | FK | |
| player_id | FK | |
| character_id | FK | Personnage secret assigné à ce joueur |
| score | integer | Score du joueur dans cette partie |

### Champ important sur `rooms`
```php
$table->foreignId('created_by')->constrained('players');
```
Seul le créateur du salon peut lancer la partie.

---

## Services

### `GameService`

Point d'entrée : `start(Room $room): Game`

1. Valide que le nombre de joueurs est pair et ≥ 4, et que tous sont `is_ready`
2. Crée la `Game` avec le statut `in_progress`
3. Mélange aléatoirement les joueurs → forme des paires → assigne un `Universe` par paire → assigne 2 `Character` distincts du même univers
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

**`playQuestion(Round, Player, string $question): Action`**
1. Vérifie que c'est le tour du joueur (`current_player_id`)
2. Vérifie que le joueur n'a pas déjà joué ce round (action valide existante)
3. Récupère le personnage du joueur et vérifie les mots interdits
4. Si mot interdit → enregistre l'action `is_valid: false`, broadcast, **tour perdu**
5. Sinon → enregistre `is_valid: true`, broadcast
6. Dans tous les cas → appelle `advanceRound()`

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
| `POST` | `/api/rooms` | `RoomController@store` | Créer un salon |
| `POST` | `/api/rooms/join` | `RoomController@join` | Rejoindre avec un code à 6 caractères |
| `GET` | `/api/rooms/{room}` | `RoomController@show` | État du salon + liste joueurs |
| `PATCH` | `/api/rooms/{room}/ready` | `RoomController@ready` | Se mettre prêt |
| `POST` | `/api/rooms/{room}/start` | `GameController@start` | Lancer la partie (créateur uniquement) |
| `GET` | `/api/games/{game}` | `GameController@show` | État de la partie (sans personnages) |
| `GET` | `/api/games/{game}/me` | `GameController@myCharacter` | Mon personnage secret + mots interdits |
| `POST` | `/api/games/{game}/rounds/{round}/question` | `ActionController@question` | Poser une question |
| `POST` | `/api/games/{game}/rounds/{round}/accusation` | `ActionController@accusation` | Faire une accusation |

### Sécurité des données

- `GET /games/{game}` → ne retourne **jamais** les personnages des joueurs
- `GET /games/{game}/me` → retourne le personnage **uniquement au joueur concerné** (via `player_id` en query param)
- Les personnages ne sont révélés publiquement que dans les events `BinomeDiscovered` et `GameEnded`

---

## Events WebSocket

Tous les events broadcastent sur le **PresenceChannel** `game.{gameId}`.

L'autorisation du channel est définie dans `routes/channels.php` : le joueur doit appartenir à la partie (via ses binomes).

### Récapitulatif

| Event | Nom broadcast | Données sensibles |
|---|---|---|
| `GameStarted` | `game.started` | ❌ Pas de personnages |
| `RoundStarted` | `round.started` | Qui joue ce round |
| `ActionPlayed` | `action.played` | Question refusée → contenu masqué / Accusation fausse → personnage masqué |
| `BinomeDiscovered` | `binome.discovered` | ✅ Personnages révélés (binome découvert) |
| `GameEnded` | `game.ended` | ✅ Tout révélé, scores finaux |

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
   POST /rooms                     → créer salon (code généré automatiquement)
   POST /rooms/join                 → rejoindre avec le code
   PATCH /rooms/{room}/ready        → chaque joueur se met prêt
   POST /rooms/{room}/start         → créateur lance la partie

2. DÉMARRAGE (GameService)
   - Mélange aléatoire des joueurs
   - Formation des binomes par paires
   - Assignation d'un univers par binome
   - Assignation d'un personnage distinct par joueur
   - Création du Round n°1
   → broadcast: GameStarted
   → broadcast: RoundStarted

3. TOUR D'UN JOUEUR
   Chaque joueur appelle d'abord :
   GET /games/{game}/me             → récupère son personnage + mots interdits

   Puis joue son action :

   [Option A] Question
   POST .../rounds/{round}/question
   body: { player_id, question }
     ├── mot interdit → is_valid: false, tour perdu
     └── valide       → is_valid: true
   → broadcast: ActionPlayed

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

### Enums natifs Laravel 11
```php
// GameStatus.php
enum GameStatus: string {
    case Waiting    = 'waiting';
    case InProgress = 'in_progress';
    case Finished   = 'finished';
}

// Dans le Model :
protected $casts = ['status' => GameStatus::class];
```

### Bootstrap Laravel 11
Plus de `Kernel.php` — les middlewares se déclarent dans `bootstrap/app.php` :
```php
->withMiddleware(function (Middleware $middleware) {
    $middleware->api(append: [
        \Laravel\Sanctum\Http\Middleware\EnsureFrontendRequestsAreStateful::class,
    ]);
})
```

### Installation de Reverb (WebSocket)
```bash
php artisan install:broadcasting
```

### Contrainte pair sur max_players
Le nombre de joueurs doit toujours être **pair** (binomes). La validation se fait dans `GameService::validateRoom()` avec `count % 2 !== 0`.

### Ordre des joueurs
L'ordre de jeu dans les rounds est toujours **trié par `player.id`** — déterministe et identique à chaque round. Pas de randomisation à chaque round.

### Transaction sur le démarrage
`GameService::start()` est entièrement dans une `DB::transaction()` — si l'assignation des personnages échoue (pas assez de personnages dans un univers), rien n'est persisté.

### Helper clé sur Player
```php
// Récupère le personnage d'un joueur dans une partie donnée
public function getCharacterInGame(Game $game): ?Character
```
Utilisé dans `ActionService` pour récupérer les mots interdits et dans `GameController@myCharacter` pour l'endpoint privé.
