<script setup>
import {ref, onMounted, onUnmounted, computed, watch} from 'vue'
import {useRoute, useRouter} from 'vue-router'
import {roomService} from '../../services/roomService'
import {characterService} from '../../services/characterService'
import {gameService} from '../../services/gameService'
import {useReverb} from '../../sockets/useReverb'
import {BButton, BCard, BContainer, BRow, BCol, BModal, BFormInput, BAlert, BSpinner, BBadge} from 'bootstrap-vue-next'
import {resetEcho} from "../../sockets/useReverb.js";
import {copyToClipboard} from '../../services/copyToClipboard'
import LobbyChat from '../../components/chat/LobbyChat.vue'
import JoinQrCode from '../../components/room/JoinQrCode.vue'
import PlayerAvatar from '../../components/player/PlayerAvatar.vue'
import AvatarEditor from '../../components/player/AvatarEditor.vue'
import PasswordPromptModal from '../../components/player/PasswordPromptModal.vue'
import {profileService} from '../../services/profileService'
import {isLoggedIn, profilePlayer, setProfile, updateProfilePlayer} from '../../services/profile'
import {resumeFromProfile} from '../../services/gameSession'

const route = useRoute()
const router = useRouter()

// ─── STATE ────────────────────────────────────────────────────────────────────

const playerName = ref('')
const showCreateModal = ref(false)
const showJoinModal = ref(false)
const codeToJoin = ref('')
const error = ref(null)

// ─── PROFIL ───────────────────────────────────────────────────────────────────
// Connecté : le pseudo est celui du profil, non modifiable. Créer ou rejoindre
// un salon connecte automatiquement au profil du pseudo saisi ; on propose
// alors une seule fois de le protéger par un mot de passe.
watch(profilePlayer, (p) => { if (p) playerName.value = p.pseudo }, {immediate: true})
const showPasswordPrompt = ref(false)
// Pseudo protégé saisi sans être connecté : lien vers la connexion
const protectedPseudo = ref(null)

function afterRoomEntry(data) {
  if (!data.profile_token || !data.profile) return
  setProfile(data.profile_token, data.profile)
  const p = data.profile
  if (!p.has_password && !p.password_prompted && !p.is_admin) showPasswordPrompt.value = true
}

function roomEntryError(e, fallback) {
  const status = e.response?.status
  const message = e.response?.data?.message
  protectedPseudo.value = status === 403 && /protégé/.test(message ?? '') ? playerName.value.trim() : null
  return message || fallback
}

const roomId = ref(null)
const gameCode = ref(null)
const gameId = ref(null)
const playerId = ref(null)
const players = ref([])
const gameStatus = ref('waiting')
const isHost = ref(false)
const hostId = ref(null)
const startingGame = ref(false)
const gameStarting = ref(false)
const showRejoinModal = ref(false)
const pendingGameId   = ref(null)

// Partie déjà lancée dans ce salon (arrivée en retard) : on peut la suivre en spectateur.
const currentGameId = ref(null)
const showQr = ref(false)

// Photo de profil : le joueur touche son propre jeton pour l'ouvrir
const showAvatarEditor = ref(false)
const me = computed(() => players.value.find(p => p.id === playerId.value) ?? null)

function setAvatar(id, url) {
  players.value = players.value.map(p => p.id === id ? {...p, avatar_url: url} : p)
}

// ─── MODE DE JEU ──────────────────────────────────────────────────────────────
const gameMode = ref('random')          // 'random' | 'cosmos'
const cosmosId = ref(null)
const cosmosOptions = ref([])            // [{ id, name, playable_universe_count }]
// Options pour les joueurs éliminés (actives par défaut côté serveur)
const revealToEliminated = ref(true)     // vue omnisciente
const bestQuestionVote = ref(true)       // vote « meilleure question du round »
const savingSettings = ref(false)

// Un joueur surnuméraire forme quand même une « équipe » : il consomme son
// propre univers, d'où le ceil().
const pairsNeeded = computed(() => Math.max(1, Math.ceil(players.value.length / 2)))

// Nombre impair : le dernier joueur tiré au sort jouera sans binôme.
const willHaveOrphan = computed(
    () => players.value.length >= 4 && players.value.length % 2 !== 0
)

const selectedCosmos = computed(
    () => cosmosOptions.value.find(c => c.id === cosmosId.value) || null
)

const selectedCosmosFeasible = computed(
    () => !!selectedCosmos.value && selectedCosmos.value.playable_universe_count >= pairsNeeded.value
)

const startBlockedReason = computed(() => {
  if (gameMode.value !== 'cosmos') return null
  if (!cosmosId.value) return 'Choisis un cosmos pour démarrer.'
  if (!selectedCosmosFeasible.value) {
    const n = selectedCosmos.value?.playable_universe_count ?? 0
    return `${selectedCosmos.value?.name} : ${n} binôme(s) possible(s), il en faut ${pairsNeeded.value}.`
  }
  return null
})

