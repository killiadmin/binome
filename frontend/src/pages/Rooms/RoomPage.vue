<script setup>
import {ref, onMounted, onUnmounted, computed} from 'vue'
import {useRouter} from 'vue-router'
import {roomService} from '../../services/roomService'
import {useReverb} from '../../sockets/useReverb'
import {BButton, BCard, BContainer, BRow, BCol, BModal, BFormInput} from 'bootstrap-vue-next'
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

// ─── SESSION localStorage ─────────────────────────────────────────────────────

function saveSession() {
  localStorage.setItem('session', JSON.stringify({
    roomId: roomId.value,
    gameCode: gameCode.value,
    playerId: playerId.value,
    hostId: hostId.value,
    isHost: isHost.value,
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
    const res = await roomService.get(session.roomId)

    roomId.value = session.roomId
    gameCode.value = session.gameCode
    playerId.value = session.playerId
    hostId.value = session.hostId
    isHost.value = session.isHost
    players.value = res.data.room.players

    resetEcho()
    initLobby(session.roomId)

  } catch {
    clearSession()
  }
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
    onGameStarted: (data) => {
      gameStatus.value = 'in_progress'
      gameId.value = data.game_id
      saveSession()
      router.push({name: 'RoundPage', params: {gameId: data.game_id}})
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
    isHost.value = false

    saveSession()
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

  try {
    const res = await roomService.start(roomId.value, playerId.value)
    gameId.value = res.data.game_id
    saveSession()
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors du démarrage.'
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
  // Tente de restaurer une session existante au refresh
  restoreSession()
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
  <BContainer>
    <h1 class="text-center m-5 color-beige">Salons de jeu</h1>

    <div v-if="error" class="alert alert-danger text-center">
      {{ error }}
    </div>

    <!-- Boutons principaux -->
    <BRow class="justify-content-center mb-4">
      <BCol>
        <BButton @click="showCreateModal = true" class="bg-color-blue-grey border m-2">
          <i class="fa-solid fa-plus"></i> Créer une partie
        </BButton>
        <BButton @click="showJoinModal = true" class="bg-color-blue-grey border m-2">
          <i class="fa-solid fa-right-to-bracket"></i> Rejoindre une partie
        </BButton>
      </BCol>
    </BRow>

    <!-- Carte du salon actif -->
    <BRow v-if="gameCode" class="justify-content-center" :style="{ paddingBottom: '100px' }">
      <BCol cols="12" md="6" :style="{ minWidth: '400px' }">
        <BCard class="bg-color-beige border shadow mb-4">
          <h2 class="text-center">Votre partie</h2>
          <p class="text-center fw-bold fs-4">Code : {{ gameCode }}</p>
          <p class="text-center">Joueurs : {{ players.length }}</p>
          <p class="text-center fw-bold" :class="getGameStatusClass(gameStatus)">
            {{ getGameStatusText(gameStatus) }}
          </p>

          <!-- Liste des joueurs -->
          <div class="mb-3">
            <h5>Participants :</h5>
            <ul class="list-unstyled">
              <li v-for="player in players" :key="player.id" class="mb-1">
                <span class="me-2" :class="player.online === false ? 'text-muted' : 'text-success'">●</span>
                {{ player.pseudo }}
                <span v-if="player.id === hostId" class="badge bg-warning text-dark ms-2">Hôte</span>
                <span v-if="player.is_ready" class="badge bg-success ms-2">Prêt</span>
              </li>
            </ul>
          </div>

          <div v-if="gameStatus === 'waiting'" class="text-center mb-3">
            <BButton
                @click="handleReady"
                :variant="isCurrentPlayerReady ? 'success' : 'outline-success'"
                class="border"
            >
              <i :class="isCurrentPlayerReady
            ? 'fa-solid fa-circle-xmark'
            : 'fa-solid fa-circle-check'">
              </i>
              {{ isCurrentPlayerReady ? 'Je ne suis plus prêt' : 'Je suis prêt ✓' }}
            </BButton>
          </div>

          <div class="text-center mt-3">
            <BButton variant="danger" class="border" @click="showLeaveModal = true">
              <i class="fa-solid fa-right-from-bracket"></i> Quitter le salon
            </BButton>
          </div>

          <BModal v-model="showLeaveModal" title="Quitter le salon" hide-footer>
            <p class="text-center">Es-tu sûr de vouloir quitter le salon ?</p>
            <p class="text-center text-muted small">Tu devras rejoindre avec le code pour revenir.</p>
            <div class="text-center mt-3">
              <BButton variant="danger" @click="handleLeaveRoom" class="m-2">
                <i class="fa-solid fa-right-from-bracket"></i> Quitter
              </BButton>
              <BButton variant="secondary" @click="showLeaveModal = false" class="m-2">
                Annuler
              </BButton>
            </div>
          </BModal>

          <!-- Bouton démarrer (hôte uniquement) -->
          <div class="text-center">
            <BButton
                v-if="isHost && gameStatus === 'waiting'"
                @click="handleStartGame"
                class="bg-color-blue-grey border"
                :disabled="players.length < 2"
            >
              Démarrer la partie
            </BButton>
          </div>
        </BCard>
      </BCol>
    </BRow>

    <!-- Modal créer -->
    <BModal v-model="showCreateModal" title="Créer une partie" hide-footer>
      <div class="mb-3">
        <label class="form-label">Votre nom :</label>
        <BFormInput
            v-model="playerName"
            placeholder="Entrez votre nom"
            @keyup.enter="handleCreateGame"
        />
      </div>
      <div class="text-center">
        <BButton @click="handleCreateGame" class="bg-color-blue-grey border m-2">Créer</BButton>
        <BButton variant="secondary" @click="showCreateModal = false" class="m-2">Annuler</BButton>
      </div>
    </BModal>

    <!-- Modal rejoindre -->
    <BModal v-model="showJoinModal" title="Rejoindre une partie" hide-footer>
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
        <BButton @click="handleJoinGame" class="bg-color-blue-grey border m-2">Rejoindre</BButton>
        <BButton variant="secondary" @click="showJoinModal = false" class="m-2">Annuler</BButton>
      </div>
    </BModal>
  </BContainer>
</template>

<style scoped></style>