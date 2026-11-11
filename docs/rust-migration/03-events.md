# Migration Rust — 03. Événements temps réel

> Formes **générées** par `tests/e2e/render-contract.mjs` depuis `tests/e2e/contracts/rust.json`.

## Canaux

| Canal | Type | Qui peut s'abonner (règles de `routes/channels.php`) | Membres de présence |
|---|---|---|---|
| `room.{id}` | presence | joueur membre du salon | `{ id, pseudo }` |
| `game.{id}` | presence | joueur de la partie **ou** membre du salon (spectateur) | `{ id, pseudo }` |
| `player.{id}` | privé | seulement `player_id == id` | — |
| `lobby` | public | tout le monde, même sans joueur | — |

Présence : un joueur connecté sur plusieurs onglets compte **une fois** ; `joining` à la
première connexion, `leaving` à la dernière déconnexion. Le front n'interprète un `leaving`
qu'après 3 s (rechargement de page).

Garanties observées et vérifiées par le banc :
- `card.word.shared` n'est **jamais** reçu par la cible ni renvoyé au questionneur ; le spectateur le reçoit.
- `card.drawn` ne contient jamais le mot.
- `game.started` ne contient aucun personnage.
- Aucun `round.started` n'est diffusé après `game.ended`.

Le banc n'observe `game.started` que sur `room.{id}` : le front ne s'abonne à `game.{id}`
qu'après l'avoir reçu. L'event `binome.discovered`, écouté par le front, n'est jamais
émis (code mort, non porté).

## Récapitulatif

