# Migration Rust — 02. Contrat de l'API REST

> **Généré** par `tests/e2e/render-contract.mjs` depuis `tests/e2e/contracts/rust.json`
> (formes observées par le banc E2E, identiques au contrat figé de l'ancien backend Laravel,
> `contracts/laravel-ref.json`). Ne pas éditer à la main.

## Conventions communes

- Préfixe : `/api` côté serveur ; le navigateur appelle `/binome/api` (préfixe ajouté par le proxy).
- Corps JSON ; `multipart/form-data` pour les fichiers de personnages. **Method spoofing** :
  `POST` + champ `_method=PUT` doit être traité comme `PUT` (forme utilisée par le front).
- Identité : `player_id` dans le corps / la query, **non signé** (modèle de confiance actuel, reproduit tel quel).
- Admin : en-tête `X-Characters-Access` = HMAC-SHA256(mot de passe, `binome-characters-access`) en hex
  (propriétaire), ou `X-Profile-Token` d'un administrateur désigné (`admin_role = "admin"`, profil avec mot
  de passe) ; absent ou faux → `403 {message}`.
- Erreurs :
  - `422` validation → `{ message, errors: { champ: [messages] } }` (messages Laravel, en anglais pour les règles génériques, en français pour les messages personnalisés) ;
  - `409` règle du jeu → `{ message }` ;
  - `403` / `404` → `{ message }` ;
  - `429` limite de débit → `{ message }`.
  Laravel en mode debug ajoute `exception / file / line / trace` : **hors contrat**, à ne pas reproduire.
- Dates : deux formats coexistent selon la route — `date:iso8601` = `2026-11-11T10:00:00+00:00`,
  `date:eloquent` = `2026-11-11T10:00:00.000000Z`. Les deux sont à reproduire là où ils apparaissent.
- Booléens : certains champs sortent en `0` / `1` (`int`), ex. `players[].is_ready`. À reproduire.
- Images (`/avatars`, `/character-images`, `/reactions/:id/image`) : binaire, `Cache-Control: public, max-age=31536000, immutable`,
  `X-Content-Type-Options: nosniff`. URLs renvoyées : `/binome/api/...?v=<updated_at timestamp>`.
- Limites de débit (par joueur, jamais par IP) : chat 20/min, réactions 30/min, emojis 5/min, avatars 10/min.

## Couverture

82 routes ; 82 observées par le banc.
Toutes les routes sont observées.

## Routes

### `POST /api/access/characters`

Échange le mot de passe admin contre le jeton `X-Characters-Access` — accès : public

Statuts observés : `200`, `422`

Réponse `200` :

```
$                                                object
$.token                                          string
```

### `GET /api/cosmos`

Cosmos + `is_enabled` + `universes_count` + `playable_universe_count` ; `game_modes` proposés — accès : public

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.cosmos                                         array
$.cosmos[]                                       object
$.cosmos[].id                                    int
$.cosmos[].is_enabled                            bool
$.cosmos[].name                                  string
$.cosmos[].playable_universe_count               int
$.cosmos[].universes_count                       int
$.game_modes                                     array
$.game_modes[]                                   string
```

### `POST /api/cosmos`

Créer un cosmos — accès : admin

Statuts observés : `201`, `403`, `422`

Réponse `201` :

```
$                                                object
$.cosmos                                         object
$.cosmos.id                                      int
$.cosmos.name                                    string
$.message                                        string
```

### `GET /api/universes`

Univers + cosmos + `characters_count` — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.universes                                      array
$.universes[]                                    object
$.universes[].characters_count                   int
$.universes[].cosmos_id                          int | null
$.universes[].cosmos_name                        null | string
$.universes[].id                                 int
$.universes[].name                               string
```

### `POST /api/universes`

Créer un univers — accès : admin

Statuts observés : `201`

Réponse `201` :

```
$                                                object
$.message                                        string
$.universe                                       object
$.universe.cosmos_id                             int
$.universe.cosmos_name                           string
$.universe.id                                    int
$.universe.name                                  string
```

### `PATCH /api/universes/:universe`

Changer le cosmos d'un univers — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.message                                        string
$.universe                                       object
$.universe.cosmos_id                             null
$.universe.cosmos_name                           null
$.universe.id                                    int
$.universe.name                                  string
```

### `POST /api/universes/:universe/characters`

Créer un personnage (multipart, `verif_manual = true`) — accès : admin

Statuts observés : `201`, `422`

Réponse `201` :

```
$                                                object
$.character                                      object
$.character.forbidden_words                      array
$.character.forbidden_words[]                    string
$.character.id                                   int
$.character.image                                null | string
$.character.level_affectation                    int
$.character.name                                 string
$.character.universe_id                          int
$.message                                        string
```

### `GET /api/characters`

Personnages (sans data URI : `image` = URL) — accès : admin

Statuts observés : `200`, `403`

Réponse `200` :

```
$                                                object
$.characters                                     array
$.characters[]                                   object
$.characters[].cosmos_id                         int | null
$.characters[].cosmos_name                       null | string
$.characters[].forbidden_words                   array
$.characters[].forbidden_words[]                 string
$.characters[].hidden                            bool
$.characters[].id                                int
$.characters[].image                             null | string
$.characters[].level_affectation                 int
$.characters[].name                              string
$.characters[].universe_id                       int
$.characters[].universe_name                     string
$.characters[].verif_manual                      bool
```

### `GET /api/characters/pending`

Binômes en attente, groupés par univers + niveau — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.binomes                                        array
$.binomes[]                                      object
$.binomes[].characters                           array
$.binomes[].characters[]                         object
$.binomes[].characters[].forbidden_words         array
$.binomes[].characters[].forbidden_words[]       string
$.binomes[].characters[].id                      int
$.binomes[].characters[].image                   string
$.binomes[].characters[].name                    string
$.binomes[].cosmos_name                          null
$.binomes[].key                                  string
$.binomes[].level_affectation                    int
$.binomes[].universe_id                          int
$.binomes[].universe_name                        string
```

