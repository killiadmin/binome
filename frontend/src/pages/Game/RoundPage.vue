<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { gameService } from '../../services/gameService'
import { useReverb, resetEcho } from '../../sockets/useReverb.js'
import {
  BButton, BContainer, BModal, BFormInput,
  BFormSelect, BAlert, BBadge, BSpinner
} from 'bootstrap-vue-next'

const route  = useRoute()
const router = useRouter()

// ─── SESSION ──────────────────────────────────────────────────────────────────

const session    = JSON.parse(localStorage.getItem('session') ?? '{}')
const gameId     = route.params.gameId ?? session.gameId
const myPlayerId = ref(session.playerId ?? null)

// ─── STATE ────────────────────────────────────────────────────────────────────

const loading    = ref(true)
const submitting = ref(false)
const error      = ref(null)

const myCharacter         = ref(null)
const players             = ref([])
const currentRound        = ref(null)
const currentPlayerId     = ref(null)
const hasPlayed           = ref(false)
const lastAction          = ref(null)
const availableCharacters = ref([])
const discoveredBinomes   = ref([])

const questionTarget = ref('')
const questionText   = ref('')

const accusationTarget    = ref('')
const accusationCharacter = ref('')

const showQuestionModal   = ref(false)
const showAccusationModal = ref(false)

const binomeNotif = ref(null)

const gameEnded      = ref(false)
const winners        = ref([])
const gameOverTitle  = ref('')
const gameOverMsg    = ref('')

const isBlurred = ref(true)

// ─── COMPUTED ─────────────────────────────────────────────────────────────────

const isMyTurn = computed(() => currentPlayerId.value === myPlayerId.value)

const currentPlayerName = computed(() => {
  const p = players.value.find(p => p.id === currentPlayerId.value)
  return p?.pseudo ?? '…'
})

const otherPlayers = computed(() =>
    players.value.filter(p => p.id !== myPlayerId.value)
)

const discoveredPlayerIds = computed(() => {
  const ids = new Set()
  discoveredBinomes.value.forEach(b => {
    if (b.player1_id) ids.add(b.player1_id)
    if (b.player2_id) ids.add(b.player2_id)
  })
  return ids
})

// ─── INIT ─────────────────────────────────────────────────────────────────────

onMounted(async () => {
  if (!gameId || !myPlayerId.value) {
    router.push({ name: 'Home' })
    return
  }

  try {
    const game = await gameService.show(gameId)
    players.value = game.binomes.flatMap(b => b.players)
    currentRound.value    = game.current_round ?? null
    currentPlayerId.value = game.current_round?.current_player_id ?? null
    availableCharacters.value = game.characters ?? []
    discoveredBinomes.value = game.binomes
        .filter(b => b.is_discovered)
        .map(b => ({
          player1_id: b.players[0]?.id,
          player2_id: b.players[1]?.id,
        }))

    const me = await gameService.myCharacter(gameId, myPlayerId.value)
    console.log('[MyCharacter] données reçues :', JSON.stringify(me))
    myCharacter.value = me
  } catch (e) {
    error.value = 'Impossible de charger la partie.'
  } finally {
    loading.value = false
  }

  resetEcho()
  const { joinGame } = useReverb(myPlayerId.value)
  joinGame(gameId, {
    onRoundStarted:     handleRoundStarted,
    onActionPlayed:     handleActionPlayed,
    onBinomeDiscovered: handleBinomeDiscovered,
    onGameEnded:        handleGameEnded,
    onError: () => { error.value = 'Connexion WebSocket perdue.' },
  })
})

onUnmounted(() => {
  const { leaveGame } = useReverb(myPlayerId.value)
  leaveGame(gameId)
})

// ─── WEBSOCKET HANDLERS ───────────────────────────────────────────────────────

function handleRoundStarted(data) {
  currentRound.value    = data.round
  currentPlayerId.value = data.round.current_player_id
  hasPlayed.value       = false
  lastAction.value      = null
  resetForms()
}

function handleActionPlayed(data) {
  lastAction.value = data
  if (data.player_id === myPlayerId.value) hasPlayed.value = true
}

function handleBinomeDiscovered(data) {
  discoveredBinomes.value.push(data.binome)
  binomeNotif.value = {
    player1:    data.binome.player1_pseudo,
    character1: data.binome.player1_character,
    player2:    data.binome.player2_pseudo,
    character2: data.binome.player2_character,
  }
  setTimeout(() => { binomeNotif.value = null }, 6000)
}

