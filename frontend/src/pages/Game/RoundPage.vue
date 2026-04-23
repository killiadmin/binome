<script setup>
import {ref, computed, onMounted, onUnmounted, nextTick, watch} from 'vue'
import {useRoute, useRouter} from 'vue-router'
import {gameService} from '../../services/gameService'
import {useReverb, resetEcho} from '../../sockets/useReverb.js'
import {
  BButton, BContainer, BModal, BFormInput,
  BFormSelect, BAlert, BBadge, BSpinner
} from 'bootstrap-vue-next'

const route = useRoute()
const router = useRouter()

// ─── SESSION ──────────────────────────────────────────────────────────────────

const session = JSON.parse(localStorage.getItem('session') ?? '{}')
const gameId = route.params.gameId ?? session.gameId
const myPlayerId = ref(session.playerId ?? null)

// ─── STATE ────────────────────────────────────────────────────────────────────

const loading = ref(true)
const submitting = ref(false)
const error = ref(null)

const myCharacter = ref(null)
const players = ref([])
const currentRound = ref(null)
const currentPlayerId = ref(null)
const hasPlayed = ref(false)
const actions = ref([])
const availableCharacters = ref([])
const discoveredBinomes = ref([])

const questionTarget = ref('')
const questionText = ref('')

const accusationTarget = ref('')
const accusationCharacter = ref('')

const showQuestionModal = ref(false)
const showAccusationModal = ref(false)

const binomeNotif = ref(null)

const gameEnded = ref(false)
const winners = ref([])
const gameOverTitle = ref('')
const gameOverMsg = ref('')

const isBlurred = ref(true)

const pendingQuestion = ref(null)
const showAnswerModal = ref(false)
const submittingAnswer = ref(false)

const showRoundTransition = ref(false)
const transitionRoundNumber = ref(1)

const transitionCanvas = ref(null)

const pendingAccusation    = ref(null)
const showAccusationConfirmModal = ref(false)
const submittingConfirm    = ref(false)

// ─── COMPUTED ─────────────────────────────────────────────────────────────────

const isMyTurn = computed(() =>
    currentPlayerId.value === myPlayerId.value &&
    !eliminatedPlayerIds.value.has(myPlayerId.value)
)

const currentPlayerName = computed(() => {
  const p = players.value.find(p => p.id === currentPlayerId.value)
  return p?.pseudo ?? '…'
})

const otherPlayers = computed(() =>
    players.value.filter(p =>
        p.id !== myPlayerId.value &&
        !p.is_eliminated
    )
)

const discoveredPlayerIds = computed(() => {
  const ids = new Set()
  discoveredBinomes.value.forEach(b => {
    if (b.player1_id) ids.add(b.player1_id)
    if (b.player2_id) ids.add(b.player2_id)
  })
  return ids
})

const actionsByRound = computed(() => {
  const groups = {}
  actions.value.forEach(action => {
    const roundId = action.round_id
    if (!groups[roundId]) {
      groups[roundId] = {
        round_id: roundId,
        // Cherche le numéro du round dans l'historique des rounds connus
        number: action.round_number ?? null,
        actions: []
      }
    }
    groups[roundId].actions.push(action)
  })
  return Object.values(groups)
})

const eliminatedPlayerIds = computed(() => {
  const ids = new Set()
  players.value.forEach(p => { if (p.is_eliminated) ids.add(p.id) })
  return ids
})

// ─── INIT ─────────────────────────────────────────────────────────────────────

watch(showRoundTransition, async (val) => {
  if (!val) return
  await nextTick()
  startWaveAnimation(transitionCanvas.value)
})