### `POST /api/characters/validation/accept`

Valider (`character_ids[]`), image obligatoire — accès : admin

Statuts observés : `200`, `422`

Réponse `200` :

```
$                                                object
$.message                                        string
$.validated_ids                                  array
$.validated_ids[]                                int
```

### `POST /api/characters/validation/reject`

Refuser → `hidden = true` — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.message                                        string
$.rejected_ids                                   array
$.rejected_ids[]                                 int
```

### `POST /api/characters/validation/restore`

Remettre à valider — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.message                                        string
$.restored_ids                                   array
$.restored_ids[]                                 int
```

### `PUT /api/characters/:character`

Modifier un personnage (JSON) — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.character                                      object
$.character.forbidden_words                      array
$.character.forbidden_words[]                    string
$.character.hidden                               bool
$.character.id                                   int
$.character.image                                string
$.character.level_affectation                    int
$.character.name                                 string
$.character.universe_id                          int
$.character.verif_manual                         bool
$.message                                        string
```

### `POST /api/characters/:character`

Modifier un personnage : **POST multipart + `_method=PUT`** (forme utilisée par le front) — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.character                                      object
$.character.forbidden_words                      array
$.character.forbidden_words[]                    string
$.character.hidden                               bool
$.character.id                                   int
$.character.image                                string
$.character.level_affectation                    int
$.character.name                                 string
$.character.universe_id                          int
$.character.verif_manual                         bool
$.message                                        string
```

### `PATCH /api/characters/:character/forbidden-words`

Remplacer les 3 mots interdits — accès : admin

Statuts observés : `200`, `422`

Réponse `200` :

```
$                                                object
$.character                                      object
$.character.forbidden_words                      array
$.character.forbidden_words[]                    string
$.character.id                                   int
$.message                                        string
```

### `POST /api/characters/:character/image`

Importer / remplacer l'image (multipart) — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.character                                      object
$.character.id                                   int
$.character.image                                string
$.message                                        string
```

### `DELETE /api/characters/:character`

Supprimer (`409` si déjà joué) — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `GET /api/admin/games`

Toutes les parties — accès : admin

Statuts observés : `200`, `403`

Réponse `200` :

```
$                                                object
$.games                                          array
$.games[]                                        object
$.games[].binomes_count                          int
$.games[].binomes_discovered_count               int
$.games[].created_at                             date:iso8601
$.games[].id                                     int
$.games[].players_count                          int
$.games[].room_code                              null | string
$.games[].room_id                                int | null
$.games[].rounds_count                           int
$.games[].status                                 string
$.games[].updated_at                             date:iso8601
$.games[].winners                                array
$.games[].winners[]                              string
```

### `GET /api/admin/games/:game`

Détail complet, personnages révélés — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.binomes                                        array
$.binomes[]                                      object
$.binomes[].cosmos                               string
$.binomes[].discovered_by_player_id              null
$.binomes[].id                                   int
$.binomes[].is_discovered                        bool
$.binomes[].is_orphan                            bool
$.binomes[].players                              array
$.binomes[].players[]                            object
$.binomes[].players[].avatar_url                 null | string
$.binomes[].players[].character                  object
$.binomes[].players[].character.forbidden_words  array
$.binomes[].players[].character.forbidden_words[] string
$.binomes[].players[].character.id               int
$.binomes[].players[].character.name             string
$.binomes[].players[].character.universe         string
$.binomes[].players[].id                         int
$.binomes[].players[].is_eliminated              bool
$.binomes[].players[].pseudo                     string
$.binomes[].players[].score                      int
$.binomes[].universe                             string
$.created_at                                     date:iso8601
$.game_stats                                     array
$.id                                             int
$.room_code                                      string
$.room_id                                        int
$.rounds                                         array
$.rounds[]                                       object
$.rounds[].actions                               array
$.rounds[].current_player_id                     int
$.rounds[].id                                    int
$.rounds[].is_finished                           bool
$.rounds[].number                                int
$.status                                         string
$.updated_at                                     date:iso8601
```

### `DELETE /api/admin/games/:game`

Supprimer une partie (cascade) — accès : admin

Statuts observés : `200`, `403`, `404`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `GET /api/admin/players`

Joueurs + stats + salon + `in_game` + `has_password` / `is_validated` / `admin_role` — accès : admin

Statuts observés : `200`, `403`

Réponse `200` :

```
$                                                object
$.players                                        array
$.players[]                                      object
$.players[].admin_role                           null | string
$.players[].avatar_url                           null | string
$.players[].created_at                           date:iso8601
$.players[].games_count                          int
$.players[].has_password                         bool
$.players[].id                                   int
$.players[].in_game                              bool
$.players[].is_admin                             bool
$.players[].is_validated                         bool
$.players[].pseudo                               string
$.players[].room_code                            null | string
$.players[].score                                int
$.players[].updated_at                           date:iso8601
$.players[].validated_at                         date:iso8601 | null
$.players[].wins                                 int
```

### `PATCH /api/admin/players/:player/admin`

`{ admin }` : désigner / retirer un administrateur (propriétaire seulement, profil avec mot de passe) — accès : admin

Statuts observés : `200`, `403`, `404`, `409`, `422`

Réponse `200` :

```
$                                                object
$.message                                        string
$.player                                         object
$.player.admin_role                              null | string
$.player.id                                      int
$.player.is_validated                            bool
```

### `PATCH /api/admin/players/:player/validation`

`{ validated }` : valider un profil (historique, emojis image) — accès : admin