function handleGameEnded(data) {
  gameEnded.value = true
  winners.value   = data.winning_binome?.players ?? []
  const iWon = winners.value.some(w => w.id === myPlayerId.value)
  gameOverTitle.value = iWon ? '🏆 Victoire !' : '💀 Défaite'
  gameOverMsg.value   = iWon
      ? "Votre binôme n'a jamais été découvert. Bien joué !"
      : 'Votre binôme a été découvert. Meilleure chance la prochaine fois !'
}

// ─── ACTIONS ──────────────────────────────────────────────────────────────────

async function submitQuestion() {
  if (!questionTarget.value || !questionText.value.trim() || submitting.value) return
  submitting.value = true
  error.value = null
  try {
    await gameService.playQuestion(gameId, currentRound.value.id, myPlayerId.value, questionText.value.trim())
    showQuestionModal.value = false
    resetForms()
  } catch (e) {
    error.value = e.response?.data?.message || "Erreur lors de l'envoi de la question."
  } finally {
    submitting.value = false
  }
}

async function submitAccusation() {
  if (!accusationTarget.value || !accusationCharacter.value || submitting.value) return
  submitting.value = true
  error.value = null
  try {
    await gameService.playAccusation(gameId, currentRound.value.id, myPlayerId.value, accusationTarget.value, accusationCharacter.value)
    showAccusationModal.value = false
    resetForms()
  } catch (e) {
    error.value = e.response?.data?.message || "Erreur lors de l'accusation."
  } finally {
    submitting.value = false
  }
}

function resetForms() {
  questionTarget.value      = ''
  questionText.value        = ''
  accusationTarget.value    = ''
  accusationCharacter.value = ''
}

function backToHome() {
  localStorage.removeItem('session')
  router.push({ name: 'Home' })
}
</script>

