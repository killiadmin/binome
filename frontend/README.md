# Frontend — Binome

## Table des matières

- [Stack technique](#stack-technique)
- [Structure du projet](#structure-du-projet)
- [Variables d'environnement](#variables-denvironnement)
- [Lancer le projet](#lancer-le-projet)
- [Architecture](#architecture)
- [WebSocket — useReverb](#websocket--usereverb)
- [Session persistée](#session-persistée)
- [Pages](#pages)
- [Services API](#services-api)
- [Flux complet côté front](#flux-complet-côté-front)
- [Points d'attention importants](#points-dattention-importants)

---

## Stack technique

| Composant | Technologie |
|---|---|
| Framework | Vue 3 (Composition API + `<script setup>`) |
| Router | Vue Router |
| UI | Bootstrap Vue Next |
| HTTP | Axios |
| WebSocket | Laravel Echo + Pusher JS (broadcaster: reverb) |
| Persistance session | localStorage |

---

## Structure du projet

```
src/
├── assets/
│
├── components/
│   └── cropper/
│       ├── ImageDropzone.vue      # zone glisser-déposer / clic → File
│       └── ImageCropperModal.vue  # recadrage carré imposé → 512×512 JPEG
│
├── composables/
│   └── useEcho.js              # (non utilisé — remplacé par useReverb)
│
├── pages/
│   ├── Characters/
│   │   ├── CharacterPage.vue      # Créer un personnage (univers/cosmos existant ou nouveau)
│   │   └── CharacterListPage.vue  # Liste triée cosmos → univers → binômes, filtres, édition/suppression, validation des propositions
│   ├── Game/
│   │   └── RoundPage.vue       # Page de jeu (en cours de développement)
│   ├── Home/
│   │   └── HomePage.vue
│   ├── Rooms/
│   │   └── RoomPage.vue        # Lobby — créer/rejoindre/gérer un salon
│   └── Rules/
│       └── RulePage.vue
│
├── services/
│   ├── api.js                  # Instance Axios configurée
│   ├── charactersAccess.js     # Déverrouillage par mot de passe des pages personnages (token en session)
│   ├── characterService.js     # Appels API personnages/univers/cosmos + validation
│   ├── gameService.js          # Appels API partie
│   └── roomService.js          # Appels API salon
│
├── sockets/
│   └── useReverb.js            # Singleton Echo + joinRoom/joinGame
│
├── stores/
│   └── gameStore.js            # Store Pinia (en cours)
│
├── App.vue
├── main.js
├── router.js
└── style.css
```

---

## Variables d'environnement

Fichier `.env` à créer à la racine du projet frontend :

```env
VITE_REVERB_APP_KEY=reverb_key
```

> `VITE_REVERB_APP_KEY` doit correspondre exactement à `REVERB_APP_KEY` du `.env` Laravel backend.

Le navigateur ne parle qu'à l'origine qui a servi la page (`http://<IP LAN>:5173`). `vite.config.js` proxifie `/api` et `/broadcasting` vers le conteneur `backend`, et le WebSocket `/app` vers le conteneur `reverb`, par nom de service Docker. **L'IP LAN n'apparaît donc dans aucune config front** : changer de Wi-Fi ne demande aucune regénération.

Hors Docker uniquement (`npm run dev`), les noms de service ne résolvent pas — surcharger alors :

```env
VITE_PROXY_BACKEND=http://127.0.0.1:8001
VITE_PROXY_REVERB=http://127.0.0.1:8080
```

---

## Lancer le projet

### Avec Docker (recommandé)

Voir le README à la racine du repo — `./start.sh` lance toute la stack (dont ce service) via Docker Compose et affiche l'URL de partage. Le front est servi sur `http://<IP LAN>:5173`, accessible depuis n'importe quel appareil connecté au même Wi-Fi.

### Sans Docker (legacy)

```bash
npm install
npm run dev -- --host
```

Le flag `--host` expose Vite sur le réseau local (nécessaire si tu accèdes depuis un autre appareil que localhost). Renseigne alors `VITE_PROXY_BACKEND` / `VITE_PROXY_REVERB` dans `.env` (voir [Variables d'environnement](#variables-denvironnement)).

---

## Architecture

### `api.js` — Instance Axios

```js
export const api = axios.create({
    baseURL: '/api', // même origine — proxifié vers le backend par le dev server
    headers: {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'X-Requested-With': 'XMLHttpRequest',
    },
    // withCredentials retiré — Sanctum non utilisé pour l'instant
})
```

Inclut un intercepteur de réponse pour logger les erreurs 401, 403, 422, 500,
et un intercepteur de **requête** qui joint l'en-tête `X-Characters-Access`
(token d'accès à la gestion des personnages) quand il est présent en session.

---

### `charactersAccess.js` — Accès cloisonné aux personnages

Les pages **Liste des personnages** (`/characters/list`) et **Création de
personnages** (`/characters`) sont masquées par défaut et protégées par un mot
de passe (défini côté backend dans `CHARACTERS_ACCESS_PASSWORD`).

- `charactersAccess.unlock(password)` → `POST /api/access/characters`, stocke le
  `token` renvoyé dans `localStorage` sous la clé `charactersAccess`.
- `charactersAccess.isUnlocked` (computed réactif) → pilote l'affichage des deux
  onglets dans `Navbar.vue` ; le cadenas ouvre la modale de saisie du mot de passe.
- `charactersAccess.lock()` → oublie le token (reverrouille).
- `router.js` : une garde `beforeEach` renvoie vers l'accueil toute navigation
  vers une route `meta.requiresCharactersAccess` sans token (accès direct par URL).
- Le token étant persisté, le mot de passe n'est demandé qu'une fois par
  navigateur (jusqu'à `lock()` ou vidage du `localStorage`).
- `api.js` renvoie ce token via `X-Characters-Access` sur toutes les requêtes ;
  le backend le valide sur `/universes*`, `/characters*` et `POST /cosmos`.

---

### `useReverb.js` — Singleton WebSocket

Echo est instancié **une seule fois** (singleton) avec le `playerId` comme header d'auth.

```js
// Création de l'instance (appelée après avoir le playerId)
const { joinRoom, leaveRoom } = useReverb(playerId.value)

// Reset si changement de joueur (ex: refresh)
import { resetEcho } from './useReverb'
resetEcho() // déconnecte et recrée l'instance
```

**Headers d'auth envoyés à chaque requête `/broadcasting/auth` :**
```js
auth: {
    headers: {
        'X-Player-Id': playerId,
        'Accept': 'application/json',
    }
}
```

#### Méthodes disponibles

| Méthode | Channel | Description |
|---|---|---|
| `joinRoom(roomId, callbacks)` | `presence-room.{id}` | Rejoindre le lobby WebSocket |
| `leaveRoom(roomId)` | `presence-room.{id}` | Quitter le lobby |
| `joinGame(gameId, callbacks)` | `presence-game.{id}` | Rejoindre la partie WebSocket |
| `leaveGame(gameId)` | `presence-game.{id}` | Quitter la partie |

#### Callbacks disponibles pour `joinRoom`

```js
joinRoom(roomId, {
    onHere:           (members) => {},   // liste initiale des connectés
    onJoining:        (member)  => {},   // quelqu'un se connecte au channel
    onLeaving:        (member)  => {},   // quelqu'un se déconnecte
    onPlayerJoined:        (data)  => {},   // event: un joueur a rejoint le salon
    onPlayerReady:         (data)  => {},   // event: toggle prêt/pas prêt
    onPlayerLeft:          (data)  => {},   // event: un joueur a quitté le salon
    onRoomSettingsUpdated: (data)  => {},   // event: l'hôte change le mode de jeu / cosmos
    onGameStarted:         (data)  => {},   // event: la partie démarre
    onError:               (error) => {},   // erreur d'auth ou de connexion
})
```

#### Callbacks disponibles pour `joinGame`

```js
joinGame(gameId, {
    onHere:              (members) => {},
    onJoining:           (member)  => {},
    onLeaving:           (member)  => {},
    onRoundStarted:      (data)    => {},
    onActionPlayed:      (data)    => {},
    onAnswerGiven:       (data)    => {},
    onBinomeDiscovered:  (data)    => {},
    onGameEnded:         (data)    => {},
    onError:             (error)   => {},
})
```

---

## Session persistée

La session du joueur est stockée dans `localStorage` sous la clé `session` :

```json
{
    "roomId": 1,
    "gameCode": "ABC123",
    "playerId": 42,
    "hostId": 42,
    "isHost": true
}
```

### Cycle de vie de la session

```
handleCreateGame() → saveSession()
handleJoinGame()   → saveSession()
handleStartGame()  → saveSession() (ajoute gameId)
onGameStarted      → saveSession() (ajoute gameId)
handleLeaveRoom()  → clearSession()
onMounted          → restoreSession() → roomService.get() → initLobby()
```

### `restoreSession()`

Au `onMounted` de `RoomPage`, si une session existe :
1. Appelle `GET /api/rooms/{roomId}` pour vérifier que la room existe encore
2. Restaure le state Vue
3. Appelle `resetEcho()` pour recréer l'instance avec le bon `playerId`
4. Appelle `initLobby()` pour se reconnecter au channel WebSocket

Si la room n'existe plus → `clearSession()`.

---

## Pages

### `RoomPage.vue`

Page principale du lobby. Gère :

- **Créer un salon** → `POST /api/rooms`
- **Rejoindre un salon** → `POST /api/rooms/join`
- **Toggle prêt** → `PATCH /api/rooms/{room}/ready`
- **Mode de jeu** (hôte) → `PATCH /api/rooms/{room}/settings` : « Aléatoire » ou « Cosmos imposé »
  (dropdown alimenté par `characterService.listCosmos()`, options grisées si
  `playable_universe_count < ceil(nbJoueurs / 2)`). Les autres joueurs voient le mode en lecture seule.
- **Quitter le salon** → `DELETE /api/rooms/{room}/leave` + confirmation modal
- **Lancer la partie** → `POST /api/rooms/{room}/start` (hôte uniquement ; bouton désactivé tant
  qu'un cosmos infaisable ou aucun cosmos n'est choisi en mode « Cosmos imposé »)
- **WebSocket lobby** → `presence-room.{roomId}`
- **Session** → sauvegarde/restauration localStorage

#### State principal

| Ref | Type | Description |
|---|---|---|
| `roomId` | Number | ID de la room (pour les appels API) |
| `gameCode` | String | Code affiché aux joueurs |
| `playerId` | Number | ID du joueur connecté |
| `hostId` | Number | ID de l'hôte du salon |
| `isHost` | Boolean | Le joueur actuel est-il l'hôte ? |
| `players` | Array | Liste des joueurs avec `is_ready` |
| `gameStatus` | String | `waiting` ou `in_progress` |
| `gameMode` | String | `random` ou `cosmos` |
| `cosmosId` | Number\|null | Cosmos imposé (mode `cosmos`) |
| `cosmosOptions` | Array | Cosmos + `playable_universe_count` (via `characterService.listCosmos()`) |
| `isCurrentPlayerReady` | Computed | Statut prêt du joueur actuel |
| `pairsNeeded` / `selectedCosmosFeasible` / `startBlockedReason` | Computed | Faisabilité du cosmos vs nb de joueurs |

#### Comportement temps réel

| Event reçu | Action dans le state |
|---|---|
| `player.joined` | `players.value = data.players` |
| `player.ready` | `players.value = data.players` |
| `room.settings.updated` | `gameMode` / `cosmosId` mis à jour chez tous les joueurs |
| `player.left` | `players.value = data.players` + update `hostId` si transfert |
| `game.started` | Redirect vers `RoundPage` avec `gameId` |

### `RoundPage.vue`

Page de jeu principale. Gère :

- Affichage du personnage secret assigné au joueur (avec toggle flou)
- Mots interdits du joueur
- Bannière de tour en cours (temps réel)
- Actions : poser une question ou faire une accusation
- Modale de réponse oui/non/je ne sais pas (s'ouvre automatiquement chez le joueur ciblé)
- Historique complet des actions groupées par round
- Animation de transition entre les rounds
- Notifications de binôme découvert
- Modale de fin de partie

---

## Services API

### `roomService.js`

| Méthode | HTTP | Endpoint | Body |
|---|---|---|---|
| `create(pseudo)` | POST | `/rooms` | `{ pseudo, is_private, max_players }` |
| `join(code, pseudo)` | POST | `/rooms/join` | `{ pseudo, code }` |
| `get(roomId)` | GET | `/rooms/{id}` | — |
| `ready(roomId, playerId)` | PATCH | `/rooms/{id}/ready` | `{ player_id }` |
| `updateSettings(roomId, playerId, { gameMode, cosmosId })` | PATCH | `/rooms/{id}/settings` | `{ player_id, game_mode, cosmos_id }` |
| `leave(roomId, playerId)` | DELETE | `/rooms/{id}/leave` | `{ player_id }` |
| `start(roomId, playerId)` | POST | `/rooms/{id}/start` | `{ player_id }` |

### `gameService.js`

| Méthode | HTTP | Endpoint | Body |
|---|---|---|---|
| `show(gameId)` | GET | `/games/{id}` | — |
| `myCharacter(gameId, playerId)` | GET | `/games/{id}/me` | `?player_id=X` |
| `playQuestion(gameId, roundId, playerId, targetPlayerId, question)` | POST | `/games/{id}/rounds/{id}/question` | `{ player_id, target_player_id, question }` |
| `playAccusation(gameId, roundId, playerId, targetId, characterId)` | POST | `/games/{id}/rounds/{id}/accusation` | `{ player_id, target_player_id, character_id }` |
| `playAnswer(gameId, roundId, actionId, playerId, answer)` | POST | `/games/{id}/rounds/{id}/actions/{id}/answer` | `{ player_id, answer }` |

---

## Flux complet côté front

```
1. ARRIVÉE SUR RoomPage
   onMounted → restoreSession()
     ├── session trouvée → get(roomId) → initLobby()   [reconnexion]
     └── pas de session  → affiche les boutons Créer/Rejoindre

2. CRÉER UN SALON
   handleCreateGame()
     → POST /api/rooms
     → saveSession()
     → initLobby(roomId) → joinRoom(roomId, callbacks)
     → POST /broadcasting/auth (X-Player-Id: playerId)
     → onHere([moi]) → players mis à jour

3. REJOINDRE UN SALON
   handleJoinGame()
     → POST /api/rooms/join
     → saveSession()
     → initLobby(roomId)
     → POST /broadcasting/auth
     → onHere([...joueurs]) + broadcast player.joined reçu par les autres

4. TOGGLE PRÊT
   handleReady()
     → PATCH /api/rooms/{room}/ready
     → broadcast player.ready → onPlayerReady → players mis à jour chez tous

5. QUITTER LE SALON
   [clic bouton] → showLeaveModal = true
   [confirme]    → handleLeaveRoom()
     → DELETE /api/rooms/{room}/leave
     → leaveRoom(roomId)
     → clearSession()
     → reset state
     → broadcast player.left → onPlayerLeft → liste mise à jour chez les autres

6. LANCER LA PARTIE (hôte)
   handleStartGame()
     → POST /api/rooms/{room}/start
     → saveSession() avec gameId
     → broadcast game.started reçu par TOUS les joueurs
     → onGameStarted → router.push('RoundPage', { gameId })

7. PAGE DE JEU (RoundPage)
   → GET /api/games/{game}/me  (personnage secret + mots interdits)
   → joinGame(gameId, callbacks)
   → écoute round.started, action.played, binome.discovered, game.ended
```

---

## Points d'attention importants

### `useReverb` doit être appelé dans une fonction, jamais en racine de `<script setup>`

```js
// ❌ FAUX — playerId pas encore déclaré
const { joinRoom } = useReverb(playerId.value)

// ✅ CORRECT — appelé dans une fonction, playerId connu
function initLobby(id) {
    const { joinRoom } = useReverb(playerId.value)
    joinRoom(id, { ... })
}
```

### `resetEcho()` requis après un refresh

Le singleton Echo est recréé à chaque session avec le bon `playerId`. Sans `resetEcho()`, l'ancien Echo sans `X-Player-Id` serait réutilisé → 403 sur `/broadcasting/auth`.

### Axios DELETE avec body

```js
// Pour les requêtes DELETE avec body, axios nécessite la clé `data`
api.delete(`/rooms/${roomId}/leave`, {
    data: { player_id: playerId }
})
```

### `ShouldBroadcastNow` côté backend

Tous les events backend utilisent `ShouldBroadcastNow`. Si tu vois qu'un event ne se déclenche pas côté front, vérifie que l'event backend n'utilise pas `ShouldBroadcast` (qui nécessite une queue).

### Vider le cache Vite si les variables `.env` ne sont pas prises en compte

```bash
rm -rf node_modules/.vite
npm run dev -- --host
```