onMounted(async () => {
  if (!gameId || !myPlayerId.value) {
    router.push({name: 'Home'})
    return
  }

  try {
    const game = await gameService.show(gameId)
    actions.value = game.actions ?? []

    const pending = (game.actions ?? []).find(a =>
        a.type === 'question' &&
        a.is_valid &&
        a.target_player?.id === myPlayerId.value &&
        a.answer === null
    )
    if (pending) {
      pendingQuestion.value = pending
      showAnswerModal.value = true
    }

    players.value = game.binomes.flatMap(b =>
        b.players.map(p => ({ ...p, is_eliminated: p.is_eliminated ?? false }))
    )
    currentRound.value = game.current_round ?? null
    currentPlayerId.value = game.current_round?.current_player_id ?? null
    availableCharacters.value = game.characters ?? []
    discoveredBinomes.value = game.binomes
        .filter(b => b.is_discovered)
        .map(b => ({
          player1_id: b.players[0]?.id,
          player2_id: b.players[1]?.id,
        }))

    const me = await gameService.myCharacter(gameId, myPlayerId.value)
    myCharacter.value = me
  } catch (e) {
    error.value = 'Impossible de charger la partie.'
  } finally {
    loading.value = false
  }

  resetEcho()
  const {joinGame} = useReverb(myPlayerId.value)
  joinGame(gameId, {
    onRoundStarted:       handleRoundStarted,
    onActionPlayed:       handleActionPlayed,
    onAnswerGiven:        handleAnswerGiven,
    onAccusationConfirmed: handleAccusationConfirmed,
    onBinomeDiscovered:   handleBinomeDiscovered,
    onGameEnded:          handleGameEnded,
    onPlayerEliminated: handlePlayerEliminated,
    onError: () => { error.value = 'Connexion WebSocket perdue.' },
  })
})

onUnmounted(() => {
  const {leaveGame} = useReverb(myPlayerId.value)
  leaveGame(gameId)
})


function startWaveAnimation(canvas) {
  if (!canvas) return

  const ctx = canvas.getContext('2d')
  const W = canvas.width = window.innerWidth
  const H = canvas.height = window.innerHeight
  const duration = 2200
  const start = performance.now()

  // Deux vagues décalées
  const waves = [
    {color: 'rgba(201, 168, 76, 0.18)', speed: 1, delay: 0},
    {color: 'rgba(201, 168, 76, 0.10)', speed: 0.85, delay: 180},
    {color: 'rgba(255, 255, 255, 0.06)', speed: 1.1, delay: 80},
  ]

  function easeInOut(t) {
    return t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t
  }

  function drawFrame(now) {
    const elapsed = now - start
    if (elapsed > duration) {
      ctx.clearRect(0, 0, W, H)
      return
    }

    ctx.clearRect(0, 0, W, H)

    waves.forEach(wave => {
      const t = Math.max(0, elapsed - wave.delay) / (duration - wave.delay)
      if (t <= 0) return

      const progress = easeInOut(Math.min(t, 1))
      const centerX = progress * (W + 300) - 150
      const amplitude = H * 0.18
      const frequency = 0.012 * wave.speed

      ctx.beginPath()
      ctx.moveTo(0, H)

      for (let x = 0; x <= W; x += 4) {
        const y = H / 2
            + Math.sin((x * frequency) + (elapsed * 0.004 * wave.speed)) * amplitude
            + Math.sin((x * frequency * 1.7) + (elapsed * 0.003)) * (amplitude * 0.4)
        // Masque : seulement à gauche du front de vague
        if (x < centerX + 60) {
          if (x === 0) ctx.moveTo(x, y)
          else ctx.lineTo(x, y)
        }
      }

      // Ferme vers le bas pour remplir
      ctx.lineTo(Math.min(centerX + 60, W), H)
      ctx.lineTo(0, H)
      ctx.closePath()
      ctx.fillStyle = wave.color
      ctx.fill()
    })

    requestAnimationFrame(drawFrame)
  }

  requestAnimationFrame(drawFrame)
}

// ─── WEBSOCKET HANDLERS ───────────────────────────────────────────────────────

async function handleRoundStarted(data) {
  if (data.is_new_round) {
    await playRoundTransition(data.number)
  }

  currentRound.value = {
    id: data.round_id,
    number: data.number,
    current_player_id: data.current_player.id,
  }
  currentPlayerId.value = data.current_player.id
  hasPlayed.value = false
  resetForms()
}

function handlePlayerEliminated(data) {
  const idx = players.value.findIndex(p => p.id === data.eliminated_player.id)
  if (idx !== -1) {
    players.value[idx] = { ...players.value[idx], is_eliminated: true }
  }
}

function handleActionPlayed(data) {
  actions.value.push(data)
  if (data.player?.id === myPlayerId.value) hasPlayed.value = true

  if (data.type === 'question' && data.is_valid &&
      data.target_player?.id === myPlayerId.value) {
    pendingQuestion.value = data
    showAnswerModal.value = true
  }

  if (data.type === 'accusation' &&
      data.target_player?.id === myPlayerId.value &&
      data.accusation_confirmed === null) {
    pendingAccusation.value = data
    showAccusationConfirmModal.value = true
  }
}