<template>
  <div class="round-page mt-5">

    <!-- Loading -->
    <div v-if="loading" class="loading-screen">
      <div class="loading-orb"></div>
      <p class="loading-text">Chargement de la partie…</p>
    </div>

    <template v-else>

      <!-- Erreur globale -->
      <div v-if="error" class="error-banner">{{ error }}</div>

      <!-- ── HEADER : tour en cours ──────────────────────────────────────── -->
      <div class="turn-banner" :class="isMyTurn ? 'turn-mine' : 'turn-other'">
        <div class="turn-inner">
          <span class="turn-icon">{{ isMyTurn ? '⚡' : '⏳' }}</span>
          <div class="turn-text">
            <span class="turn-main">
              {{ isMyTurn ? 'Ton tour !' : `Tour de ${currentPlayerName}` }}
            </span>
            <span class="turn-sub">Round {{ currentRound?.number ?? '—' }}</span>
          </div>
        </div>
      </div>

      <!-- ── CARTE PERSONNAGE ────────────────────────────────────────────── -->
      <div class="character-card">
        <div class="character-card-glow"></div>

        <!-- Bouton flou -->
        <button class="blur-toggle" @click="isBlurred = !isBlurred">
          {{ isBlurred ? '👁 Révéler' : '🙈 Masquer' }}
        </button>

        <!-- Image du personnage -->
        <div class="character-image-wrap" :class="{ blurred: isBlurred }">
          <img
              v-if="myCharacter?.image_url"
              :src="myCharacter.image_url"
              :alt="myCharacter?.name"
              class="character-image"
          />
          <div v-else class="character-image-placeholder">
            <span>?</span>
          </div>
          <div class="character-universe-badge">{{ myCharacter?.universe ?? '…' }}</div>
        </div>

        <!-- Infos personnage -->
        <div class="character-info">
          <p class="character-label" :class="{ blurred: isBlurred }">Ton personnage</p>
          <h1 class="character-name" :class="{ blurred: isBlurred }">
            {{ myCharacter?.name ?? '???' }}
          </h1>

          <!-- Mots interdits -->
          <div class="forbidden-section">
            <p class="forbidden-title" :class="{ blurred: isBlurred }">☠ Mots interdits</p>
            <div class="forbidden-words" :class="{ blurred: isBlurred }">
        <span
            v-for="word in myCharacter?.forbidden_words ?? []"
            :key="word"
            class="forbidden-word"
        >
          {{ word }}
        </span>
            </div>
          </div>
        </div>
      </div>

      <!-- ── NOTIFICATION BINÔME DÉCOUVERT ─────────────────────────────── -->
      <div v-if="binomeNotif" class="binome-notif">
        <span class="binome-notif-icon">🔍</span>
        <div>
          <p class="binome-notif-title">Binôme découvert !</p>
          <p class="binome-notif-sub">
            {{ binomeNotif.player1 }} ({{ binomeNotif.character1 }})
            &amp; {{ binomeNotif.player2 }} ({{ binomeNotif.character2 }})
          </p>
        </div>
      </div>

      <!-- ── DERNIÈRE ACTION ────────────────────────────────────────────── -->
      <div v-if="lastAction" class="last-action" :class="`last-action-${lastAction.type === 'question' && !lastAction.is_valid ? 'danger' : lastAction.type === 'accusation' && lastAction.accusation_correct ? 'success' : lastAction.type === 'accusation' ? 'danger' : 'info'}`">
        <span v-if="lastAction.type === 'question' && !lastAction.is_valid">
          ❌ {{ players.find(p => p.id === lastAction.player_id)?.pseudo }} — mot interdit, tour perdu.
        </span>
        <span v-else-if="lastAction.type === 'question'">
          💬 {{ players.find(p => p.id === lastAction.player_id)?.pseudo }} a posé une question.
        </span>
        <span v-else-if="lastAction.accusation_correct">
          🎯 {{ players.find(p => p.id === lastAction.player_id)?.pseudo }} — accusation correcte !
        </span>
        <span v-else>
          ❌ {{ players.find(p => p.id === lastAction.player_id)?.pseudo }} — mauvaise accusation.
        </span>
      </div>

      <!-- ── ACTIONS (mon tour) ─────────────────────────────────────────── -->
      <div class="actions-section">
        <template v-if="isMyTurn && !hasPlayed">
          <button class="action-btn action-btn-question" @click="showQuestionModal = true">
            <span class="action-btn-icon">💬</span>
            <span class="action-btn-label">Poser une question</span>
          </button>
          <button class="action-btn action-btn-accuse" @click="showAccusationModal = true">
            <span class="action-btn-icon">🎯</span>
            <span class="action-btn-label">Faire une accusation</span>
          </button>
        </template>

        <div v-else-if="isMyTurn && hasPlayed" class="action-waiting">
          ✅ Action envoyée — en attente des autres joueurs…
        </div>

        <div v-else class="action-waiting">
          👁 Observe et prépare ta stratégie…
        </div>
      </div>

      <!-- ── LISTE DES JOUEURS ──────────────────────────────────────────── -->
      <div class="players-section">
        <p class="players-title">Joueurs</p>
        <div class="players-list">
          <div
              v-for="player in players"
              :key="player.id"
              class="player-row"
              :class="{
                'player-row-active': player.id === currentPlayerId,
                'player-row-discovered': discoveredPlayerIds.has(player.id),
              }"
          >
            <div class="player-avatar" :class="player.id === myPlayerId ? 'avatar-me' : 'avatar-other'">
              {{ player.pseudo.slice(0,2).toUpperCase() }}
            </div>
            <span class="player-name">{{ player.pseudo }}</span>
            <div class="player-badges">
              <span v-if="player.id === myPlayerId"         class="badge-pill badge-me">moi</span>
              <span v-if="player.id === currentPlayerId"    class="badge-pill badge-active">joue</span>
              <span v-if="discoveredPlayerIds.has(player.id)" class="badge-pill badge-discovered">découvert</span>
            </div>
          </div>
        </div>
      </div>

      <!-- ── MODAL : Poser une question ─────────────────────────────────── -->
      <BModal v-model="showQuestionModal" title="💬 Poser une question" hide-footer>
        <div class="mb-3">
          <label class="form-label">À qui poses-tu la question ?</label>
          <BFormSelect v-model="questionTarget" :options="[
            { value: '', text: 'Choisir un joueur…', disabled: true },
            ...otherPlayers.map(p => ({ value: p.id, text: p.pseudo }))
          ]" />
        </div>
        <div class="mb-3">
          <label class="form-label">Ta question :</label>
          <BFormInput
              v-model="questionText"
              placeholder="Ex : Est-ce un homme ?"
              maxlength="200"
              @keyup.enter="submitQuestion"
          />
          <small class="text-muted">Attention à tes mots interdits !</small>
        </div>
        <div class="text-center">
          <BButton variant="info" class="fw-bold m-2"
                   :disabled="!questionTarget || !questionText.trim() || submitting"
                   @click="submitQuestion">
            <BSpinner v-if="submitting" small class="me-1" />
            Poser la question
          </BButton>
          <BButton variant="secondary" class="m-2" @click="showQuestionModal = false">Annuler</BButton>
        </div>
      </BModal>

      <!-- ── MODAL : Faire une accusation ──────────────────────────────── -->
      <BModal v-model="showAccusationModal" title="🎯 Faire une accusation" hide-footer>
        <BAlert variant="warning" class="small">
          ⚠️ Une accusation incorrecte ne te fait pas perdre ton tour, mais révèle ta stratégie !
        </BAlert>
        <div class="mb-3">
          <label class="form-label">Qui accuses-tu ?</label>
          <BFormSelect v-model="accusationTarget" :options="[
            { value: '', text: 'Choisir un joueur…', disabled: true },
            ...otherPlayers.map(p => ({ value: p.id, text: p.pseudo }))
          ]" />
        </div>
        <div class="mb-3">
          <label class="form-label">Son personnage selon toi :</label>
          <BFormSelect v-model="accusationCharacter" :options="[
            { value: '', text: 'Choisir un personnage…', disabled: true },
            ...availableCharacters.map(c => ({ value: c.id, text: `${c.name} (${c.universe})` }))
          ]" :disabled="!accusationTarget" />
        </div>
        <div class="text-center">
          <BButton variant="warning" class="fw-bold m-2"
                   :disabled="!accusationTarget || !accusationCharacter || submitting"
                   @click="submitAccusation">
            <BSpinner v-if="submitting" small class="me-1" />
            Accuser
          </BButton>
          <BButton variant="secondary" class="m-2" @click="showAccusationModal = false">Annuler</BButton>
        </div>
      </BModal>

      <!-- ── MODAL : Fin de partie ──────────────────────────────────────── -->
      <BModal v-model="gameEnded" title="Fin de partie" hide-footer no-close-on-backdrop no-close-on-esc>
        <div class="text-center py-3">
          <h2 class="mb-3">{{ gameOverTitle }}</h2>
          <p class="text-muted mb-4">{{ gameOverMsg }}</p>
          <div v-if="winners.length" class="mb-4">
            <h6 class="mb-2">Binôme gagnant :</h6>
            <BBadge v-for="w in winners" :key="w.id" variant="success" class="me-2 fs-6 p-2">
              🏆 {{ w.pseudo }}
            </BBadge>
          </div>
          <BButton variant="primary" class="fw-bold" @click="backToHome">
            Retour à l'accueil
          </BButton>
        </div>
      </BModal>

    </template>
  </div>