async function loadCosmosOptions() {
  try {
    const res = await characterService.listCosmos()
    cosmosOptions.value = res.data.cosmos ?? []
  } catch {
    cosmosOptions.value = []
  }
}

async function handleUpdateSettings() {
  if (!isHost.value || !roomId.value) return
  // En mode cosmos, on n'enregistre qu'une fois le cosmos choisi (évite un 422 inutile).
  if (gameMode.value === 'cosmos' && !cosmosId.value) return
  savingSettings.value = true
  error.value = null

  try {
    await roomService.updateSettings(roomId.value, playerId.value, {
      gameMode: gameMode.value,
      cosmosId: gameMode.value === 'cosmos' ? cosmosId.value : null,
      revealToEliminated: revealToEliminated.value,
      bestQuestionVote: bestQuestionVote.value,
    })
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors de la mise à jour du mode de jeu.'
  } finally {
    savingSettings.value = false
  }
}

// ─── SESSION localStorage ─────────────────────────────────────────────────────

function saveSession() {
  localStorage.setItem('session', JSON.stringify({
    roomId: roomId.value,
    gameCode: gameCode.value,
    playerId: playerId.value,
    hostId: hostId.value,
    isHost: isHost.value,
    gameId: gameId.value,
  }))
}

function clearSession() {
  localStorage.removeItem('session')
}

async function restoreSession() {
  let raw = localStorage.getItem('session')
  if (!raw && isLoggedIn.value) {
    // Autre navigateur : on retrouve le salon / la partie du profil connecté
    try {
      const {data} = await profileService.show()
      updateProfilePlayer(data.player)
      if (resumeFromProfile(data.player.id, data.current)) raw = localStorage.getItem('session')
    } catch { /* profil indisponible : hall normal */ }
  }
  if (!raw) return

  try {
    const session = JSON.parse(raw)

    // Session de jeu sans le jeton de CE joueur (session antérieure aux jetons,
    // profil déconnecté…) : le serveur refuserait toute action et tout canal
    // temps réel. La connexion ramène au salon / à la partie en cours.
    if (!isLoggedIn.value || profilePlayer.value?.id !== session.playerId) {
      clearSession()
      router.replace({name: 'Login', query: {expired: '1'}})
      return
    }

    if (session.gameId) {
      // La session n'est nettoyée que via le bouton du tableau des scores : si
      // le joueur a quitté autrement (navbar, retour, onglet fermé), le gameId
      // d'une partie terminée traîne encore. On vérifie avant de proposer.
      const game = await gameService.show(session.gameId)
      if (game.status === 'finished') {
        clearSession()
        return
      }

      pendingGameId.value = session.gameId
      playerId.value  = session.playerId
      gameCode.value  = session.gameCode
      showRejoinModal.value = true
      return
    }

    const res = await roomService.get(session.roomId)

    roomId.value   = session.roomId
    gameCode.value = session.gameCode
    playerId.value = session.playerId
    hostId.value   = session.hostId
    isHost.value   = session.isHost
    players.value  = res.data.room.players
    gameMode.value = res.data.room.game_mode ?? 'random'
    cosmosId.value = res.data.room.cosmos_id ?? null
    revealToEliminated.value = res.data.room.reveal_to_eliminated ?? true
    bestQuestionVote.value = res.data.room.best_question_vote ?? true
    currentGameId.value = res.data.room.current_game_id ?? null

    resetEcho()
    initLobby(session.roomId)

  } catch {
    clearSession()
  }
}

function handleRejoinGame() {
  showRejoinModal.value = false
  router.push({ name: 'RoundPage', params: { gameId: pendingGameId.value } })
}

// Arrivé après le lancement : pas de personnage, on suit la partie en spectateur.
// Le gameId est sauvé en session comme pour un joueur, le refresh et la modale
// « Partie en cours » fonctionnent donc de la même façon.
function handleWatchGame() {
  gameId.value = currentGameId.value
  saveSession()
  router.push({ name: 'RoundPage', params: { gameId: currentGameId.value } })
}

// Lien de partage / QR code : /rooms?code=XXXXXX ouvre directement la modale
// « Rejoindre » avec le code rempli. Ignoré si on est déjà dans un salon.
function prefillJoinFromLink() {
  const code = String(route.query.code ?? '').trim().toUpperCase()
  if (!code) return
  router.replace({ query: {} })
  if (gameCode.value || showRejoinModal.value) return
  codeToJoin.value = code
  // Connecté : le pseudo est connu, le code aussi → on entre directement
  if (isLoggedIn.value) handleJoinGame()
  else showJoinModal.value = true
}

// Connecté, le pseudo est le seul champ : pas de modale, création immédiate.
function openCreate() {
  error.value = null
  protectedPseudo.value = null
  if (isLoggedIn.value) handleCreateGame()
  else showCreateModal.value = true
}

function openJoin() {
  error.value = null
  protectedPseudo.value = null
  showJoinModal.value = true
}

function handleAbandonGame() {
  clearSession()
  showRejoinModal.value = false
  pendingGameId.value   = null
  playerId.value        = null
  gameCode.value        = null
}

