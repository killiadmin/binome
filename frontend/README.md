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
│   ├── game/
│   │   ├── GameNotepad.vue        # Bloc-notes privé (modale) : note libre + une note par joueur
│   │   ├── GameRecap.vue          # Récap de fin : trophées, qui a trouvé qui, binômes, partage
│   │   └── QuickReactions.vue     # Bouton flottant d'emojis + animation des réactions reçues
│   ├── player/
│   │   ├── PlayerAvatar.vue       # Contenu d'un badge joueur : photo, sinon initiales
│   │   └── AvatarEditor.vue       # Prise de vue / import → recadrage → envoi de sa photo
│   ├── room/
│   │   └── JoinQrCode.vue         # QR code du salon → /rooms?code=XXXXXX sur l'IP LAN
│   ├── chat/
│   │   └── LobbyChat.vue         # Chat commun du hall (channel public `lobby`)
│   └── cropper/
│       ├── ImageDropzone.vue      # zone glisser-déposer / clic → File
│       └── ImageCropperModal.vue  # recadrage carré imposé → 512×512 JPEG
│
├── composables/
│   └── useEcho.js              # (non utilisé — remplacé par useReverb)
│
├── pages/
│   ├── Admin/
│   │   └── AdminGamesPage.vue    # Vue admin : lister / consulter / supprimer les parties (accès `charactersAccess`)
│   ├── Characters/
│   │   ├── CharacterPage.vue      # Créer un personnage (univers/cosmos existant ou nouveau)
│   │   └── CharacterListPage.vue  # Liste triée cosmos → univers → binômes, filtres, édition/suppression, validation des propositions
│   ├── Game/
│   │   └── RoundPage.vue       # Page de jeu (en cours de développement)
│   ├── History/
│   │   └── HistoryPage.vue     # Historique public + classement cumulé (2 onglets)
│   ├── Home/
│   │   └── HomePage.vue
│   ├── Rooms/
│   │   └── RoomPage.vue        # Lobby — créer/rejoindre/gérer un salon
│   └── Rules/
│       └── RulePage.vue
│
├── services/
│   ├── adminService.js         # Appels API vue admin des parties (liste / détail / suppression)
│   ├── api.js                  # Instance Axios configurée
│   ├── charactersAccess.js     # Déverrouillage par mot de passe des pages personnages (token en session)
│   ├── characterService.js     # Appels API personnages/univers/cosmos + validation
│   ├── chatIdentity.js         # Identifiant anonyme persistant (`Anonyme#1234`) pour le chat
│   ├── chatService.js          # Appels API chat commun (historique / envoi)
│   ├── gameService.js          # Appels API partie
│   ├── historyService.js       # Historique public + classement (aucune auth)
│   └── roomService.js          # Appels API salon
│
├── sockets/
│   └── useReverb.js            # Singleton Echo + joinRoom/joinGame/joinLobbyChat
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

Les pages **Liste des personnages** (`/characters/list`), **Création de
personnages** (`/characters`) et **Parties** (`/admin/games` — vue admin :
lister / consulter / supprimer les parties) sont masquées par défaut et
protégées par un mot de passe (défini côté backend dans
`CHARACTERS_ACCESS_PASSWORD`). Un seul déverrouillage donne accès aux trois.

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

Deux helpers complètent le singleton :

- `onEchoReset(cb)` — s'abonner à la destruction de l'instance (renvoie la fonction de désabonnement).
  Utilisé par les channels qui ne sont pas re-souscrits par la page appelante, comme le chat du hall.
- `hasEcho()` — savoir si le singleton existe, pour éviter de rouvrir une connexion juste pour la fermer
  dans un `onUnmounted`.

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
| `joinLobbyChat(callbacks)` | `lobby` (**public**) | Chat commun du hall — aucune auth, donc accessible sans `playerId` |
| `leaveLobbyChat()` | `lobby` | Quitter le chat commun |
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

#### Callbacks disponibles pour `joinLobbyChat`