function handleAnswerGiven(data) {
  const idx = actions.value.findIndex(
      a => (a.action_id ?? a.id) === data.action_id
  )
  if (idx !== -1) {
    actions.value[idx] = {...actions.value[idx], answer: data.answer}
  }
}

async function playRoundTransition(roundNumber) {
  transitionRoundNumber.value = roundNumber
  showRoundTransition.value = true
  await new Promise(resolve => setTimeout(resolve, 5000))
  showRoundTransition.value = false
}

async function submitAnswer(answer) {
  if (submittingAnswer.value || !pendingQuestion.value) return
  submittingAnswer.value = true
  try {
    await gameService.playAnswer(
        gameId,
        pendingQuestion.value.round_id,
        pendingQuestion.value.action_id,
        myPlayerId.value,
        answer
    )
    showAnswerModal.value = false
    pendingQuestion.value = null
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors de la réponse.'
  } finally {
    submittingAnswer.value = false
  }
}

function handleBinomeDiscovered(data) {
  discoveredBinomes.value.push(data.binome)
  binomeNotif.value = {
    player1: data.binome.player1_pseudo,
    character1: data.binome.player1_character,
    player2: data.binome.player2_pseudo,
    character2: data.binome.player2_character,
  }
  setTimeout(() => {
    binomeNotif.value = null
  }, 6000)
}

function handleGameEnded(data) {
  gameEnded.value = true
  winners.value = data.winning_binome?.players ?? []
  const iWon = winners.value.some(w => w.id === myPlayerId.value)
  gameOverTitle.value = iWon ? '🏆 Victoire !' : '💀 Défaite'
  gameOverMsg.value = iWon
      ? "Votre binôme n'a jamais été découvert. Bien joué !"
      : 'Votre binôme a été découvert. Meilleure chance la prochaine fois !'
}

function handleAccusationConfirmed(data) {
  const idx = actions.value.findIndex(a => (a.action_id ?? a.id) === data.action_id)
  if (idx !== -1) {
    actions.value[idx] = {
      ...actions.value[idx],
      accusation_confirmed: data.accusation_confirmed,
      accusation_correct:   data.accusation_correct,
    }
  }
}

async function submitConfirmAccusation(confirmed) {
  if (submittingConfirm.value || !pendingAccusation.value) return
  submittingConfirm.value = true
  try {
    await gameService.confirmAccusation(
        gameId,
        pendingAccusation.value.round_id,
        pendingAccusation.value.action_id,
        myPlayerId.value,
        confirmed
    )
    showAccusationConfirmModal.value = false
    pendingAccusation.value = null
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors de la confirmation.'
  } finally {
    submittingConfirm.value = false
  }
}

// ─── ACTIONS ──────────────────────────────────────────────────────────────────

async function submitQuestion() {
  if (!questionTarget.value || !questionText.value.trim() || submitting.value) return
  submitting.value = true
  error.value = null
  try {
    await gameService.playQuestion(gameId, currentRound.value.id, myPlayerId.value, questionTarget.value, questionText.value.trim())
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
  questionTarget.value = ''
  questionText.value = ''
  accusationTarget.value = ''
  accusationCharacter.value = ''
}

function backToHome() {
  localStorage.removeItem('session')
  router.push({name: 'Home'})
}
</script>

<template>
  <div class="round-page mt-5">

    <!-- ── ANIMATION TRANSITION ROUND ─────────────────────────────────────── -->
    <div v-if="showRoundTransition" class="round-transition-overlay">
      <canvas ref="transitionCanvas" class="round-transition-canvas"></canvas>
      <div class="round-transition-text">
        <span class="round-transition-label">ROUND</span>
        <span class="round-transition-number">{{ transitionRoundNumber }}</span>
      </div>
    </div>

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

      <!-- ── HISTORIQUE DES ACTIONS ─────────────────────────────────────────── -->
      <div v-if="actions.length" class="actions-history">
        <p class="history-title">📜 Historique de la partie</p>
        <div class="history-list">

          <template v-for="group in [...actionsByRound].reverse()" :key="group.round_id">

            <!-- Séparateur de round -->
            <div class="history-round-separator">
              <span class="history-round-line"></span>
              <span class="history-round-badge">
          Round {{ group.number ?? group.round_id }}
        </span>
              <span class="history-round-line"></span>
            </div>

            <!-- Actions du round -->
            <div
                v-for="(action, i) in [...group.actions].reverse()"
                :key="action.action_id ?? i"
                class="history-item"
                :class="{
          'history-question-invalid': action.type === 'question' && !action.is_valid,
          'history-question-valid':   action.type === 'question' && action.is_valid,
          'history-accusation-ok':    action.type === 'accusation' && action.accusation_correct,
          'history-accusation-ko':    action.type === 'accusation' && !action.accusation_correct,
        }"
            >
        <span class="history-icon">
  <template v-if="action.type === 'question' && !action.is_valid">❌</template>
  <template v-else-if="action.type === 'question'">💬</template>
  <template v-else-if="action.type === 'accusation' && action.accusation_confirmed === null">⏳</template>
  <template v-else-if="action.type === 'accusation' && action.accusation_correct">🎯</template>
  <template v-else>❌</template>
