<script setup>
import {ref, onMounted, onUnmounted, computed} from 'vue'
import {useRouter} from 'vue-router'
import {roomService} from '../../services/roomService'
import {characterService} from '../../services/characterService'
import {useReverb} from '../../sockets/useReverb'
import {BButton, BCard, BContainer, BRow, BCol, BModal, BFormInput, BAlert, BSpinner, BBadge} from 'bootstrap-vue-next'
import {resetEcho} from "../../sockets/useReverb.js";

const router = useRouter()

// ─── STATE ────────────────────────────────────────────────────────────────────

const playerName = ref('')
const showCreateModal = ref(false)
const showJoinModal = ref(false)
const codeToJoin = ref('')
const error = ref(null)

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

// ─── MODE DE JEU ──────────────────────────────────────────────────────────────
const gameMode = ref('random')          // 'random' | 'cosmos'
const cosmosId = ref(null)
const cosmosOptions = ref([])            // [{ id, name, playable_universe_count }]
const savingSettings = ref(false)

const pairsNeeded = computed(() => Math.max(1, Math.ceil(players.value.length / 2)))

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
  const raw = localStorage.getItem('session')
  if (!raw) return

  try {
    const session = JSON.parse(raw)

    if (session.gameId) {
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
    onRoomSettingsUpdated: (data) => {
      gameMode.value = data.game_mode ?? 'random'
      cosmosId.value = data.cosmos_id ?? null
    },
    onGameStarted: (data) => {
      console.log('[GameStarted] data reçu :', data)
      gameStatus.value = 'in_progress'
      gameId.value = data.game_id
      saveSession()
      router.push({ name: 'RoundPage', params: { gameId: data.game_id } })
    },
    onError: () => {
      error.value = 'Connexion WebSocket perdue. Recharge la page.'
    },
    onPlayerLeft: (data) => {
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
  if (!playerName.value.trim()) return
  error.value = null

  try {
    const res = await roomService.create(playerName.value)

    roomId.value = res.data.room.id
    gameCode.value = res.data.room.code
    playerId.value = res.data.player.id
    hostId.value = res.data.player.id
    isHost.value = true
    players.value = [{id: res.data.player.id, pseudo: res.data.player.pseudo, is_ready: false}]

    saveSession()
    resetEcho()
    initLobby(roomId.value)

    showCreateModal.value = false
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors de la création du salon.'
  }
}

const handleJoinGame = async () => {
  if (!playerName.value.trim() || !codeToJoin.value.trim()) return
  error.value = null

  try {
    const res = await roomService.join(codeToJoin.value, playerName.value)

    roomId.value = res.data.room.id
    gameCode.value = res.data.room.code
    playerId.value = res.data.player.id
    players.value = res.data.room.players
    gameMode.value = res.data.room.game_mode ?? 'random'
    cosmosId.value = res.data.room.cosmos_id ?? null
    isHost.value = false

    saveSession()
    resetEcho()
    initLobby(roomId.value)

    showJoinModal.value = false
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors de la connexion au salon.'
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

const handleLeaveRoom = async () => {
  try {
    await roomService.leave(roomId.value, playerId.value)
  } catch (e) {
    console.error('Erreur leave:', e)
  } finally {
    const {leaveRoom} = useReverb(playerId.value)
    leaveRoom(roomId.value)
    resetEcho()
    clearSession()
    roomId.value = null
    gameCode.value = null
    playerId.value = null
    players.value = []
    isHost.value = false
    hostId.value = null
    showLeaveModal.value = false
  }
}

const isCurrentPlayerReady = computed(() => {
  const me = players.value.find(p => p.id === playerId.value)
  return me?.is_ready ?? false
})

// ─── LIFECYCLE ────────────────────────────────────────────────────────────────

onMounted(() => {
  restoreSession()
  loadCosmosOptions()
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

      <div v-if="error" class="arcade-alert">
        <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
      </div>

      <!-- Boutons principaux -->
      <div class="cabinet-buttons">
        <button class="cabinet-btn stacked" type="button" @click="showCreateModal = true">
          <span class="cabinet-btn__icon"><i class="fa-solid fa-plus"></i></span>
          <span class="cabinet-btn__label">Créer une partie</span>
        </button>
        <button class="cabinet-btn stacked" type="button" @click="showJoinModal = true">
          <span class="cabinet-btn__icon"><i class="fa-solid fa-right-to-bracket"></i></span>
          <span class="cabinet-btn__label">Rejoindre une partie</span>
        </button>
      </div>

      <!-- Carte du salon actif -->
      <BRow v-if="gameCode" class="justify-content-center" :style="{ paddingBottom: '100px' }">
        <BCol cols="12" md="7" lg="6" :style="{ minWidth: '360px' }">
          <div class="board-card">
            <div class="board-card__rivet board-card__rivet--tl"></div>
            <div class="board-card__rivet board-card__rivet--tr"></div>
            <div class="board-card__rivet board-card__rivet--bl"></div>
            <div class="board-card__rivet board-card__rivet--br"></div>

            <h2 class="board-card__title">Votre partie</h2>

            <div class="lcd-screen">
              <span class="lcd-label">Code</span>
              <div class="lcd-code">
                <span v-for="(char, i) in gameCode.split('')" :key="i" class="lcd-char">{{ char }}</span>
              </div>
            </div>

            <div class="status-pills">
              <span class="pill">
                <i class="fa-solid fa-users"></i> {{ players.length }} joueur{{ players.length > 1 ? 's' : '' }}
              </span>
              <span class="pill" :class="gameStatus === 'in_progress' ? 'pill--danger' : 'pill--success'">
                <span class="pill-dot"></span>
                {{ getGameStatusText(gameStatus) }}
              </span>
            </div>

            <!-- Liste des joueurs -->
            <div class="roster">
              <h5 class="roster__title">Participants</h5>
              <div class="roster__grid">
                <div
                    v-for="player in players"
                    :key="player.id"
                    class="player-token"
                    :class="{ 'is-offline': player.online === false, 'is-ready': player.is_ready }"
                >
                  <span v-if="player.id === hostId" class="player-token__crown">
                    <i class="fa-solid fa-crown"></i>
                  </span>
                  <span v-if="player.is_ready" class="player-token__stamp">
                    <i class="fa-solid fa-check"></i>
                  </span>
                  <div class="player-token__avatar">
                    {{ player.pseudo?.charAt(0).toUpperCase() }}
                    <span class="player-token__online" :class="player.online === false ? 'is-off' : 'is-on'"></span>
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

      <!-- Modal créer -->
      <BModal v-model="showCreateModal" title="Créer une partie" no-footer class="arcade-modal">
        <div class="mb-3">
          <label class="form-label">Votre nom :</label>
          <BFormInput
              v-model="playerName"
              placeholder="Entrez votre nom"
              @keyup.enter="handleCreateGame"
          />
        </div>
        <div class="text-center">
          <button type="button" class="cabinet-btn cabinet-btn--sm" @click="handleCreateGame">Créer</button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showCreateModal = false">Annuler</button>
        </div>
      </BModal>

      <!-- Modal rejoindre -->
      <BModal v-model="showJoinModal" title="Rejoindre une partie" no-footer class="arcade-modal">
        <div class="mb-3">
          <label class="form-label">Votre nom :</label>
          <BFormInput
              v-model="playerName"
              placeholder="Entrez votre nom"
          />
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
  font-size: 1.9rem;
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

/* ── Plateau / carte de salon ─────────────────────────────────── */
.board-card {
  margin-bottom: 1.5rem;
}

.lcd-screen {
  margin-bottom: 1.25rem;
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

.start-blocked {
  margin-top: 0.6rem;
  font-size: 0.85rem;
  color: #c62828;
  text-align: center;
}
</style>