// ─── WEBSOCKET LOBBY ──────────────────────────────────────────────────────────

function initLobby(id) {
  const {joinRoom} = useReverb(playerId.value)

  joinRoom(id, {
    onHere: (members) => {
      players.value = players.value.map(p => ({
        ...p,
        online: members.some(m => m.id === p.id),
      }))
      members.forEach(member => {
        const exists = players.value.find(p => p.id === member.id)
        if (!exists) {
          players.value.push({
            id: member.id,
            pseudo: member.pseudo,
            is_ready: false,
            online: true,
          })
        }
      })
    },
    onJoining: (member) => {
      const player = players.value.find(p => p.id === member.id)
      if (player) {
        if (player._offlineTimeout) {
          clearTimeout(player._offlineTimeout)
          player._offlineTimeout = null
        }
        player.online = true
      } else {
        players.value.push({
          id: member.id,
          pseudo: member.pseudo,
          is_ready: false,
          online: true,
        })
      }
    },
    onLeaving: (member) => {
      const player = players.value.find(p => p.id === member.id)
      if (player) {
        player._offlineTimeout = setTimeout(() => {
          player.online = false
        }, 2000)
      }
    },
    onPlayerJoined: (data) => {
      players.value = data.players.map(p => ({ ...p, online: true }))
    },
    onPlayerReady: (data) => {
      players.value = data.players
    },
    onPlayerAvatarUpdated: (data) => setAvatar(data.player_id, data.avatar_url),
    onRoomSettingsUpdated: (data) => {
      gameMode.value = data.game_mode ?? 'random'
      cosmosId.value = data.cosmos_id ?? null
      revealToEliminated.value = data.reveal_to_eliminated ?? true
      bestQuestionVote.value = data.best_question_vote ?? true
    },
    onGameStarted: (data) => {
      gameStatus.value = 'in_progress'
      gameId.value = data.game_id
      saveSession()
      router.push({ name: 'RoundPage', params: { gameId: data.game_id } })
    },
    onError: () => {
      error.value = 'Connexion WebSocket perdue. Recharge la page.'
    },
    onPlayerLeft: (data) => {
      // Retiré par un administrateur (onglet Joueurs) : on quitte le lobby.
      if (!data.players.some(p => p.id === playerId.value)) {
        resetLocalRoom()
        error.value = 'Tu as été retiré du salon par un administrateur.'
        return
      }

      players.value = data.players

      if (data.new_host_id) {
        hostId.value = data.new_host_id
        if (data.new_host_id === playerId.value) {
          isHost.value = true
        }
      }
    },
  })
}

// ─── ACTIONS ──────────────────────────────────────────────────────────────────

const handleCreateGame = async () => {
  if (!playerName.value.trim()) {
    error.value = 'Entre un pseudo pour continuer.'
    return
  }
  error.value = null

  try {
    const res = await roomService.create(playerName.value)

    roomId.value = res.data.room.id
    gameCode.value = res.data.room.code
    playerId.value = res.data.player.id
    hostId.value = res.data.player.id
    isHost.value = true
    gameMode.value = 'random'
    cosmosId.value = null
    revealToEliminated.value = true
    bestQuestionVote.value = true
    players.value = [{
      id: res.data.player.id,
      pseudo: res.data.player.pseudo,
      is_ready: false,
      avatar_url: res.data.player.avatar_url ?? null,
    }]

    // Jeton du profil AVANT d'ouvrir le WebSocket : le serveur en déduit le joueur
    afterRoomEntry(res.data)
    saveSession()
    resetEcho()
    initLobby(roomId.value)

    showCreateModal.value = false
  } catch (e) {
    error.value = roomEntryError(e, 'Erreur lors de la création du salon.')
  }
}

const handleJoinGame = async () => {
  if (!playerName.value.trim() || !codeToJoin.value.trim()) {
    error.value = playerName.value.trim() ? 'Entre le code du salon.' : 'Entre un pseudo pour continuer.'
    return
  }
  error.value = null

  try {
    const res = await roomService.join(codeToJoin.value, playerName.value)

    roomId.value = res.data.room.id
    gameCode.value = res.data.room.code
    playerId.value = res.data.player.id
    players.value = res.data.room.players
    gameMode.value = res.data.room.game_mode ?? 'random'
    cosmosId.value = res.data.room.cosmos_id ?? null
    revealToEliminated.value = res.data.room.reveal_to_eliminated ?? true
    bestQuestionVote.value = res.data.room.best_question_vote ?? true
    currentGameId.value = res.data.room.current_game_id ?? null
    isHost.value = false

    // Jeton du profil AVANT d'ouvrir le WebSocket : le serveur en déduit le joueur
    afterRoomEntry(res.data)
    saveSession()
    resetEcho()
    initLobby(roomId.value)

    showJoinModal.value = false
  } catch (e) {
    error.value = roomEntryError(e, 'Erreur lors de la connexion au salon.')
  }
}