</span>
              <div class="history-content">
                <span class="history-actor">{{ action.player?.pseudo }}</span>

                <!-- Question valide -->
                <template v-if="action.type === 'question' && action.is_valid">
                  &nbsp;demande à
                  <span class="history-target">{{ action.target_player?.pseudo }}</span>
                  : <em>« {{ action.question }} »</em>
                  <span v-if="action.answer !== null && action.answer !== undefined"
                        :class="{
            'answer-yes':       action.answer === 'yes',
            'answer-no':        action.answer === 'no',
            'answer-dont-know': action.answer === 'dont_know',
          }">
      → {{ action.answer === 'yes' ? 'Oui ✅' : action.answer === 'no' ? 'Non ❌' : 'Je ne sais pas 🤷' }}
    </span>
                  <span v-else class="history-muted"> → en attente de réponse…</span>
                </template>

                <!-- Question refusée -->
                <template v-else-if="action.type === 'question' && !action.is_valid">
                  &nbsp;<span class="history-muted">a utilisé un mot interdit — tour perdu.</span>
                </template>

                <!-- Accusation en attente ← EN PREMIER avant les autres cas accusation -->
                <template v-else-if="action.type === 'accusation' && action.accusation_confirmed === null">
                  &nbsp;accuse
                  <span class="history-target">{{ action.target_player?.pseudo }}</span>
                  d'être <em>« {{ action.character_name }} »</em>
                  <span class="history-muted"> → en attente de confirmation…</span>
                </template>

                <!-- Accusation correcte -->
                <template v-else-if="action.type === 'accusation' && action.accusation_correct">
                  &nbsp;a correctement accusé
                  <span class="history-target">{{ action.target_player?.pseudo }}</span>
                  d'être <em>{{ action.character_name }}</em> ! 🎯
                </template>

                <!-- Accusation niée ou incorrecte -->
                <template v-else-if="action.type === 'accusation' && action.accusation_confirmed === false">
                  &nbsp;a accusé
                  <span class="history-target">{{ action.target_player?.pseudo }}</span>
                  d'être <em>{{ action.character_name }}</em>
                  — <span class="history-muted">nié par le joueur.</span>
                </template>

                <!-- Fallback -->
                <template v-else>
                  &nbsp;a accusé
                  <span class="history-target">{{ action.target_player?.pseudo }}</span>
                  — <span class="history-muted">mauvaise accusation.</span>
                </template>
              </div>
            </div>

          </template>
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
                'player-row-active':     player.id === currentPlayerId,
                'player-row-eliminated': eliminatedPlayerIds.has(player.id),
              }"
          >
            <div class="player-avatar" :class="player.id === myPlayerId ? 'avatar-me' : 'avatar-other'">
              {{ player.pseudo.slice(0, 2).toUpperCase() }}
            </div>
            <span class="player-name">{{ player.pseudo }}</span>
            <div class="player-badges">
              <span v-if="player.id === myPlayerId" class="badge-pill badge-me">moi</span>
              <span v-if="player.id === currentPlayerId" class="badge-pill badge-active">joue</span>
              <span v-if="discoveredPlayerIds.has(player.id)" class="badge-pill badge-discovered">découvert</span>
              <span v-if="eliminatedPlayerIds.has(player.id)" class="badge-pill badge-eliminated">
                💀 éliminé
              </span>
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
          ]"/>
        </div>
        <div class="mb-3">
          <label class="form-label">Ta question :</label>
          <BFormInput
              v-model="questionText"
              placeholder="Pose ta question !"
              maxlength="200"
              @keyup.enter="submitQuestion"
          />
          <small class="text-muted">Attention à tes mots interdits !</small>
        </div>
        <div class="text-center">
          <BButton variant="info" class="fw-bold m-2"
                   :disabled="!questionTarget || !questionText.trim() || submitting"
                   @click="submitQuestion">
            <BSpinner v-if="submitting" small class="me-1"/>
            Poser la question
          </BButton>
          <BButton variant="secondary" class="m-2" @click="showQuestionModal = false">Annuler</BButton>
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

      <!-- ── MODAL : Répondre à une question ───────────────────────────────── -->
      <BModal
          v-model="showAnswerModal"
          title="❓ On te pose une question !"
          hide-footer
          no-close-on-backdrop
          no-close-on-esc
          centered
          scrollable
      >
        <div class="answer-modal-body">

          <!-- Question posée -->
          <div class="answer-question-box">
            <p class="answer-from">
              <span class="history-actor">{{ pendingQuestion?.player?.pseudo }}</span>
              te demande :
            </p>
            <p class="answer-question-text">« {{ pendingQuestion?.question }} »</p>
          </div>

          <!-- Rappel mots interdits -->
          <div class="answer-forbidden">
            <p class="forbidden-title">☠ Tes mots interdits (attention dans ta réponse !)</p>
            <div class="forbidden-words">
        <span
            v-for="word in myCharacter?.forbidden_words ?? []"
            :key="word"
            class="forbidden-word"
        >{{ word }}</span>
            </div>
          </div>

          <div class="answer-buttons">
            <button class="answer-btn answer-btn-yes"
                    :disabled="submittingAnswer"
                    @click="submitAnswer('yes')">
              <BSpinner v-if="submittingAnswer" small class="me-1"/>
              ✅ Oui
            </button>
            <button class="answer-btn answer-btn-no"
                    :disabled="submittingAnswer"
                    @click="submitAnswer('no')">
              ❌ Non
            </button>
            <button class="answer-btn answer-btn-dont-know"
                    :disabled="submittingAnswer"
                    @click="submitAnswer('dont_know')">
              🤷 Je ne sais pas
            </button>
          </div>
        </div>
      </BModal>

      <BModal v-model="showAccusationModal" title="🎯 Faire une accusation" hide-footer centered>
        <BAlert variant="warning" class="small">
          ⚠️ Si le joueur confirme, son binôme est éliminé !
        </BAlert>
        <div class="mb-3">
          <label class="form-label">Qui accuses-tu ?</label>
          <BFormSelect v-model="accusationTarget" :options="[
      { value: '', text: 'Choisir un joueur…', disabled: true },
      ...otherPlayers.map(p => ({ value: p.id, text: p.pseudo }))
    ]"/>
        </div>
        <div class="mb-3">
          <label class="form-label">Son personnage selon toi :</label>
          <BFormInput
              v-model="accusationCharacter"
              placeholder="Ex : Simba, Iron Man…"
              maxlength="100"
              :disabled="!accusationTarget"
              @keyup.enter="submitAccusation"
          />
        </div>
        <div class="text-center">
          <BButton variant="warning" class="fw-bold m-2"
                   :disabled="!accusationTarget || !accusationCharacter.trim() || submitting"
                   @click="submitAccusation">
            <BSpinner v-if="submitting" small class="me-1"/>
            Accuser
          </BButton>
          <BButton variant="secondary" class="m-2" @click="showAccusationModal = false">Annuler</BButton>
        </div>
      </BModal>

      <BModal v-model="showAccusationConfirmModal"
              title="⚔️ Tu es accusé !"
              hide-footer
              no-close-on-backdrop
              no-close-on-esc
              centered>
        <div class="answer-modal-body">
          <div class="answer-question-box">
            <p class="answer-from">
              <span class="history-actor">{{ pendingAccusation?.player?.pseudo }}</span>
              t'accuse d'être :
            </p>
            <p class="answer-question-text">« {{ pendingAccusation?.character_name }} »</p>
          </div>

          <BAlert variant="danger" class="small mb-0">
            ⚠️ Si tu confirmes, ton binôme sera éliminé de la partie !
          </BAlert>

          <div class="answer-buttons">
            <button class="answer-btn answer-btn-yes"
                    :disabled="submittingConfirm"
                    @click="submitConfirmAccusation(true)">
              <BSpinner v-if="submittingConfirm" small class="me-1"/>
              ✅ Oui, c'est moi
            </button>
            <button class="answer-btn answer-btn-no"
                    :disabled="submittingConfirm"
                    @click="submitConfirmAccusation(false)">
              ❌ Non, ce n'est pas moi
            </button>
          </div>
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
  max-width: 430px;
  margin: 0 auto;
  position: relative;
  box-shadow: 0 0 60px rgba(0, 0, 0, 0.8);
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
  width: 48px;
  height: 48px;
  border-radius: 50%;
  border: 2px solid #c9a84c;
  border-top-color: transparent;
  animation: spin 1s linear infinite;
}