| Event | Canal | Déclencheur | Observé |
|---|---|---|---|
| `player.joined` | `room.{id}` | Un joueur rejoint le salon | ✓ |
| `player.ready` | `room.{id}` | Bascule prêt | ✓ |
| `player.left` | `room.{id}` | Départ (+ `new_host_id` si l'hôte part) | ✓ |
| `room.settings.updated` | `room.{id}` | L'hôte change les réglages | ✓ |
| `game.started` | `room.{id}` + `game.{id}` | Lancement | ✓ |
| `player.avatar.updated` | `room.{id}` de chaque salon du joueur + `game.{id}` en cours | Photo modifiée / retirée | ✓ |
| `character.vote.cast` | `game.{id}` | Un vote de personnage ; aussi au dernier vote unanime | ✓ |
| `characters.redrawn` | `game.{id}` | Tous ont voté, ≥ 1 changement : nouveau tirage | ✓ |
| `round.started` | `game.{id}` | Nouveau round (`is_new_round: true`) ou joueur suivant (`false`) | ✓ |
| `action.played` | `game.{id}` | Question ou accusation jouée | ✓ |
| `answer.given` | `game.{id}` | Réponse (+ `card_draw` si « je ne sais pas » ouvre un tirage) | ✓ |
| `answer.corrected` | `game.{id}` | Correction de l'hôte | ✓ |
| `card.drawn` | `game.{id}` | Carte retournée (index seulement) | ✓ |
| `card.word.shared` | `player.{id}` de chaque destinataire **sauf la cible et le questionneur** | Mot montré | ✓ |
| `card.draw.finished` | `game.{id}` | Tirage clos (montré, gardé ou expiré — indiscernables) | ✓ |
| `accusation.confirmed` | `game.{id}` | Accusation tranchée (par l'accusé ou par le serveur) | ✓ |
| `player.eliminated` | `game.{id}` | Accusation juste | ✓ |
| `player.excluded` | `game.{id}` | Exclusion par l'hôte | ✓ |
| `turn.skipped` | `game.{id}` | Tour passé par l'hôte (`reason`: `turn` | `accusation` | `draw`) | ✓ |
| `question.voted` | `game.{id}` | Vote meilleure question (tous les votes du round) | ✓ |
| `reaction.sent` | `game.{id}` | Réaction rapide (rien n'est stocké) | ✓ |
| `game.ended` | `game.{id}` | Fin de partie : gagnants, stats, révélation complète | ✓ |
| `chat.message` | `lobby` (public) | Message du chat du hall | ✓ |

## Payloads

### `player.joined` (@room)

```
$                                                object
$.player                                         object
$.player.id                                      int
$.player.is_ready                                bool
$.player.pseudo                                  string
$.players                                        array
$.players[]                                      object
$.players[].avatar_url                           null | string
$.players[].id                                   int
$.players[].is_ready                             int
$.players[].pseudo                               string
```

### `player.ready` (@room)

```
$                                                object
$.can_start                                      bool
$.player_id                                      int
$.players                                        array
$.players[]                                      object
$.players[].avatar_url                           null | string
$.players[].id                                   int
$.players[].is_ready                             int
$.players[].pseudo                               string
```

### `player.left` (@room)

```
$                                                object
$.new_host_id                                    int | null
$.players                                        array
$.players[]                                      object
$.players[].avatar_url                           null
$.players[].id                                   int
$.players[].is_ready                             int
$.players[].pseudo                               string
```

### `room.settings.updated` (@room)

```
$                                                object
$.best_question_vote                             bool
$.cosmos_id                                      int | null
$.cosmos_name                                    null | string
$.game_mode                                      string
$.reveal_to_eliminated                           bool
```

### `game.started` (@room)

```
$                                                object
$.draw_number                                    int
$.game_id                                        int
$.has_orphan                                     bool
$.players                                        array
$.players[]                                      object
$.players[].id                                   int
$.players[].pseudo                               string
$.status                                         string
```

### `player.avatar.updated` (@game)

```
$                                                object
$.avatar_url                                     null
$.player_id                                      int
```

### `player.avatar.updated` (@room)

```
$                                                object
$.avatar_url                                     null | string
$.player_id                                      int
```

### `character.vote.cast` (@game)

```
$                                                object
$.accepted_player_ids                            array
$.accepted_player_ids[]                          int
$.change_player_ids                              array
$.change_player_ids[]                            int
$.draw_number                                    int
$.game_id                                        int
```

### `characters.redrawn` (@game)

```
$                                                object
$.change_count                                   int
$.draw_number                                    int
$.game_id                                        int
```

### `round.started` (@game)

```
$                                                object
$.current_player                                 object
$.current_player.id                              int
$.current_player.pseudo                          string
$.is_new_round                                   bool
$.number                                         int
$.round_id                                       int
```

### `action.played` (@game)

```
$                                                object
$.accusation_confirmed                           null
$.accusation_correct                             null
$.action_id                                      int
$.character_name                                 string
$.is_valid                                       bool
$.player                                         object
$.player.id                                      int
$.player.pseudo                                  string
$.question                                       string
$.refused_reason                                 null
$.round_id                                       int
$.round_number                                   int
$.target_player                                  object
$.target_player.id                               int
$.target_player.pseudo                           string
$.type                                           string
```

### `answer.given` (@game)

```
$                                                object
$.action_id                                      int
$.answer                                         string
$.answerer                                       object
$.answerer.id                                    int
$.answerer.pseudo                                string
$.asker                                          object
$.asker.id                                       int
$.asker.pseudo                                   string
$.card_draw                                      null | object
$.card_draw.card_count                           int
$.card_draw.deadline                             date:iso8601
$.card_draw.status                               string
$.round_id                                       int
```

### `answer.corrected` (@game)

```
$                                                object
$.action_id                                      int
$.answer                                         string
$.answer_corrected_from                          null | string
$.answerer                                       object
$.answerer.id                                    int
$.answerer.pseudo                                string
$.host                                           object
$.host.id                                        int
$.host.pseudo                                    string
$.previous_answer                                string
$.question                                       string
$.round_id                                       int
```

### `card.drawn` (@game)

```
$                                                object
$.action_id                                      int
$.asker                                          object
$.asker.id                                       int
$.asker.pseudo                                   string
$.card                                           int
```

### `card.word.shared` (@player)

```
$                                                object
$.action_id                                      int
$.asker                                          object
$.asker.id                                       int
$.asker.pseudo                                   string
$.reveal_seconds                                 int
$.target                                         object
$.target.id                                      int
$.target.pseudo                                  string
$.word                                           string
```

### `card.draw.finished` (@game)

```
$                                                object
$.action_id                                      int
$.asker                                          object
$.asker.id                                       int
$.asker.pseudo                                   string
```

### `accusation.confirmed` (@game)

```
$                                                object
$.accusation_confirmed                           bool
$.accusation_correct                             bool
$.accused                                        object
$.accused.id                                     int
$.accused.pseudo                                 string
$.accuser                                        object
$.accuser.id                                     int
$.accuser.pseudo                                 string
$.action_id                                      int
$.character_name                                 string
$.round_id                                       int
```

### `player.eliminated` (@game)

```
$                                                object
$.eliminated_by                                  object
$.eliminated_by.id                               int
$.eliminated_by.pseudo                           string
$.eliminated_player                              object
$.eliminated_player.id                           int
$.eliminated_player.pseudo                       string
```

### `player.excluded` (@game)

```
$                                                object
$.cancelled_action_id                            null
$.excluded_by                                    object
$.excluded_by.id                                 int
$.excluded_by.pseudo                             string
$.partner                                        null | object
$.partner.id                                     int
$.partner.pseudo                                 string
$.player                                         object
$.player.id                                      int
$.player.pseudo                                  string
```

### `turn.skipped` (@game)

```
$                                                object
$.action_id                                      int | null
$.player                                         object
$.player.id                                      int
$.player.pseudo                                  string
$.reason                                         string
$.skipped_by                                     object
$.skipped_by.id                                  int
$.skipped_by.pseudo                              string
```

### `question.voted` (@game)

```
$                                                object
$.round_id                                       int
$.votes                                          array
$.votes[]                                        object
$.votes[].action_id                              int
$.votes[].voter_id                               int
```

### `reaction.sent` (@game)

```
$                                                object
$.player                                         object
$.player.id                                      int
$.player.pseudo                                  string
$.reaction                                       object
$.reaction.created_by                            null
$.reaction.emoji                                 string
$.reaction.id                                    int
$.reaction.name                                  string
$.reaction.url                                   null
```

### `game.ended` (@game)

```
$                                                object
$.all_binomes                                    array
$.all_binomes[]                                  object
$.all_binomes[].id                               int
$.all_binomes[].is_orphan                        bool
$.all_binomes[].players                          array
$.all_binomes[].players[]                        object
$.all_binomes[].players[].character              string
$.all_binomes[].players[].pseudo                 string
$.all_binomes[].universe                         string
$.game_id                                        int
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
$.winners[]                                      object
$.winners[].id                                   int
$.winners[].pseudo                               string
```

### `chat.message` (@lobby)

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