const handleReady = async () => {
  if (!roomId.value || !playerId.value) return
  error.value = null

  try {
    await roomService.ready(roomId.value, playerId.value)
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur.'
  }
}

const handleStartGame = async () => {
  error.value = null
  startingGame.value = true

  try {
    const res = await roomService.start(roomId.value, playerId.value)
    gameId.value = res.data.game_id
    saveSession()

    await router.push({
      name: 'RoundPage',
      params: {gameId: res.data.game_id}
    })

  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors du démarrage.'
    startingGame.value = false
  }
}

const showLeaveModal = ref(false)

// ─── Copie du code de la partie (clic sur l'écran LCD pour le partager) ────────
const codeCopied = ref(false)
let codeCopiedTimer = null

const copyGameCode = async () => {
  if (!gameCode.value) return
  const ok = await copyToClipboard(gameCode.value)
  if (!ok) return
  codeCopied.value = true
  clearTimeout(codeCopiedTimer)
  codeCopiedTimer = setTimeout(() => { codeCopied.value = false }, 2000)
}

const handleLeaveRoom = async () => {
  try {
    await roomService.leave(roomId.value, playerId.value)
  } catch (e) {
    // 409 : le joueur est dans la partie en cours, il reste dans le salon
    if (e.response?.status === 409) {
      error.value = e.response.data?.message || 'Impossible de quitter le salon pendant la partie.'
      showLeaveModal.value = false
      return
    }
    console.error('Erreur leave:', e)
  }

  resetLocalRoom()
}

// Retour à l'écran d'accueil du hall : départ volontaire ou retrait par un admin.
function resetLocalRoom() {
  const {leaveRoom} = useReverb(playerId.value)
  leaveRoom(roomId.value)
  resetEcho()
  clearSession()
  roomId.value = null
  gameCode.value = null
  playerId.value = null
  currentGameId.value = null
  showQr.value = false
  players.value = []
  isHost.value = false
  hostId.value = null
  showLeaveModal.value = false
}

const isCurrentPlayerReady = computed(() => {
  const me = players.value.find(p => p.id === playerId.value)
  return me?.is_ready ?? false
})

// ─── LIFECYCLE ────────────────────────────────────────────────────────────────

onMounted(async () => {
  loadCosmosOptions()
  await restoreSession()
  prefillJoinFromLink()
})

onUnmounted(() => {
  if (roomId.value) {
    const {leaveRoom} = useReverb(playerId.value)
    leaveRoom(roomId.value)
  }
})

// ─── UI HELPERS ───────────────────────────────────────────────────────────────
const getGameStatusText = (s) => s === 'in_progress' ? 'En cours' : 'En attente'
const getGameStatusClass = (s) => s === 'in_progress' ? 'text-danger' : 'text-success'
</script>