```js
joinLobbyChat({
    onChatMessage: (data) => {},   // event: nouveau message dans le chat commun
    onError:       (error) => {},
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
    onReactionSent:      (data)    => {},   // réaction rapide (emoji)
    onTurnSkipped:       (data)    => {},   // l'hôte a passé le tour d'un joueur déconnecté
    onPlayerExcluded:    (data)    => {},   // l'hôte a exclu un joueur déconnecté
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
- **Chat commun** → `<LobbyChat>` monté sous les boutons « Créer / Rejoindre »
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
| `game.started` | Redirect vers `RoundPage` avec `gameId` (payload : `players[]` à plat + `has_orphan`, jamais les binômes) |

#### Photo de profil

- Dans le lobby, toucher **son propre jeton** (badge 📷) ouvre `AvatarEditor` : « Prendre une
  photo » / « Importer une photo » → `ImageCropperModal` (carré, sortie **256 px JPEG q0.85**,
  via les props `output-size` / `quality`) → `avatarService.upload()` (data URI).
- Prise de vue : sur téléphone, `<input type="file" capture="user">` ouvre la caméra frontale et
  marche en `http://` sur l'IP LAN. Sur ordinateur, `getUserMedia` n'existe qu'en contexte
  sécurisé (https / **localhost**) : webcam dans la modale pour l'hôte sur localhost, bouton
  masqué pour un PC qui passe par l'IP LAN (l'import reste possible).
- Affichage : `PlayerAvatar` remplit les badges existants (lobby, jetons et fiche en partie,
  choix de la cible, bloc-notes) avec `avatar_url`, et retombe sur les initiales si l'image
  échoue. Mise à jour en direct via `player.avatar.updated` (lobby et partie).