Statuts observés : `200`, `403`, `404`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
$.player                                         object
$.player.id                                      int
$.player.is_validated                            bool
$.player.validated_at                            date:iso8601 | null
```

### `GET /api/admin/settings`

Modes de jeu, cosmos et binômes complets, avec leur état — accès : admin

Statuts observés : `200`, `403`

Réponse `200` :

```
$                                                object
$.binomes                                        array
$.binomes[]                                      object
$.binomes[].characters                           array
$.binomes[].characters[]                         string
$.binomes[].cosmos_id                            int
$.binomes[].is_enabled                           bool
$.binomes[].level_affectation                    int
$.binomes[].universe_id                          int
$.binomes[].universe_name                        string
$.cosmos                                         array
$.cosmos[]                                       object
$.cosmos[].id                                    int
$.cosmos[].is_enabled                            bool
$.cosmos[].name                                  string
$.cosmos[].playable_universe_count               int
$.cosmos[].universes_count                       int
$.game_modes                                     array
$.game_modes[]                                   object
$.game_modes[].enabled                           bool
$.game_modes[].key                               string
$.game_modes[].label                             string
```

### `PATCH /api/admin/settings/game-modes/:mode`

`{ enabled }` (`409` pour le dernier mode actif) — accès : admin

Statuts observés : `200`, `404`, `409`

Réponse `200` :

```
$                                                object
$.game_modes                                     array
$.game_modes[]                                   string
$.message                                        string
```

### `PATCH /api/admin/cosmos/:cosmos`

`{ is_enabled }` : cosmos proposé aux hôtes — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.cosmos                                         object
$.cosmos.id                                      int
$.cosmos.is_enabled                              bool
$.cosmos.name                                    string
$.message                                        string
```

### `PATCH /api/admin/binomes`

`{ universe_id, level_affectation, is_enabled }` : binôme tirable — accès : admin

Statuts observés : `200`, `422`

Réponse `200` :

```
$                                                object
$.binome                                         object
$.binome.is_enabled                              bool
$.binome.level_affectation                       int
$.binome.universe_id                             int
$.message                                        string
```

### `DELETE /api/admin/players/:player/room`

Retirer le joueur de son salon — accès : admin

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `DELETE /api/admin/players/:player`

Supprimer (`?with_games=1` pour ses parties) — accès : admin

Statuts observés : `200`, `403`, `404`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `GET /api/admin/reactions`

Toutes les réactions, masquées comprises — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.reactions                                      array
$.reactions[]                                    object
$.reactions[].created_at                         date:eloquent
$.reactions[].created_by                         null | string
$.reactions[].emoji                              null | string
$.reactions[].id                                 int
$.reactions[].is_enabled                         bool
$.reactions[].name                               string
$.reactions[].url                                null | string
```

### `POST /api/admin/reactions`

Ajouter `{name, emoji}` ou `{name, image}` — accès : admin

Statuts observés : `201`, `422`

Réponse `201` :

```
$                                                object
$.message                                        string
$.reaction                                       object
$.reaction.created_at                            date:eloquent
$.reaction.created_by                            null
$.reaction.emoji                                 string
$.reaction.id                                    int
$.reaction.is_enabled                            bool
$.reaction.name                                  string
$.reaction.url                                   null
```

### `PATCH /api/admin/reactions/:reaction`

Renommer / masquer — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.message                                        string
$.reaction                                       object
$.reaction.created_at                            date:eloquent
$.reaction.created_by                            null
$.reaction.emoji                                 string
$.reaction.id                                    int
$.reaction.is_enabled                            bool
$.reaction.name                                  string
$.reaction.url                                   null
```

### `DELETE /api/admin/reactions/:reaction`

Supprimer — accès : admin

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `GET /api/ideas`

Boîte à idées : favoris puis plus votées ; `has_voted` / `is_mine` / `remaining_today` selon le jeton — accès : public

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.daily_limit                                    int
$.ideas                                          array
$.ideas[]                                        object
$.ideas[].author_avatar_url                      null
$.ideas[].author_name                            string
$.ideas[].body                                   string
$.ideas[].created_at                             date:iso8601
$.ideas[].has_voted                              bool
$.ideas[].id                                     int
$.ideas[].is_mine                                bool
$.ideas[].is_pinned                              bool
$.ideas[].pinned_at                              date:iso8601 | null
$.ideas[].votes                                  int
$.remaining_today                                int | null
```

### `POST /api/ideas`

Poster `{player_id, body}` (10–500 car., insultes refusées, 5 par 24 h) — accès : player_id · 10/min

Statuts observés : `201`, `401`, `403`, `422`

Réponse `201` :

```
$                                                object
$.idea                                           object
$.idea.author_avatar_url                         null
$.idea.author_name                               string
$.idea.body                                      string
$.idea.created_at                                date:iso8601
$.idea.has_voted                                 bool
$.idea.id                                        int
$.idea.is_mine                                   bool
$.idea.is_pinned                                 bool
$.idea.pinned_at                                 null
$.idea.votes                                     int
$.message                                        string
```

### `DELETE /api/ideas/:idea`

Supprimer sa propre idée — accès : player_id (auteur)

Statuts observés : `200`, `403`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `PUT /api/ideas/:idea/vote`

+1 (idempotent ; 409 sur sa propre idée) — accès : player_id · 30/min

Statuts observés : `200`, `401`, `404`, `409`

Réponse `200` :

```
$                                                object
$.idea                                           object
$.idea.author_avatar_url                         null
$.idea.author_name                               string
$.idea.body                                      string
$.idea.created_at                                date:iso8601
$.idea.has_voted                                 bool
$.idea.id                                        int
$.idea.is_mine                                   bool
$.idea.is_pinned                                 bool
$.idea.pinned_at                                 null
$.idea.votes                                     int
$.message                                        string
```

### `DELETE /api/ideas/:idea/vote`

Retirer son +1 (idempotent) — accès : player_id · 30/min

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.idea                                           object
$.idea.author_avatar_url                         null
$.idea.author_name                               string
$.idea.body                                      string
$.idea.created_at                                date:iso8601
$.idea.has_voted                                 bool
$.idea.id                                        int
$.idea.is_mine                                   bool
$.idea.is_pinned                                 bool
$.idea.pinned_at                                 null
$.idea.votes                                     int
$.message                                        string
```