<template>
  <div class="arcade-page arcade-bg">
    <BContainer>
      <div class="arcade-title-wrap">
        <h1 class="arcade-title">Salons de jeu</h1>
        <p class="arcade-subtitle">Rassemble ton binôme et lance la partie !</p>
      </div>

      <div v-if="error && !showCreateModal && !showJoinModal" class="arcade-alert">
        <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
        <router-link v-if="protectedPseudo" :to="{ name: 'Login', query: { pseudo: protectedPseudo } }" class="login-link">
          Se connecter
        </router-link>
      </div>

      <!-- Boutons principaux : masqués quand un salon est actif, pour libérer l'écran -->
      <div v-if="!gameCode" class="cabinet-buttons">
        <button class="cabinet-btn stacked" type="button" @click="openCreate">
          <span class="cabinet-btn__icon"><i class="fa-solid fa-plus"></i></span>
          <span class="cabinet-btn__label">Créer une partie</span>
        </button>
        <button class="cabinet-btn stacked" type="button" @click="openJoin">
          <span class="cabinet-btn__icon"><i class="fa-solid fa-right-to-bracket"></i></span>
          <span class="cabinet-btn__label">Rejoindre une partie</span>
        </button>
      </div>

      <!-- Carte du salon actif -->
      <BRow v-if="gameCode" class="justify-content-center" :style="{ paddingBottom: '100px' }">
        <BCol cols="12" md="7" lg="6" :style="{ minWidth: 'min(360px, 100%)' }">
          <div class="board-card">
            <div class="board-card__rivet board-card__rivet--tl"></div>
            <div class="board-card__rivet board-card__rivet--tr"></div>
            <div class="board-card__rivet board-card__rivet--bl"></div>
            <div class="board-card__rivet board-card__rivet--br"></div>

            <h2 class="board-card__title">Votre partie</h2>

            <div class="lcd-screen">
              <span class="lcd-label">Code</span>
              <button
                  type="button"
                  class="lcd-code lcd-code--copy"
                  :class="{ 'is-copied': codeCopied }"
                  :title="codeCopied ? 'Code copié !' : 'Cliquer pour copier le code et le partager'"
                  @click="copyGameCode"
              >
                <span v-for="(char, i) in gameCode.split('')" :key="i" class="lcd-char">{{ char }}</span>
              </button>
              <span class="lcd-copy-hint">
                <i class="fa-solid" :class="codeCopied ? 'fa-check' : 'fa-copy'"></i>
                {{ codeCopied ? 'Copié !' : 'Toucher pour copier' }}
              </span>
            </div>

            <div class="qr-toggle-wrap">
              <button type="button" class="qr-toggle" :aria-expanded="showQr" @click="showQr = !showQr">
                <i class="fa-solid fa-qrcode"></i>
                {{ showQr ? 'Masquer le QR code' : 'QR code pour rejoindre' }}
              </button>
            </div>
            <JoinQrCode v-if="showQr" :code="gameCode" />

            <div v-if="currentGameId" class="spectate-banner">
              <p class="spectate-banner__text">
                <i class="fa-solid fa-circle-play"></i>
                Une partie est déjà en cours dans ce salon.
                Tu joueras à la prochaine — en attendant, tu peux la regarder.
              </p>
              <button type="button" class="cabinet-btn cabinet-btn--sm" @click="handleWatchGame">
                <i class="fa-solid fa-eye"></i> Regarder en spectateur
              </button>
            </div>

            <div class="status-pills">
              <span class="pill">
                <i class="fa-solid fa-users"></i> {{ players.length }} joueur{{ players.length > 1 ? 's' : '' }}
              </span>
              <span class="pill" :class="gameStatus === 'in_progress' ? 'pill--danger' : 'pill--success'">
                <span class="pill-dot"></span>
                {{ getGameStatusText(gameStatus) }}
              </span>
              <span v-if="willHaveOrphan && gameStatus === 'waiting'" class="pill pill--orphan">
                <i class="fa-solid fa-user-slash"></i> 1 orphelin
              </span>
            </div>

            <p v-if="willHaveOrphan && gameStatus === 'waiting'" class="orphan-hint">
              <i class="fa-solid fa-circle-info"></i>
              Nombre impair : un joueur sera tiré au sort pour jouer sans binôme.
              Il gagne les mêmes points qu'un binôme s'il finit dernier en jeu —
              mais il ne saura pas qu'il est seul.
            </p>

            <!-- Liste des joueurs -->
            <div class="roster">
              <h5 class="roster__title">Participants</h5>
              <p v-if="me" class="roster__hint">
                <i class="fa-solid fa-camera"></i>
                Touche ton jeton pour {{ me.avatar_url ? 'changer' : 'ajouter' }} ta photo
              </p>
              <div class="roster__grid">
                <div
                    v-for="player in players"
                    :key="player.id"
                    class="player-token"
                    :class="{
                      'is-offline': player.online === false,
                      'is-ready': player.is_ready,
                      'is-mine': player.id === playerId,
                    }"
                    :role="player.id === playerId ? 'button' : undefined"
                    :tabindex="player.id === playerId ? 0 : undefined"
                    :aria-label="player.id === playerId ? 'Changer ma photo' : undefined"
                    @click="player.id === playerId && (showAvatarEditor = true)"
                    @keydown.enter.prevent="player.id === playerId && (showAvatarEditor = true)"
                >
                  <span v-if="player.id === hostId" class="player-token__crown">
                    <i class="fa-solid fa-crown"></i>
                  </span>
                  <span v-if="player.is_ready" class="player-token__stamp">
                    <i class="fa-solid fa-check"></i>
                  </span>
                  <div class="player-token__avatar">
                    <PlayerAvatar :pseudo="player.pseudo" :url="player.avatar_url" :letters="1" />
                    <span class="player-token__online" :class="player.online === false ? 'is-off' : 'is-on'"></span>
                    <span v-if="player.id === playerId" class="player-token__camera">
                      <i class="fa-solid fa-camera"></i>
                    </span>
                  </div>
                  <div class="player-token__name">{{ player.pseudo }}</div>
                </div>
              </div>
            </div>

            <!-- Mode de jeu -->
            <div v-if="gameStatus === 'waiting'" class="game-mode-panel">
              <h5 class="roster__title">Mode de jeu</h5>

              <template v-if="isHost">
                <div class="mode-toggle">
                  <button
                      type="button"
                      class="mode-toggle__btn"
                      :class="{ 'is-active': gameMode === 'random' }"
                      @click="gameMode = 'random'; handleUpdateSettings()"
                  >
                    <i class="fa-solid fa-shuffle"></i> Aléatoire
                  </button>
                  <button
                      type="button"
                      class="mode-toggle__btn"
                      :class="{ 'is-active': gameMode === 'cosmos' }"
                      @click="gameMode = 'cosmos'; handleUpdateSettings()"
                  >
                    <i class="fa-solid fa-globe"></i> Cosmos imposé
                  </button>
                </div>

                <div v-if="gameMode === 'cosmos'" class="mode-cosmos">
                  <select
                      v-model="cosmosId"
                      class="form-select mode-cosmos__select"
                      @change="handleUpdateSettings"
                  >
                    <option :value="null" disabled>— Choisir un cosmos —</option>
                    <option
                        v-for="c in cosmosOptions"
                        :key="c.id"
                        :value="c.id"
                        :disabled="c.playable_universe_count < pairsNeeded"
                    >
                      {{ c.name }} — {{ c.playable_universe_count }} binôme(s) possible(s)
                    </option>
                  </select>
                  <p class="mode-cosmos__hint">
                    Il faut au moins {{ pairsNeeded }} univers jouables dans le cosmos
                    ({{ players.length }} joueur{{ players.length > 1 ? 's' : '' }}).
                  </p>
                </div>
              </template>

              <p v-else class="mode-readonly">
                <i :class="gameMode === 'cosmos' ? 'fa-solid fa-globe' : 'fa-solid fa-shuffle'"></i>
                {{ gameMode === 'cosmos'
                    ? `Cosmos imposé${selectedCosmos ? ' — ' + selectedCosmos.name : ''}`
                    : 'Univers aléatoires' }}
              </p>

              <!-- Joueurs éliminés : de quoi les occuper jusqu'à la fin -->
              <h5 class="roster__title eliminated-options__title">Joueurs éliminés</h5>
              <div v-if="isHost" class="mode-toggle eliminated-options">
                <button
                    type="button"
                    class="mode-toggle__btn"
                    :class="{ 'is-active': revealToEliminated }"
                    :aria-pressed="revealToEliminated"
                    @click="revealToEliminated = !revealToEliminated; handleUpdateSettings()"
                >
                  <i class="fa-solid fa-eye"></i> Vue omnisciente
                </button>
                <button
                    type="button"
                    class="mode-toggle__btn"
                    :class="{ 'is-active': bestQuestionVote }"
                    :aria-pressed="bestQuestionVote"
                    @click="bestQuestionVote = !bestQuestionVote; handleUpdateSettings()"
                >
                  <i class="fa-solid fa-star"></i> Vote meilleure question
                </button>
              </div>
              <p class="mode-readonly eliminated-options__hint">
                <template v-if="revealToEliminated">👁️ Un éliminé voit tous les personnages.</template>
                <template v-if="revealToEliminated && bestQuestionVote"><br></template>
                <template v-if="bestQuestionVote">⭐ Les éliminés votent pour la meilleure question de chaque round : +1 point pour son auteur.</template>
                <template v-if="!revealToEliminated && !bestQuestionVote">Les éliminés regardent la fin de la partie.</template>
              </p>
            </div>

            <div class="btn-actions-row mb-3">
              <button
                  v-if="gameStatus === 'waiting'"
                  type="button"
                  class="stamp-toggle stamp-toggle--icon-only"
                  :class="{ 'is-active': isCurrentPlayerReady }"
                  :aria-label="isCurrentPlayerReady ? 'Je ne suis plus prêt' : 'Je suis prêt !'"
                  :title="isCurrentPlayerReady ? 'Je ne suis plus prêt' : 'Je suis prêt !'"
                  @click="handleReady"
              >
                <i :class="isCurrentPlayerReady
                  ? 'fa-solid fa-circle-xmark'
                  : 'fa-solid fa-circle-check'">
                </i>
              </button>

              <button
                  type="button"
                  class="cabinet-btn cabinet-btn--danger cabinet-btn--sm cabinet-btn--icon-only"
                  aria-label="Quitter le salon"
                  title="Quitter le salon"
                  @click="showLeaveModal = true"
              >
                <i class="fa-solid fa-right-from-bracket"></i>
              </button>
            </div>

            <BModal v-model="showLeaveModal" title="Quitter le salon" no-footer class="arcade-modal">
              <p class="text-center">Es-tu sûr de vouloir quitter le salon ?</p>
              <p class="text-center text-muted small">Tu devras rejoindre avec le code pour revenir.</p>
              <div class="text-center mt-3 btn-modal">
                <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" @click="handleLeaveRoom">
                  <i class="fa-solid fa-right-from-bracket"></i> Quitter
                </button>
                <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showLeaveModal = false">
                  Annuler
                </button>
              </div>
            </BModal>

            <!-- Bouton démarrer (hôte uniquement) -->
            <div class="text-center">
              <button
                  v-if="isHost && gameStatus === 'waiting' && !startingGame"
                  type="button"
                  class="play-btn"
                  :disabled="players.length < 2 || !!startBlockedReason"
                  @click="handleStartGame"
              >
                <i class="fa-solid fa-play"></i> Démarrer la partie
              </button>

              <p v-if="isHost && gameStatus === 'waiting' && startBlockedReason" class="start-blocked">
                <i class="fa-solid fa-circle-info"></i> {{ startBlockedReason }}
              </p>

              <BAlert
                  v-if="!isHost && gameStarting"
                  :model-value="true"
                  variant="warning"
                  class="text-center mb-0"
              >
                <BSpinner small class="me-2" />
                La partie démarre …
              </BAlert>
            </div>
          </div>
        </BCol>
      </BRow>

      <AvatarEditor
          v-model="showAvatarEditor"
          :player="me"
          @updated="url => { setAvatar(playerId, url); updateProfilePlayer({ avatar_url: url }) }"
      />

      <PasswordPromptModal v-model="showPasswordPrompt" />

      <!-- Chat commun : déplié par défaut hors salon, replié dès qu'on en rejoint un -->
      <LobbyChat :player-id="playerId" :is-in-room="!!gameCode" />

      <!-- Modal créer -->
      <BModal v-model="showCreateModal" title="Créer une partie" no-footer class="arcade-modal">
        <!-- L'alerte de la page est masquée par la modale : on la répète ici -->
        <div v-if="error" class="arcade-alert mb-3" role="alert">
          <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
          <router-link v-if="protectedPseudo" :to="{ name: 'Login', query: { pseudo: protectedPseudo } }" class="login-link">
            Se connecter
          </router-link>
        </div>
        <div class="mb-3">
          <label class="form-label">Votre nom :</label>
          <BFormInput
              v-model="playerName"
              placeholder="Entrez votre nom"
              :readonly="isLoggedIn"
              @keyup.enter="handleCreateGame"
          />
          <p v-if="!isLoggedIn" class="pseudo-hint">
            Déjà un profil protégé ? <router-link :to="{ name: 'Login' }">Connecte-toi</router-link>
          </p>
        </div>
        <div class="text-center">
          <button type="button" class="cabinet-btn cabinet-btn--sm" @click="handleCreateGame">Créer</button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showCreateModal = false">Annuler</button>
        </div>
      </BModal>

      <!-- Modal rejoindre -->
      <BModal v-model="showJoinModal" title="Rejoindre une partie" no-footer class="arcade-modal">
        <!-- L'alerte de la page est masquée par la modale : on la répète ici -->
        <div v-if="error" class="arcade-alert mb-3" role="alert">
          <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
          <router-link v-if="protectedPseudo" :to="{ name: 'Login', query: { pseudo: protectedPseudo } }" class="login-link">
            Se connecter
          </router-link>
        </div>
        <div class="mb-3">
          <label class="form-label">Votre nom :</label>
          <BFormInput
              v-model="playerName"
              placeholder="Entrez votre nom"
              :readonly="isLoggedIn"
          />
          <p v-if="isLoggedIn" class="pseudo-hint">Pseudo de ton profil.</p>
          <p v-else class="pseudo-hint">
            Déjà un profil protégé ? <router-link :to="{ name: 'Login' }">Connecte-toi</router-link>
          </p>
        </div>
        <div class="mb-3">
          <label class="form-label">Code de la partie :</label>
          <BFormInput
              v-model="codeToJoin"
              placeholder="Entrez le code"
              @keyup.enter="handleJoinGame"
          />
        </div>
        <div class="text-center">
          <button type="button" class="cabinet-btn cabinet-btn--sm" @click="handleJoinGame">Rejoindre</button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showJoinModal = false">Annuler</button>
        </div>
      </BModal>

      <!-- Modal rejoindre partie en cours -->
      <BModal v-model="showRejoinModal" title="Partie en cours" no-footer no-close-on-backdrop no-close-on-esc class="arcade-modal">
        <div class="text-center py-2">
          <p class="fs-5 mb-1">Tu as une partie en cours !</p>
          <p class="text-muted small mb-4">
            Code : <strong>{{ gameCode }}</strong>
          </p>
          <div class="d-flex justify-content-center gap-3">
            <button type="button" class="play-btn play-btn--sm" @click="handleRejoinGame">
              <i class="fa-solid fa-play me-2"></i> Reprendre la partie
            </button>
            <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" @click="handleAbandonGame">
              <i class="fa-solid fa-trash me-2"></i> Abandonner
            </button>
          </div>
        </div>
      </BModal>
    </BContainer>
  </div>