.loading-text {
  color: #a89060;
  font-size: 0.9rem;
  letter-spacing: 0.1em;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

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

.turn-mine {
  background: linear-gradient(135deg, #1a1200, #2a1e00);
}

.turn-other {
  background: linear-gradient(135deg, #0d0d1a, #151528);
}

.turn-inner {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.turn-icon {
  font-size: 1.4rem;
}

.turn-text {
  display: flex;
  flex-direction: column;
}

.turn-main {
  font-size: 1rem;
  font-weight: bold;
  color: #c9a84c;
  letter-spacing: 0.03em;
}

.turn-sub {
  font-size: 0.75rem;
  color: #7a6e58;
  letter-spacing: 0.08em;
}

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
  top: -40px;
  left: -40px;
  width: 180px;
  height: 180px;
  background: radial-gradient(circle, rgba(201, 168, 76, 0.12) 0%, transparent 70%);
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

.forbidden-section {
  margin-top: 0.5rem;
}

.forbidden-title {
  font-size: 0.65rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: #8b3a3a;
  margin: 0 0 0.4rem;
}

.forbidden-words {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

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

.binome-notif-icon {
  font-size: 1.3rem;
  flex-shrink: 0;
}

.binome-notif-title {
  font-size: 0.85rem;
  font-weight: bold;
  color: #80e0a0;
  margin: 0 0 0.2rem;
}

.binome-notif-sub {
  font-size: 0.75rem;
  color: #60b080;
  margin: 0;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }
}

/* ─── DERNIÈRE ACTION ───────────────────────────────────────────────────────── */
.last-action {
  margin: 0 1rem 1rem;
  padding: 0.65rem 1rem;
  border-radius: 10px;
  font-size: 0.82rem;
  border-left: 3px solid;
}

.last-action-info {
  background: rgba(30, 60, 100, 0.4);
  border-color: #4a90d9;
  color: #90bff0;
}

.last-action-success {
  background: rgba(30, 80, 50, 0.4);
  border-color: #4db870;
  color: #80d090;
}

.last-action-danger {
  background: rgba(100, 30, 30, 0.4);
  border-color: #d94a4a;
  color: #f09090;
}

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

.action-btn:active {
  transform: scale(0.98);
}

.action-btn-question {
  border-color: rgba(74, 144, 217, 0.5);
  background: rgba(20, 40, 80, 0.5);
}

.action-btn-question:hover {
  background: rgba(20, 40, 80, 0.8);
}

.action-btn-accuse {
  border-color: rgba(201, 168, 76, 0.5);
  background: rgba(40, 30, 10, 0.5);
}

.action-btn-accuse:hover {
  background: rgba(40, 30, 10, 0.8);
}

.action-btn-icon {
  font-size: 1.4rem;
  flex-shrink: 0;
}

.action-btn-label {
  font-size: 0.95rem;
  color: #c8c0a8;
  letter-spacing: 0.02em;
}

.action-waiting {
  text-align: center;
  padding: 1rem;
  color: #7a6e58;
  font-size: 0.85rem;
  font-style: italic;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.05);
}

/* ─── JOUEURS ───────────────────────────────────────────────────────────────── */
.players-section {
  margin: 0 1rem;
}

.players-title {
  font-size: 0.65rem;
  letter-spacing: 0.15em;
  text-transform: uppercase;
  color: #7a6e58;
  margin: 0 0 0.75rem;
}

.players-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.player-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.6rem 0.75rem;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
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
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.65rem;
  font-weight: bold;
  flex-shrink: 0;
  font-family: sans-serif;
}

.avatar-me {
  background: rgba(74, 144, 217, 0.3);
  color: #90bff0;
  border: 1px solid rgba(74, 144, 217, 0.5);
}

.avatar-other {
  background: rgba(255, 255, 255, 0.08);
  color: #a09080;
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.player-name {
  flex: 1;
  font-size: 0.9rem;
  color: #c8c0a8;
}

.player-badges {
  display: flex;
  gap: 0.3rem;
}

.badge-pill {
  font-size: 0.6rem;
  padding: 2px 7px;
  border-radius: 20px;
  font-family: sans-serif;
  letter-spacing: 0.05em;
  font-weight: bold;
  text-transform: uppercase;
}

.badge-me {
  background: rgba(74, 144, 217, 0.2);
  color: #90bff0;
  border: 1px solid rgba(74, 144, 217, 0.4);
}

.badge-active {
  background: rgba(201, 168, 76, 0.2);
  color: #c9a84c;
  border: 1px solid rgba(201, 168, 76, 0.4);
}

.badge-discovered {
  background: rgba(180, 60, 60, 0.2);
  color: #e09090;
  border: 1px solid rgba(180, 60, 60, 0.4);
}

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

/* ─── HISTORIQUE DES ACTIONS ────────────────────────────────────────────────── */
.actions-history {
  margin: 0 1rem 1.5rem;
}

.history-title {
  font-size: 0.65rem;
  letter-spacing: 0.15em;
  text-transform: uppercase;
  color: #7a6e58;
  margin: 0 0 0.6rem;
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  max-height: 220px;
  overflow-y: auto;
  padding-right: 0.25rem;
}

.history-list::-webkit-scrollbar {
  width: 3px;
}

.history-list::-webkit-scrollbar-track {
  background: transparent;
}

.history-list::-webkit-scrollbar-thumb {
  background: rgba(201, 168, 76, 0.3);
  border-radius: 2px;
}

.history-item {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  border-radius: 8px;
  font-size: 0.8rem;
  border-left: 2px solid;
  line-height: 1.4;
  min-width: 0;
}

.history-question-valid {
  background: rgba(30, 60, 100, 0.35);
  border-color: #4a90d9;
  color: #a0c4f0;
}

.history-question-invalid {
  background: rgba(100, 30, 30, 0.35);
  border-color: #d94a4a;
  color: #f09090;
}

.history-accusation-ok {
  background: rgba(30, 80, 50, 0.35);
  border-color: #4db870;
  color: #80d090;
}

.history-accusation-ko {
  background: rgba(100, 30, 30, 0.35);
  border-color: #d94a4a;
  color: #f09090;
}

.history-icon {
  flex-shrink: 0;
  font-size: 0.9rem;
  margin-top: 1px;
}

.history-actor {
  font-weight: bold;
  color: #c9a84c;
}

.history-target {
  font-weight: bold;
  color: #e8d898;
}

.history-muted {
  color: inherit;
  opacity: 0.7;
  font-style: italic;
}

.history-content {
  flex: 1;
  word-break: break-word;
  overflow-wrap: break-word;
  min-width: 0;
}

/* ─── MODALE RÉPONSE ────────────────────────────────────────────────────────── */
.answer-modal-body {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.answer-question-box {
  background: rgba(74, 144, 217, 0.1);
  border: 1px solid rgba(74, 144, 217, 0.3);
  border-radius: 10px;
  padding: 1rem;
}

.answer-from {
  font-size: 0.8rem;
  color: #7a6e58;
  margin: 0 0 0.4rem;
}

.answer-question-text {
  font-size: 1.1rem;
  color: #e8d898;
  font-style: italic;
  margin: 0;
}

.answer-forbidden {
  background: rgba(139, 58, 58, 0.15);
  border: 1px solid rgba(200, 80, 80, 0.25);
  border-radius: 10px;
  padding: 0.75rem 1rem;
}

.answer-buttons {
  display: flex;
  gap: 0.75rem;
}

.answer-btn {
  flex: 1;
  padding: 0.9rem;
  border-radius: 12px;
  border: 1px solid;
  font-size: 1rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.15s;
  background: transparent;
}

.answer-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.answer-btn-yes {
  border-color: rgba(77, 184, 112, 0.6);
  color: #80d090;
}

.answer-btn-yes:hover:not(:disabled) {
  background: rgba(77, 184, 112, 0.15);
}

.answer-btn-no {
  border-color: rgba(217, 74, 74, 0.6);
  color: #f09090;
}

.answer-btn-no:hover:not(:disabled) {
  background: rgba(217, 74, 74, 0.15);
}

/* ─── RÉPONSE DANS L'HISTORIQUE ─────────────────────────────────────────────── */
.answer-yes {
  color: #80d090;
  font-weight: bold;
}

.answer-no {
  color: #f09090;
  font-weight: bold;
}

.answer-btn-dont-know {
  border-color: rgba(201, 168, 76, 0.6);
  color: #c9a84c;
}

.answer-btn-dont-know:hover:not(:disabled) {
  background: rgba(201, 168, 76, 0.15);
}

.answer-dont-know {
  color: #c9a84c;
  font-weight: bold;
}


/* ─── MODALE RÉPONSE ────────────────────────────────────────────────────────── */
.answer-modal-body {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  max-height: 70vh; /* ← ne jamais dépasser 70% de l'écran */
  overflow-y: auto; /* ← scroll interne si trop long */
}

.answer-question-box {
  background: rgba(74, 144, 217, 0.1);
  border: 1px solid rgba(74, 144, 217, 0.3);
  border-radius: 10px;
  padding: 0.75rem;
}

.answer-from {
  font-size: 0.8rem;
  color: #7a6e58;
  margin: 0 0 0.4rem;
}

.answer-question-text {
  font-size: 1rem; /* ← réduit légèrement */
  color: #e8d898;
  font-style: italic;
  margin: 0;
  word-break: break-word; /* ← empêche le texte long de casser le layout */
  overflow-wrap: break-word;
  white-space: normal;
}

.answer-forbidden {
  background: rgba(139, 58, 58, 0.15);
  border: 1px solid rgba(200, 80, 80, 0.25);
  border-radius: 10px;
  padding: 0.65rem 0.75rem;
}

.answer-buttons {
  display: flex;
  gap: 0.5rem;
}

.answer-btn {
  flex: 1;
  padding: 0.75rem 0.25rem;
  border-radius: 12px;
  border: 1px solid;
  font-size: 0.85rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.15s;
  background: transparent;
  white-space: nowrap;
}

/* ─── TRANSITION ROUND ──────────────────────────────────────────────────────── */
.round-transition-overlay {
  position: absolute;
  inset: 0;
  z-index: 1000;
  background: rgba(10, 6, 20, 0.92);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  animation: overlayFade 5s ease forwards;
}

@keyframes overlayFade {
  0% {
    opacity: 0;
  }
  15% {
    opacity: 1;
  }
  75% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

.round-transition-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

.round-transition-text {
  position: relative;
  z-index: 2;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.25rem;
  animation: textPop 5s ease forwards;
}

@keyframes textPop {
  0% {
    opacity: 0;
    transform: scale(0.7);
  }
  20% {
    opacity: 1;
    transform: scale(1.05);
  }
  35% {
    transform: scale(1);
  }
  75% {
    opacity: 1;
    transform: scale(1);
  }
  100% {
    opacity: 0;
    transform: scale(1.1);
  }
}

.round-transition-label {
  font-size: 0.75rem;
  letter-spacing: 0.4em;
  color: #c9a84c;
  font-family: sans-serif;
  font-weight: bold;
  text-transform: uppercase;
}

.round-transition-number {
  font-size: 6rem;
  font-weight: bold;
  color: #e8d898;
  font-family: 'Georgia', serif;
  line-height: 1;
  text-shadow: 0 0 40px rgba(201, 168, 76, 0.6),
  0 0 80px rgba(201, 168, 76, 0.3);
}

/* ─── SÉPARATEUR DE ROUND ───────────────────────────────────────────────────── */
.history-round-separator {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin: 0.5rem 0 0.25rem;
}

.history-round-line {
  flex: 1;
  height: 1px;
  background: rgba(201, 168, 76, 0.2);
}

.history-round-badge {
  font-size: 0.6rem;
  letter-spacing: 0.15em;
  text-transform: uppercase;
  color: #c9a84c;
  background: rgba(201, 168, 76, 0.1);
  border: 1px solid rgba(201, 168, 76, 0.25);
  padding: 2px 10px;
  border-radius: 20px;
  white-space: nowrap;
  font-family: sans-serif;
  font-weight: bold;
}

.badge-eliminated {
  background: rgba(100,30,30,0.3);
  color: #e09090;
  border: 1px solid rgba(180,60,60,0.4);
}

.player-row-eliminated {
  opacity: 0.4;
  text-decoration: line-through;
}
</style>