### `PATCH /api/admin/ideas/:idea`

`{ is_pinned }` : favori en tête de liste — accès : admin

Statuts observés : `200`, `403`, `422`

Réponse `200` :

```
$                                                object
$.idea                                           object
$.idea.author_avatar_url                         null
$.idea.author_name                               string
$.idea.body                                      string
$.idea.created_at                                date:iso8601
$.idea.has_voted                                 bool
$.idea.id                                        int
$.idea.is_mine                                   bool
$.idea.is_pinned                                 bool
$.idea.pinned_at                                 date:iso8601 | null
$.idea.votes                                     int
$.message                                        string
```

### `DELETE /api/admin/ideas/:idea`

Supprimer une idée (votes compris) — accès : admin

Statuts observés : `200`, `404`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `GET /api/reactions`

Réactions actives (Unicode puis images ; images pour un profil validé seulement) — accès : public

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.reactions                                      array
$.reactions[]                                    object
$.reactions[].created_by                         null | string
$.reactions[].emoji                              null | string
$.reactions[].id                                 int
$.reactions[].name                               string
$.reactions[].url                                null | string
```

### `POST /api/reactions`

Un joueur crée un emoji image (JPEG carré 64–256 px ≤ 60 ko, 10/pseudo) — accès : player_id validé · 5/min

Statuts observés : `201`, `403`, `422`

Réponse `201` :

```
$                                                object
$.message                                        string
$.reaction                                       object
$.reaction.created_by                            string
$.reaction.emoji                                 null
$.reaction.id                                    int
$.reaction.name                                  string
$.reaction.url                                   string
```

### `GET /api/reactions/:reaction/image`

Image d'emoji (cache 1 an) — accès : public

Statuts observés : `200`

Réponse `200` (binaire) :

```
$                                                object
$.cache_control                                  string
$.content_type                                   string
```

### `GET /api/history/games`

Parties **terminées** — accès : profil validé

Statuts observés : `200`, `401`, `403`

Réponse `200` :

```
$                                                object
$.games                                          array
$.games[]                                        object
$.games[].binomes_count                          int
$.games[].binomes_discovered_count               int
$.games[].created_at                             date:iso8601
$.games[].id                                     int
$.games[].players_count                          int
$.games[].room_code                              null | string
$.games[].room_id                                int | null
$.games[].rounds_count                           int
$.games[].status                                 string
$.games[].updated_at                             date:iso8601
$.games[].winners                                array
$.games[].winners[]                              string
```

### `GET /api/history/games/:game`

Détail d'une partie terminée (`404` sinon) — accès : profil validé

Statuts observés : `200`, `404`

Réponse `200` :

```
$                                                object
$.binomes                                        array
$.binomes[]                                      object
$.binomes[].cosmos                               string
$.binomes[].discovered_by_player_id              null
$.binomes[].id                                   int
$.binomes[].is_discovered                        bool
$.binomes[].is_orphan                            bool
$.binomes[].players                              array
$.binomes[].players[]                            object
$.binomes[].players[].avatar_url                 null
$.binomes[].players[].character                  object
$.binomes[].players[].character.forbidden_words  array
$.binomes[].players[].character.forbidden_words[] string
$.binomes[].players[].character.id               int
$.binomes[].players[].character.name             string
$.binomes[].players[].character.universe         string
$.binomes[].players[].id                         int
$.binomes[].players[].is_eliminated              bool
$.binomes[].players[].pseudo                     string
$.binomes[].players[].score                      int
$.binomes[].universe                             string
$.created_at                                     date:iso8601
$.game_stats                                     array
$.game_stats[]                                   object
$.game_stats[].avatar_url                        null
$.game_stats[].best_questions                    int
$.game_stats[].character_name                    string
$.game_stats[].eliminations                      int
$.game_stats[].is_eliminated                     bool
$.game_stats[].is_winner                         bool
$.game_stats[].player_pseudo                     string
$.game_stats[].rounds_survived                   int
$.game_stats[].score                             int
$.game_stats[].survived_full_game                bool
$.id                                             int
$.room_code                                      null | string
$.room_id                                        int | null
$.rounds                                         array
$.rounds[]                                       object
$.rounds[].actions                               array
$.rounds[].actions[]                             object
$.rounds[].actions[].accusation_confirmed        bool
$.rounds[].actions[].accusation_correct          bool
$.rounds[].actions[].answer                      null | string
$.rounds[].actions[].answer_corrected_from       null
$.rounds[].actions[].character_name              string
$.rounds[].actions[].content                     string
$.rounds[].actions[].created_at                  date:iso8601
$.rounds[].actions[].id                          int
$.rounds[].actions[].is_valid                    bool
$.rounds[].actions[].player                      object
$.rounds[].actions[].player.avatar_url           null
$.rounds[].actions[].player.id                   int
$.rounds[].actions[].player.pseudo               string
$.rounds[].actions[].refused_reason              null
$.rounds[].actions[].round_id                    int
$.rounds[].actions[].target_player               object
$.rounds[].actions[].target_player.avatar_url    null
$.rounds[].actions[].target_player.id            int
$.rounds[].actions[].target_player.pseudo        string
$.rounds[].actions[].type                        string
$.rounds[].current_player_id                     int
$.rounds[].id                                    int
$.rounds[].is_finished                           bool
$.rounds[].number                                int
$.status                                         string
$.updated_at                                     date:iso8601
```

### `GET /api/history/leaderboard`

Classement cumulé par pseudo (rangs ex æquo) — accès : profil validé

Statuts observés : `200`, `401`, `403`

Réponse `200` :

```
$                                                object
$.leaderboard                                    array
$.leaderboard[]                                  object
$.leaderboard[].avatar_url                       null | string
$.leaderboard[].average_score                    float | int
$.leaderboard[].eliminations                     int
$.leaderboard[].games_played                     int
$.leaderboard[].pseudo                           string
$.leaderboard[].rank                             int
$.leaderboard[].rounds_survived                  int
$.leaderboard[].total_score                      int
$.leaderboard[].wins                             int
```

### `GET /api/history/players`

`?pseudo=` totaux + parties (insensible casse/accents) — accès : profil validé

Statuts observés : `200`, `404`, `422`

Réponse `200` :

```
$                                                object
$.games                                          array
$.games[]                                        object
$.games[].character_name                         string
$.games[].eliminations                           int
$.games[].game_id                                int
$.games[].is_eliminated                          bool
$.games[].is_winner                              bool
$.games[].played_at                              date:iso8601
$.games[].room_code                              null | string
$.games[].rounds_survived                        int
$.games[].score                                  int
$.pseudo                                         string
$.totals                                         object
$.totals.average_score                           float | int
$.totals.eliminations                            int
$.totals.games_played                            int
$.totals.rounds_survived                         int
$.totals.total_score                             int
$.totals.wins                                    int
```

### `GET /api/chat/messages`

50 derniers messages de moins d'1 h — accès : public

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.messages                                       array
$.messages[]                                     object
$.messages[].author_key                          string
$.messages[].author_name                         string
$.messages[].body                                string
$.messages[].codes                               array
$.messages[].codes[]                             string
$.messages[].created_at                          date:iso8601
$.messages[].id                                  int
$.messages[].is_anonymous                        bool
```