</template>

<style scoped>
.pseudo-hint {
  margin: 0.35rem 0 0;
  font-size: 0.85rem;
  opacity: 0.8;
}

.login-link {
  margin-left: 0.4rem;
  font-weight: 700;
  color: inherit;
  text-decoration: underline;
}

.arcade-page {
  font-family: 'Baloo 2', system-ui, sans-serif;
  min-height: 100vh;
  padding-top: 1.5rem;
  padding-bottom: 2rem;
}

/* ── Titre enseigne ────────────────────────────────────────────── */
.arcade-title-wrap {
  text-align: center;
  margin: 2rem 0 2.5rem;
}

.arcade-title {
  /* Fluide : Press Start 2P n'a pas de point de coupure dans un mot,
     une taille fixe déborde du viewport sous ~360px. */
  font-size: clamp(1.25rem, 6.5vw, 1.9rem);
  margin-top: 0.75rem;
}

/* ── Boutons "borne d'arcade" ──────────────────────────────────── */
.cabinet-buttons {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 1.25rem;
  margin-bottom: 2.5rem;
}

/* ── QR code & spectateur ─────────────────────────────────────── */
.qr-toggle-wrap {
  display: flex;
  justify-content: center;
  margin: -0.5rem 0 1rem;
}

.qr-toggle {
  min-height: 44px;
  padding: 0 0.9rem;
  font-family: 'Baloo 2', sans-serif;
  font-weight: 700;
  font-size: 0.85rem;
  color: var(--arcade-blue-grey-dark);
  background: transparent;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 8px;
}