- Mini-avatar (`.mini-avatar`, décoratif : `aria-hidden`) devant les pseudos du panneau
  « Historique de la partie » (photo retrouvée par id via `avatarById`) et de `/historique`
  (joueurs, scores, journal, classement — `avatar_url` fourni par l'API).

#### Lien de salon, QR code, spectateur

- **`/rooms?code=XXXXXX`** ouvre directement la modale « Rejoindre » avec le code rempli
  (`prefillJoinFromLink()`, ignoré si on est déjà dans un salon), puis retire la query de l'URL.
- **QR code** (bouton sous l'écran LCD) → `JoinQrCode.vue` : même résolution d'adresse que
  « Partager le lien » (`resolveJoinUrl()`, donc `lan-url.json` quand l'hôte est sur localhost),
  SVG généré localement par la lib `qrcode`.
- **Partie déjà en cours** (`current_game_id` renvoyé par `join` / `show`) : bannière
  « Regarder en spectateur » → `gameId` sauvé en session, redirection vers `RoundPage`.

### `LobbyChat.vue` (composant)

Chat commun affiché sur `/rooms`, sous les boutons « Créer une partie » / « Rejoindre une partie »
(et sous la carte du salon une fois celui-ci rejoint).

- **Visibilité** : déplié par défaut tant que le visiteur n'a ni créé ni rejoint de salon ; replié
  par défaut dès qu'il en rejoint un (bouton **Afficher / Masquer**, avec pastille de messages non lus).
  Un `watch` sur `isInRoom` remet la visibilité à sa valeur par défaut à chaque entrée/sortie de salon.
- **Identité** : `player-id` quand le joueur est dans un salon → le backend signe le message avec son
  pseudo. Sinon, `chatIdentity.js` tire un identifiant stocké dans le `localStorage` (clé `chatAnonId`)
  et le message est signé `Anonyme#1234`. Le nom affiché est **toujours** calculé côté serveur.
- **Codes de partie copiables** : le backend renvoie, pour chaque message, la liste `codes` des codes
  de salon **réellement existants** qu'il contient. Le front découpe le corps du message sur ces codes
  et en fait des puces cliquables → `copyToClipboard()`. Un mot de 6 caractères qui ne correspond à
  aucun salon reste du texte normal.
- **Expiration silencieuse** : un message vit une heure (`MESSAGE_TTL_MS`, aligné sur
  `ChatController::RETENTION_MINUTES`). Le backend ne le sert plus au-delà ; un `setInterval` de 30 s
  (`expireOldMessages()`) le retire aussi de la liste locale, pour qu'un onglet laissé ouvert le voie
  disparaître sans rechargement. Aucun message système : il s'efface, point.
- **Re-souscription** : le channel `lobby` est public, mais il vit sur le singleton Echo que `RoomPage`
  détruit via `resetEcho()` à chaque changement d'identité. Le composant s'abonne donc à `onEchoReset()`
  et se rebranche **au `nextTick` suivant** — jamais de façon synchrone, sinon il recréerait le
  singleton avec l'ancien `X-Player-Id` et le `presence-room.{roomId}` de la page se ferait refuser.

| Event reçu | Action |
|---|---|
| `chat.message` (channel `lobby`) | `pushMessage()` — dédoublonné par `id` (le message posté arrive aussi par la réponse HTTP) |

### `HistoryPage.vue`

Route publique `/historique` — aucune authentification. Deux onglets :

- **Parties** : les parties **terminées** uniquement (le backend refuse le reste, cf. `HistoryController`).
  Chaque carte se déplie sur le détail complet : binômes, personnages révélés avec leurs mots interdits,
  scores de la partie et journal intégral des questions et accusations.
- **Classement** : une ligne par pseudo, triée sur le total cumulé, avec médailles pour le podium,
  rangs partagés en cas d'ex æquo et recherche par pseudo. Déplier un joueur charge son détail
  **partie par partie** (`/history/players?pseudo=…`) : le cumul global d'un côté, le split de l'autre.

Le détail d'une partie et celui d'un joueur sont chargés à la demande puis mémorisés (`details`,
`breakdowns`) pour éviter de recharger au repliage.

#### Pièges évités dans cette page

- **Ne jamais nommer une classe `card` / `card-title`** : ce sont des classes Bootstrap, chargé
  globalement par `main.js`. Bootstrap impose alors `color: var(--bs-body-color)` (texte sombre) sur
  un fond sombre et le texte devient illisible. Les classes sont préfixées `hist-card__…`, comme
  `AdminGamesPage.vue` utilise `game-card`.
- **`min-width: 0` sur `.table-scroll`** : enfant flex de `.detail`, sa largeur min-content (celle du
  tableau) remonterait sinon jusqu'au conteneur. Le tableau défile horizontalement dans la carte au
  lieu d'élargir la page.

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
- Bannière « un orphelin dans la partie » (voir ci-dessous)
- Modale de fin de partie

#### Spectateur, présence et reconnexion

- **Spectateur** (`isSpectator`) : pas dans `players[]` → pas d'appel à `/me` (404), pas de
  carte personnage, bannière dédiée. Un joueur **éliminé** garde sa carte et voit une bannière
  « Tu as été démasqué ». Les spectateurs connectés sont listés sous les joueurs.
- **Présence** : `here` / `joining` / `leaving` du channel `game.{id}` alimentent `members` et
  `offlineSince` (badge « hors ligne »). Un `leaving` n'est pris en compte qu'après 3 s
  (`LEAVE_GRACE_MS`) : un rechargement de page produit un leaving immédiatement suivi d'un joining.
- **Partie en pause** : `blocker` (miroir de `ActionService::findBlocker`) + `afkBlocker` →
  bannière « Partie en pause » quand le joueur attendu est hors ligne. **Seul l'hôte**
  (`host_id` de `GET /games/{game}`) voit les boutons, actifs après `AFK_GRACE_SECONDS` (30 s) :
  « Passer son tour » (absent quand le joueur doit répondre à une question — une réponse est
  obligatoire) et « Exclure » (modale de confirmation). Les autres voient « en attente de son
  retour ou de la décision de l'hôte », ou « la partie attend son retour » si c'est l'hôte qui
  est parti. Le serveur revérifie la présence via Reverb.
- **Exclusion** (`player.excluded`) : l'exclu et son binôme passent éliminés (badge « 🚪 exclu »),
  l'action annulée (`cancelled_action_id`) est retirée de l'historique et son auteur peut rejouer.
- **Reconnexion** : `onConnectionChange()` (useReverb) affiche « Connexion perdue » et, au retour
  à `connected`, appelle `resync()` → `loadGameState()` (même fonction que le chargement initial :
  actions, round, `hasPlayed`, modales en attente via `restorePendingPrompts()`). Même resync au
  retour sur l'onglet après plus de 3 s masqué (`visibilitychange`, téléphone verrouillé).
- Les modales réponse / confirmation se referment si `answer.given` / `accusation.confirmed`
  concerne leur action (réponse donnée depuis un autre onglet, tour passé) et à la fin de partie.

#### Bloc-notes, réactions, récap

- **Bloc-notes** (`composables/useGameNotes.js`) : `localStorage` uniquement, clé
  `binome:notes:{gameId}:{playerId}`, jamais envoyé au serveur ; les notes des autres parties
  sont purgées au chargement. Éditable depuis la fiche d'un joueur ou la modale `GameNotepad`.
- **Réactions** : `QuickReactions.vue` (liste d'emojis alignée sur `GameController::REACTIONS`),
  1,2 s de délai local entre deux envois + throttle serveur. La couche d'animation passe
  au-dessus des modales (z-index 1060) pour rester visible sur l'écran de fin.
- **Fin de partie** : `handleGameEnded()` charge `GET /games/{game}/recap` (avec reprises : l'event
  part depuis la transaction qui clôt la partie, le récap peut répondre 409 un court instant)
  puis affiche le tableau des scores + `GameRecap`. Une partie déjà terminée au chargement
  (refresh, reconnexion tardive) affiche directement le récap.
- Le récap liste aussi les exclusions (`recap.exclusions`) et le tableau des scores affiche
  « 🚪 Exclu » (score 0).

#### Orphelin (nombre de joueurs impair)

Quand la partie démarre à un nombre impair de joueurs, un joueur tiré au sort n'a
pas de binôme. Le backend n'envoie **jamais** la composition des binômes tant qu'aucun
n'est découvert : `GET /games/{game}` renvoie `players[]` **à plat** plus un booléen
`has_orphan`. La page l'utilise pour deux choses :

- `hasOrphan` → bannière `.orphan-banner` : tout le monde sait qu'un orphelin existe,
  personne ne sait qui — **pas même l'orphelin lui-même**, qui joue une partie normale.
- À la fin, `GameEnded.all_binomes[].is_orphan` alimente `orphanPseudos` : chip
  « 🚷 Orphelin » au tableau des scores, et message de fin adapté si c'était toi.

L'orphelin marque comme un binôme intact (bonus de +5) s'il finit dernier en jeu.
`RoomPage` prévient l'hôte dès le lobby dès que le compte est impair.

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
| `recap(gameId)` | GET | `/games/{id}/recap` | — |
| `react(gameId, playerId, emoji)` | POST | `/games/{id}/reactions` | `{ player_id, emoji }` |
| `skipTurn(gameId, playerId)` | POST | `/games/{id}/skip-turn` | `{ player_id }` (hôte) |
| `excludePlayer(gameId, targetPlayerId, playerId)` | POST | `/games/{id}/players/{target}/exclude` | `{ player_id }` (hôte) |

### `historyService.js`

| Méthode | HTTP | Endpoint | Body |
|---|---|---|---|
| `listGames()` | GET | `/history/games` | — |
| `getGame(id)` | GET | `/history/games/{id}` | — |
| `leaderboard()` | GET | `/history/leaderboard` | — |
| `player(pseudo)` | GET | `/history/players` | `?pseudo=X` |

### `chatService.js`

| Méthode | HTTP | Endpoint | Body |
|---|---|---|---|
| `list()` | GET | `/chat/messages` | — |
| `send({ body, playerId, anonId })` | POST | `/chat/messages` | `{ body, player_id, anon_id }` |

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

### `#app { min-width: 0 }` — sans lui, tout élément large élargit la page
`style.css` met `body` en `display: flex`. Un enfant flex a `min-width: auto` par défaut et **refuse
de rétrécir sous la largeur min-content de son contenu** : un seul tableau (ou la navbar) suffisait
alors à faire passer toute la page de 320 à 389 px sur mobile, au lieu de la laisser défiler ou se
replier. Mesuré via le protocole DevTools : `docScrollWidth=389` avant, `320` après.

À garder en tête pour toute nouvelle page : c'est la largeur **min-content** du contenu qui compte,
pas sa largeur souhaitée. Un tableau, un mot très long ou un titre en « Press Start 2P » peuvent
dépasser. Le repli habituel est `min-width: 0` sur l'enfant flex + `overflow-x: auto` sur un conteneur
dédié.

### Vider le cache Vite si les variables `.env` ne sont pas prises en compte

```bash
rm -rf node_modules/.vite
npm run dev -- --host
```