</template>

<style scoped>
/* ─── BASE ──────────────────────────────────────────────────────────────────── */
.round-page {
  min-height: 100vh;
  background: #0f0c1a;
  padding: 0 0 6rem;
  font-family: 'Georgia', serif;
  color: #e8e0d0;
}

/* ─── LOADING ───────────────────────────────────────────────────────────────── */
.loading-screen {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  gap: 1.5rem;
}
.loading-orb {
  width: 48px; height: 48px;
  border-radius: 50%;
  border: 2px solid #c9a84c;
  border-top-color: transparent;
  animation: spin 1s linear infinite;
}
.loading-text { color: #a89060; font-size: 0.9rem; letter-spacing: 0.1em; }
@keyframes spin { to { transform: rotate(360deg); } }

/* ─── ERROR ─────────────────────────────────────────────────────────────────── */
.error-banner {
  background: #3d1515;
  color: #f5a0a0;
  text-align: center;
  padding: 0.75rem 1rem;
  font-size: 0.85rem;
}

/* ─── TURN BANNER ───────────────────────────────────────────────────────────── */
.turn-banner {
  position: sticky;
  top: 0;
  z-index: 10;
  padding: 0.75rem 1.25rem;
  border-bottom: 1px solid rgba(201, 168, 76, 0.2);
}
.turn-mine    { background: linear-gradient(135deg, #1a1200, #2a1e00); }
.turn-other   { background: linear-gradient(135deg, #0d0d1a, #151528); }
.turn-inner   { display: flex; align-items: center; gap: 0.75rem; }
.turn-icon    { font-size: 1.4rem; }
.turn-text    { display: flex; flex-direction: column; }
.turn-main    { font-size: 1rem; font-weight: bold; color: #c9a84c; letter-spacing: 0.03em; }
.turn-sub     { font-size: 0.75rem; color: #7a6e58; letter-spacing: 0.08em; }

/* ─── CARTE PERSONNAGE ──────────────────────────────────────────────────────── */
.character-card {
  position: relative;
  margin: 1.25rem 1rem;
  background: linear-gradient(160deg, #1c1530 0%, #120d22 100%);
  border: 1px solid rgba(201, 168, 76, 0.35);
  border-radius: 16px;
  overflow: hidden;
  display: flex;
  gap: 1rem;
  padding: 1rem;
}
.character-card-glow {
  position: absolute;
  top: -40px; left: -40px;
  width: 180px; height: 180px;
  background: radial-gradient(circle, rgba(201,168,76,0.12) 0%, transparent 70%);
  pointer-events: none;
}
.character-image-wrap {
  position: relative;
  flex-shrink: 0;
  width: 110px;
}
.character-image {
  width: 110px;
  height: 140px;
  object-fit: cover;
  border-radius: 10px;
  border: 1px solid rgba(201, 168, 76, 0.4);
  display: block;
}
.character-image-placeholder {
  width: 110px;
  height: 140px;
  border-radius: 10px;
  background: #2a1e3d;
  border: 1px solid rgba(201, 168, 76, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 3rem;
  color: rgba(201, 168, 76, 0.3);
}
.character-universe-badge {
  position: absolute;
  bottom: -8px;
  left: 50%;
  transform: translateX(-50%);
  background: #c9a84c;
  color: #1a1200;
  font-size: 0.6rem;
  font-weight: bold;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  padding: 2px 8px;
  border-radius: 20px;
  white-space: nowrap;
}
.character-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding-top: 0.25rem;
}
.character-label {
  font-size: 0.65rem;
  letter-spacing: 0.15em;
  text-transform: uppercase;
  color: #7a6e58;
  margin: 0;
}
.character-name {
  font-size: 1.4rem;
  color: #e8d898;
  margin: 0;
  line-height: 1.2;
  font-style: italic;
}
.forbidden-section { margin-top: 0.5rem; }
.forbidden-title {
  font-size: 0.65rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: #8b3a3a;
  margin: 0 0 0.4rem;
}
.forbidden-words { display: flex; flex-wrap: wrap; gap: 0.4rem; }
.forbidden-word {
  background: rgba(139, 58, 58, 0.25);
  border: 1px solid rgba(200, 80, 80, 0.4);
  color: #e89090;
  font-size: 0.75rem;
  padding: 3px 10px;
  border-radius: 20px;
  font-family: 'Courier New', monospace;
  letter-spacing: 0.05em;
}

/* ─── NOTIFICATION BINÔME ───────────────────────────────────────────────────── */
.binome-notif {
  margin: 0 1rem 1rem;
  background: rgba(30, 80, 50, 0.5);
  border: 1px solid rgba(80, 180, 100, 0.4);
  border-radius: 12px;
  padding: 0.75rem 1rem;
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  animation: slideIn 0.3s ease;
}
.binome-notif-icon { font-size: 1.3rem; flex-shrink: 0; }
.binome-notif-title { font-size: 0.85rem; font-weight: bold; color: #80e0a0; margin: 0 0 0.2rem; }
.binome-notif-sub { font-size: 0.75rem; color: #60b080; margin: 0; }
@keyframes slideIn { from { opacity: 0; transform: translateY(-8px); } }

/* ─── DERNIÈRE ACTION ───────────────────────────────────────────────────────── */
.last-action {
  margin: 0 1rem 1rem;
  padding: 0.65rem 1rem;
  border-radius: 10px;
  font-size: 0.82rem;
  border-left: 3px solid;
}
.last-action-info     { background: rgba(30, 60, 100, 0.4); border-color: #4a90d9; color: #90bff0; }
.last-action-success  { background: rgba(30, 80, 50, 0.4);  border-color: #4db870; color: #80d090; }
.last-action-danger   { background: rgba(100, 30, 30, 0.4); border-color: #d94a4a; color: #f09090; }

/* ─── BOUTONS D'ACTION ──────────────────────────────────────────────────────── */
.actions-section {
  margin: 0 1rem 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.action-btn {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  width: 100%;
  padding: 1rem 1.25rem;
  border: 1px solid;
  border-radius: 12px;
  background: transparent;
  cursor: pointer;
  transition: all 0.15s ease;
  text-align: left;
}
.action-btn:active { transform: scale(0.98); }
.action-btn-question {
  border-color: rgba(74, 144, 217, 0.5);
  background: rgba(20, 40, 80, 0.5);
}
.action-btn-question:hover { background: rgba(20, 40, 80, 0.8); }
.action-btn-accuse {
  border-color: rgba(201, 168, 76, 0.5);
  background: rgba(40, 30, 10, 0.5);
}
.action-btn-accuse:hover { background: rgba(40, 30, 10, 0.8); }
.action-btn-icon  { font-size: 1.4rem; flex-shrink: 0; }
.action-btn-label { font-size: 0.95rem; color: #c8c0a8; letter-spacing: 0.02em; }
.action-waiting {
  text-align: center;
  padding: 1rem;
  color: #7a6e58;
  font-size: 0.85rem;
  font-style: italic;
  background: rgba(255,255,255,0.03);
  border-radius: 10px;
  border: 1px solid rgba(255,255,255,0.05);
}

/* ─── JOUEURS ───────────────────────────────────────────────────────────────── */
.players-section { margin: 0 1rem; }
.players-title {
  font-size: 0.65rem;
  letter-spacing: 0.15em;
  text-transform: uppercase;
  color: #7a6e58;
  margin: 0 0 0.75rem;
}
.players-list { display: flex; flex-direction: column; gap: 0.5rem; }
.player-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.6rem 0.75rem;
  background: rgba(255,255,255,0.03);
  border: 1px solid rgba(255,255,255,0.06);
  border-radius: 10px;
  transition: all 0.15s;
}
.player-row-active {
  background: rgba(201, 168, 76, 0.08);
  border-color: rgba(201, 168, 76, 0.3);
}
.player-row-discovered {
  opacity: 0.5;
}
.player-avatar {
  width: 32px; height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.65rem;
  font-weight: bold;
  flex-shrink: 0;
  font-family: sans-serif;
}
.avatar-me    { background: rgba(74, 144, 217, 0.3); color: #90bff0; border: 1px solid rgba(74,144,217,0.5); }
.avatar-other { background: rgba(255,255,255,0.08);  color: #a09080; border: 1px solid rgba(255,255,255,0.1); }
.player-name  { flex: 1; font-size: 0.9rem; color: #c8c0a8; }
.player-badges { display: flex; gap: 0.3rem; }
.badge-pill {
  font-size: 0.6rem;
  padding: 2px 7px;
  border-radius: 20px;
  font-family: sans-serif;
  letter-spacing: 0.05em;
  font-weight: bold;
  text-transform: uppercase;
}
.badge-me         { background: rgba(74,144,217,0.2);  color: #90bff0; border: 1px solid rgba(74,144,217,0.4); }
.badge-active     { background: rgba(201,168,76,0.2);  color: #c9a84c; border: 1px solid rgba(201,168,76,0.4); }
.badge-discovered { background: rgba(180,60,60,0.2);   color: #e09090; border: 1px solid rgba(180,60,60,0.4); }

/* ─── FLOU ──────────────────────────────────────────────────────────────────── */
.blurred {
  filter: blur(8px);
  user-select: none;
  transition: filter 0.3s ease;
}

.blur-toggle {
  position: absolute;
  top: 0.6rem;
  right: 0.6rem;
  z-index: 2;
  background: rgba(0, 0, 0, 0.55);
  border: 1px solid rgba(201, 168, 76, 0.4);
  border-radius: 20px;
  color: #c9a84c;
  font-size: 0.7rem;
  padding: 4px 10px;
  cursor: pointer;
  letter-spacing: 0.05em;
  transition: background 0.15s;
}

.blur-toggle:hover {
  background: rgba(0, 0, 0, 0.8);
}
</style>