.spectate-banner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.6rem;
  margin-bottom: 1.25rem;
  padding: 0.75rem;
  text-align: center;
  background: rgba(224, 163, 28, 0.15);
  border: 2px solid var(--arcade-gold);
  border-radius: 8px;
}

.spectate-banner__text {
  margin: 0;
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--arcade-blue-grey-dark);
}

/* ── Plateau / carte de salon ─────────────────────────────────── */
.board-card {
  margin-bottom: 1.5rem;
}

.lcd-screen {
  margin-bottom: 1.25rem;
}

/* Le code devient un bouton « copier » sans perdre le look écran LCD */
.lcd-code--copy {
  cursor: pointer;
  font: inherit;
  transition: box-shadow 0.15s ease, transform 0.1s ease;
}

.lcd-code--copy:hover {
  box-shadow: inset 0 2px 6px rgba(0, 0, 0, 0.6), 0 0 10px rgba(124, 255, 158, 0.45);
}

.lcd-code--copy:active {
  transform: translateY(1px);
}

.lcd-code--copy.is-copied {
  border-color: #7CFF9E;
  box-shadow: inset 0 2px 6px rgba(0, 0, 0, 0.6), 0 0 14px rgba(124, 255, 158, 0.7);
}

.lcd-copy-hint {
  font-family: 'Baloo 2', sans-serif;
  font-size: 0.7rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: var(--arcade-taupe);
}