### `POST /api/chat/messages`

Poster (`player_id` ou `anon_id`) → `chat.message` — accès : public · 20/min par auteur

Statuts observés : `201`, `422`, `429`

Réponse `201` :

```
$                                                object
$.message                                        object
$.message.author_key                             string
$.message.author_name                            string
$.message.body                                   string
$.message.codes                                  array
$.message.codes[]                                string
$.message.created_at                             date:iso8601
$.message.id                                     int
$.message.is_anonymous                           bool
```

### `GET /api/avatars/:avatar`

Photo JPEG (`?v=`, cache 1 an) — accès : public

Statuts observés : `200`

Réponse `200` (binaire) :

```
$                                                object
$.cache_control                                  string
$.content_type                                   string
```

### `GET /api/character-images/:character`

Image de personnage (type d'origine, cache 1 an) — accès : public

Statuts observés : `200`, `404`

Réponse `200` (binaire) :

```
$                                                object
$.cache_control                                  string
$.content_type                                   string
```

### `POST /api/players/:player/avatar`

Enregistrer sa photo (JPEG carré 64–512 px ≤ 200 ko) — accès : player_id = :player · 10/min

Statuts observés : `200`, `401`, `403`, `422`

Réponse `200` :

```
$                                                object
$.avatar_url                                     string
$.message                                        string
```

### `DELETE /api/players/:player/avatar`

Retirer sa photo — accès : player_id = :player · 10/min

Statuts observés : `200`, `403`

Réponse `200` :

```
$                                                object
$.avatar_url                                     null
$.message                                        string
```

### `POST /api/rooms`

Créer un salon (`409` si le pseudo est connecté ailleurs) — accès : public

Statuts observés : `201`, `403`, `409`, `422`

Réponse `201` :

```
$                                                object
$.message                                        string
$.player                                         object
$.player.avatar_url                              null
$.player.id                                      int
$.player.pseudo                                  string
$.profile                                        object
$.profile.admin_role                             null | string
$.profile.avatar_url                             null
$.profile.has_password                           bool
$.profile.id                                     int
$.profile.is_admin                               bool
$.profile.is_validated                           bool
$.profile.password_prompted                      bool
$.profile.pseudo                                 string
$.profile_token                                  string
$.room                                           object
$.room.code                                      string
$.room.id                                        int
$.room.is_private                                bool
$.room.max_players                               int
```

### `POST /api/rooms/join`

Rejoindre par code (mêmes `409`) — accès : public

Statuts observés : `200`, `403`, `409`, `422`

Réponse `200` :

```
$                                                object
$.message                                        string
$.player                                         object
$.player.avatar_url                              null
$.player.id                                      int
$.player.pseudo                                  string
$.profile                                        object
$.profile.admin_role                             null
$.profile.avatar_url                             null
$.profile.has_password                           bool
$.profile.id                                     int
$.profile.is_admin                               bool
$.profile.is_validated                           bool
$.profile.password_prompted                      bool
$.profile.pseudo                                 string
$.profile_token                                  string
$.room                                           object
$.room.best_question_vote                        bool
$.room.code                                      string
$.room.cosmos_id                                 null
$.room.cosmos_name                               null
$.room.current_game_id                           int | null
$.room.game_mode                                 string
$.room.id                                        int
$.room.players                                   array
$.room.players[]                                 object
$.room.players[].avatar_url                      null | string
$.room.players[].id                              int
$.room.players[].is_ready                        int
$.room.players[].pseudo                          string
$.room.reveal_to_eliminated                      bool
```

### `GET /api/rooms/:room`

État du salon — accès : public

Statuts observés : `200`, `404`

Réponse `200` :

```
$                                                object
$.room                                           object
$.room.best_question_vote                        bool
$.room.code                                      string
$.room.cosmos_id                                 null
$.room.cosmos_name                               null
$.room.created_by                                int
$.room.current_game_id                           null
$.room.game_mode                                 string
$.room.id                                        int
$.room.is_locked                                 bool
$.room.max_players                               int
$.room.players                                   array
$.room.players[]                                 object
$.room.players[].avatar_url                      null | string
$.room.players[].id                              int
$.room.players[].is_ready                        int
$.room.players[].pseudo                          string
$.room.reveal_to_eliminated                      bool
```

### `PATCH /api/rooms/:room/settings`

Mode de jeu + options éliminés — accès : hôte

Statuts observés : `200`, `403`, `422`

Réponse `200` :

```
$                                                object
$.best_question_vote                             bool
$.cosmos_id                                      int | null
$.cosmos_name                                    null | string
$.game_mode                                      string
$.message                                        string
$.reveal_to_eliminated                           bool
```

### `PATCH /api/rooms/:room/ready`

Bascule prêt / pas prêt — accès : membre

Statuts observés : `200`, `403`, `422`

Réponse `200` :

```
$                                                object
$.all_ready                                      bool
$.can_start                                      bool
$.is_ready                                       bool
$.message                                        string
$.players                                        array
$.players[]                                      object
$.players[].avatar_url                           null | string
$.players[].id                                   int
$.players[].is_ready                             int
$.players[].pseudo                               string
```

### `POST /api/rooms/:room/start`

Lancer la partie — accès : hôte

Statuts observés : `201`, `403`, `409`, `422`

Réponse `201` :

```
$                                                object
$.game_id                                        int
$.message                                        string
$.status                                         string
```

### `DELETE /api/rooms/:room/leave`

Quitter (`409` si joueur de la partie en cours) — accès : membre

Statuts observés : `200`, `422`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `GET /api/games/:game`

État public (jamais de personnage ni de binôme non découvert) — accès : public

Statuts observés : `200`, `404`

Réponse `200` :

```
$                                                object
$.accepted_player_ids                            array
$.actions                                        array
$.actions[]                                      object
$.actions[].accusation_confirmed                 bool
$.actions[].accusation_correct                   bool
$.actions[].action_id                            int
$.actions[].answer                               null | string
$.actions[].answer_corrected_from                null
$.actions[].card_draw                            null | object
$.actions[].card_draw.card_count                 int
$.actions[].card_draw.deadline                   date:iso8601
$.actions[].card_draw.status                     string
$.actions[].character_name                       string
$.actions[].created_at                           date:iso8601
$.actions[].id                                   int
$.actions[].is_valid                             bool
$.actions[].player                               object
$.actions[].player.id                            int
$.actions[].player.pseudo                        string
$.actions[].question                             string
$.actions[].refused_reason                       null
$.actions[].round_id                             int
$.actions[].round_number                         int
$.actions[].target_player                        object
$.actions[].target_player.id                     int
$.actions[].target_player.pseudo                 string
$.actions[].type                                 string
$.binomes                                        array
$.change_player_ids                              array
$.characters_confirmed                           bool
$.current_round                                  null | object
$.current_round.current_player_id                int
$.current_round.id                               int
$.current_round.number                           int
$.draw_number                                    int
$.game_id                                        int
$.has_orphan                                     bool
$.host_id                                        int
$.max_character_changes                          int
$.players                                        array
$.players[]                                      object
$.players[].avatar_url                           null | string
$.players[].character_changes_left               int
$.players[].id                                   int
$.players[].is_eliminated                        bool
$.players[].is_excluded                          bool
$.players[].pseudo                               string
$.question_votes                                 array
$.question_votes[]                               object
$.question_votes[].action_id                     int
$.question_votes[].round_id                      int
$.question_votes[].voter_id                      int
$.settings                                       object
$.settings.best_question_vote                    bool
$.settings.reveal_to_eliminated                  bool
$.status                                         string
```

### `GET /api/games/:game/me`

Mon personnage + mots interdits — accès : player_id

Statuts observés : `200`, `401`, `403`, `404`

Réponse `200` :

```
$                                                object
$.character                                      object
$.character.cosmos                               string
$.character.forbidden_words                      array
$.character.forbidden_words[]                    string
$.character.id                                   int
$.character.image                                string
$.character.name                                 string
$.character.universe                             string
```

### `GET /api/games/:game/recap`

Récap (`409` tant que non terminée) — accès : public

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.accusations_count                              int
$.awards                                         array
$.awards[]                                       object
$.awards[].detail                                string
$.awards[].emoji                                 string
$.awards[].key                                   string
$.awards[].pseudos                               array
$.awards[].pseudos[]                             string
$.awards[].title                                 string
$.best_questions                                 array
$.best_questions[]                               object
$.best_questions[].author                        string
$.best_questions[].question                      string
$.best_questions[].round                         int
$.best_questions[].votes                         int
$.binomes                                        array
$.binomes[]                                      object
$.binomes[].cosmos                               string
$.binomes[].id                                   int
$.binomes[].is_orphan                            bool
$.binomes[].players                              array
$.binomes[].players[]                            object
$.binomes[].players[].avatar_url                 null
$.binomes[].players[].character                  string
$.binomes[].players[].character_image            string
$.binomes[].players[].is_eliminated              bool
$.binomes[].players[].pseudo                     string
$.binomes[].universe                             string
$.duration_seconds                               int
$.eliminations                                   array
$.eliminations[]                                 object
$.eliminations[].by                              string
$.eliminations[].character                       string
$.eliminations[].round                           int
$.eliminations[].target                          string
$.exclusions                                     array
$.exclusions[]                                   object
$.exclusions[].partner                           null | string
$.exclusions[].pseudo                            string
$.exclusions[].round                             int
$.game_id                                        int
$.questions_count                                int
$.rounds_count                                   int
$.stats                                          array
$.stats[]                                        object
$.stats[].best_questions                         int
$.stats[].character_name                         string
$.stats[].eliminations                           int
$.stats[].is_eliminated                          bool
$.stats[].is_excluded                            bool
$.stats[].is_winner                              bool
$.stats[].player_pseudo                          string
$.stats[].rounds_survived                        int
$.stats[].score                                  int
$.stats[].survived_full_game                     bool
$.winners                                        array
$.winners[]                                      string
```

### `GET /api/games/:game/reveal`

Vue omnisciente (éliminés, si l'option est active) — accès : player_id éliminé

Statuts observés : `200`, `403`

Réponse `200` :

```
$                                                object
$.binomes                                        array
$.binomes[]                                      object
$.binomes[].cosmos                               string
$.binomes[].id                                   int
$.binomes[].is_orphan                            bool
$.binomes[].players                              array
$.binomes[].players[]                            object
$.binomes[].players[].character                  string
$.binomes[].players[].forbidden_words            array
$.binomes[].players[].forbidden_words[]          string
$.binomes[].players[].id                         int
$.binomes[].players[].image                      string
$.binomes[].players[].is_eliminated              bool
$.binomes[].players[].pseudo                     string
$.binomes[].universe                             string
```

### `POST /api/games/:game/question-votes`

Vote meilleure question (éliminés) — accès : player_id éliminé

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `POST /api/games/:game/character-vote`

Accepter / changer son personnage — accès : player_id

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `POST /api/games/:game/reactions`

Réaction rapide → `reaction.sent` — accès : joueur ou spectateur · 30/min

Statuts observés : `200`, `403`, `404`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `POST /api/games/:game/skip-turn`

Passer le tour d'un joueur déconnecté — accès : hôte

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
$.reason                                         string
```

### `POST /api/games/:game/players/:player/exclude`

Exclure un joueur déconnecté (+ son binôme) — accès : hôte

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `POST /api/games/:game/actions/:action/correct-answer`

Corriger une réponse oui ⇄ non — accès : hôte

Statuts observés : `200`, `409`, `422`

Réponse `200` :

```
$                                                object
$.action_id                                      int
$.answer                                         string
$.answer_corrected_from                          null | string
$.message                                        string
```

### `POST /api/games/:game/rounds/:round/question`

Poser une question — accès : joueur courant

Statuts observés : `200`, `404`, `409`, `422`

Réponse `200` :

```
$                                                object
$.action                                         object
$.action.accusation_correct                      null
$.action.content                                 string
$.action.created_at                              date:eloquent
$.action.id                                      int
$.action.is_valid                                bool
$.action.player                                  object
$.action.player.created_at                       date:eloquent
$.action.player.id                               int
$.action.player.pseudo                           string
$.action.player.updated_at                       date:eloquent
$.action.player_id                               int
$.action.round                                   object
$.action.round.created_at                        date:eloquent
$.action.round.current_player_id                 int
$.action.round.game_id                           int
$.action.round.id                                int
$.action.round.is_finished                       int
$.action.round.number                            int
$.action.round.updated_at                        date:eloquent
$.action.round_id                                int
$.action.target_player                           object
$.action.target_player.created_at                date:eloquent
$.action.target_player.id                        int
$.action.target_player.pseudo                    string
$.action.target_player.updated_at                date:eloquent
$.action.target_player_id                        int
$.action.type                                    string
$.action.updated_at                              date:eloquent
$.is_valid                                       bool
$.message                                        string
```

### `POST /api/games/:game/rounds/:round/accusation`

Accuser (`character_name`) — accès : joueur courant

Statuts observés : `200`, `422`

Réponse `200` :

```
$                                                object
$.action                                         object
$.action.accusation_correct                      null
$.action.character_name                          string
$.action.content                                 string
$.action.created_at                              date:eloquent
$.action.id                                      int
$.action.is_valid                                bool
$.action.player                                  object
$.action.player.created_at                       date:eloquent
$.action.player.id                               int
$.action.player.pseudo                           string
$.action.player.updated_at                       date:eloquent
$.action.player_id                               int
$.action.round                                   object
$.action.round.created_at                        date:eloquent
$.action.round.current_player_id                 int
$.action.round.game_id                           int
$.action.round.id                                int
$.action.round.is_finished                       int
$.action.round.number                            int
$.action.round.updated_at                        date:eloquent
$.action.round_id                                int
$.action.target_player                           object
$.action.target_player.created_at                date:eloquent
$.action.target_player.id                        int
$.action.target_player.pseudo                    string
$.action.target_player.updated_at                date:eloquent
$.action.target_player_id                        int
$.action.type                                    string
$.action.updated_at                              date:eloquent
$.message                                        string
```

### `POST /api/games/:game/rounds/:round/actions/:action/answer`

Répondre `yes` / `no` / `dont_know` — accès : cible

Statuts observés : `200`, `409`, `422`

Réponse `200` :

```
$                                                object
$.action                                         object
$.action.accusation_confirmed                    null
$.action.accusation_correct                      null
$.action.answer                                  string
$.action.answer_corrected_at                     null
$.action.answer_corrected_from                   null
$.action.card_count                              int | null
$.action.card_draw_deadline                      date:eloquent | null
$.action.card_draw_status                        null | string
$.action.character_name                          null
$.action.content                                 string
$.action.created_at                              date:eloquent
$.action.id                                      int
$.action.is_valid                                bool
$.action.player                                  object
$.action.player.created_at                       date:eloquent
$.action.player.id                               int
$.action.player.pseudo                           string
$.action.player.updated_at                       date:eloquent
$.action.player_id                               int
$.action.round                                   object
$.action.round.created_at                        date:eloquent
$.action.round.current_player_id                 int
$.action.round.game_id                           int
$.action.round.id                                int
$.action.round.is_finished                       int
$.action.round.number                            int
$.action.round.updated_at                        date:eloquent
$.action.round_id                                int
$.action.target_player                           object
$.action.target_player.created_at                date:eloquent
$.action.target_player.id                        int
$.action.target_player.pseudo                    string
$.action.target_player.updated_at                date:eloquent
$.action.target_player_id                        int
$.action.type                                    string
$.action.updated_at                              date:eloquent
$.message                                        string
```

### `POST /api/games/:game/rounds/:round/actions/:action/confirm`

Confirmer / nier une accusation — accès : accusé

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.action                                         object
$.action.accusation_confirmed                    bool
$.action.accusation_correct                      bool
$.action.answer                                  null
$.action.answer_corrected_at                     null
$.action.answer_corrected_from                   null
$.action.card_count                              null
$.action.card_draw_deadline                      null
$.action.card_draw_status                        null
$.action.character_name                          string
$.action.content                                 string
$.action.created_at                              date:eloquent
$.action.id                                      int
$.action.is_valid                                bool
$.action.player                                  object
$.action.player.created_at                       date:eloquent
$.action.player.id                               int
$.action.player.pseudo                           string
$.action.player.updated_at                       date:eloquent
$.action.player_id                               int
$.action.round                                   object
$.action.round.created_at                        date:eloquent
$.action.round.current_player_id                 int
$.action.round.game_id                           int
$.action.round.id                                int
$.action.round.is_finished                       int
$.action.round.number                            int
$.action.round.updated_at                        date:eloquent
$.action.round_id                                int
$.action.target_player                           object
$.action.target_player.created_at                date:eloquent
$.action.target_player.id                        int
$.action.target_player.pseudo                    string
$.action.target_player.updated_at                date:eloquent
$.action.target_player_id                        int
$.action.type                                    string
$.action.updated_at                              date:eloquent
$.message                                        string
```

### `POST /api/games/:game/rounds/:round/actions/:action/draw`

Tirer une carte → `{word, reveal_seconds}` (seul endroit où le mot circule en HTTP) — accès : questionneur

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.reveal_seconds                                 int
$.word                                           string
```

### `POST /api/games/:game/rounds/:round/actions/:action/draw/decision`

Montrer (`share: true`) ou garder — accès : questionneur

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
```

### `DELETE /api/profile/password`

— — accès : —

Statuts observés : `200`, `409`

Réponse `200` :

```
$                                                object
$.message                                        string
$.player                                         object
$.player.admin_role                              null
$.player.avatar_url                              null | string
$.player.has_password                            bool
$.player.id                                      int
$.player.is_admin                                bool
$.player.is_validated                            bool
$.player.password_prompted                       bool
$.player.pseudo                                  string
$.token                                          string
```

### `GET /api/profile`

— — accès : —

Statuts observés : `200`, `401`

Réponse `200` :

```
$                                                object
$.current                                        null | object
$.current.game_id                                null
$.current.host_id                                int
$.current.room_code                              string
$.current.room_game_id                           null
$.current.room_id                                int
$.games                                          array
$.player                                         object
$.player.admin_role                              null | string
$.player.avatar_url                              null | string
$.player.has_password                            bool
$.player.id                                      int
$.player.is_admin                                bool
$.player.is_validated                            bool
$.player.password_prompted                       bool
$.player.pseudo                                  string
$.totals                                         object
$.totals.games_played                            int
$.totals.total_score                             int
$.totals.wins                                    int
```

### `PATCH /api/profile`

— — accès : —

Statuts observés : `200`, `403`, `409`, `422`

Réponse `200` :

```
$                                                object
$.message                                        string
$.player                                         object
$.player.admin_role                              null
$.player.avatar_url                              string
$.player.has_password                            bool
$.player.id                                      int
$.player.is_admin                                bool
$.player.is_validated                            bool
$.player.password_prompted                       bool
$.player.pseudo                                  string
$.token                                          string
```

### `POST /api/profile/login`

— — accès : —

Statuts observés : `200`, `409`, `422`

Réponse `200` :

```
$                                                object
$.admin_token                                    string
$.current                                        null | object
$.current.game_id                                null
$.current.host_id                                int
$.current.room_code                              string
$.current.room_game_id                           null
$.current.room_id                                int
$.message                                        string
$.player                                         object
$.player.admin_role                              null | string
$.player.avatar_url                              null | string
$.player.has_password                            bool
$.player.id                                      int
$.player.is_admin                                bool
$.player.is_validated                            bool
$.player.password_prompted                       bool
$.player.pseudo                                  string
$.token                                          string
```

### `POST /api/profile/password-prompt`

— — accès : —

Statuts observés : `200`

Réponse `200` :

```
$                                                object
$.player                                         object
$.player.admin_role                              null
$.player.avatar_url                              null
$.player.has_password                            bool
$.player.id                                      int
$.player.is_admin                                bool
$.player.is_validated                            bool
$.player.password_prompted                       bool
$.player.pseudo                                  string
```

### `PUT /api/profile/password`

— — accès : —

Statuts observés : `200`, `403`, `422`

Réponse `200` :

```
$                                                object
$.message                                        string
$.player                                         object
$.player.admin_role                              null
$.player.avatar_url                              null
$.player.has_password                            bool
$.player.id                                      int
$.player.is_admin                                bool
$.player.is_validated                            bool
$.player.password_prompted                       bool
$.player.pseudo                                  string
$.token                                          string
```