.lcd-code--copy.is-copied + .lcd-copy-hint {
  color: #4caf7d;
}

.status-pills {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.6rem;
  margin-bottom: 1.5rem;
}

.roster__title {
  text-align: center;
  color: var(--arcade-blue-grey-dark);
  font-weight: 800;
  text-transform: uppercase;
  letter-spacing: 1px;
  font-size: 0.85rem;
  margin-bottom: 0.9rem;
}

.roster__hint {
  margin: -0.4rem 0 0.75rem;
  text-align: center;
  font-size: 0.8rem;
  color: var(--arcade-blue-grey-dark);
}

/* Mon jeton : ouvre l'éditeur de photo */
.player-token.is-mine {
  cursor: pointer;
}

.player-token.is-mine:focus-visible {
  outline: 3px solid var(--arcade-gold);
  outline-offset: 2px;
}

.player-token__camera {
  position: absolute;
  bottom: -2px;
  left: -2px;
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  font-size: 0.55rem;
  font-family: system-ui, sans-serif;
  color: #4a2f00;
  background: var(--arcade-gold);
  border: 2px solid #fff;
  border-radius: 50%;
}

.roster__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
  gap: 0.75rem;
  margin-bottom: 1.5rem;
}

.btn-actions-row {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 0.75rem;
}

.stamp-toggle--icon-only,
.cabinet-btn--icon-only {
  width: 3rem;
  height: 3rem;
  padding: 0;
  margin: 0;
  font-size: 1.2rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.btn-modal {
  display: flex;
  justify-content: space-evenly;
  flex-direction: row-reverse;
}

/* ── Mode de jeu ──────────────────────────────────────────────── */
.game-mode-panel {
  margin-bottom: 1.25rem;
}

.mode-toggle {
  display: flex;
  gap: 0.5rem;
  justify-content: center;
  flex-wrap: wrap;
  margin-bottom: 0.75rem;
}

.mode-toggle__btn {
  flex: 1 1 140px;
  padding: 0.5rem 0.75rem;
  border: 2px solid var(--arcade-blue-grey-dark, #37474f);
  border-radius: 10px;
  background: transparent;
  font-weight: 700;
  font-size: 0.85rem;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.mode-toggle__btn.is-active {
  background: var(--arcade-blue-grey-dark, #37474f);
  color: #fff;
}

.eliminated-options__title {
  margin-top: 1rem;
}

.eliminated-options {
  margin-bottom: 0.4rem;
}

.mode-cosmos__select {
  margin-bottom: 0.4rem;
}

.mode-cosmos__hint,
.mode-readonly {
  text-align: center;
  font-size: 0.8rem;
  color: var(--arcade-blue-grey-dark, #37474f);
  margin: 0;
}

.pill--orphan {
  border-color: var(--arcade-blue-grey);
  color: var(--arcade-blue-grey-dark);
}

.orphan-hint {
  margin: 0.6rem 0 0;
  font-size: 0.8rem;
  color: var(--arcade-blue-grey-dark);
  text-align: center;
}

.start-blocked {
  margin-top: 0.6rem;
  font-size: 0.85rem;
  color: #c62828;
  text-align: center;
}
</style>
