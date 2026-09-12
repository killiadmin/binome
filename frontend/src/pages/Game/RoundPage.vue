<script setup>
import {ref, computed, onMounted, onUnmounted, nextTick, watch} from 'vue'
import {useRoute, useRouter} from 'vue-router'
import {gameService} from '../../services/gameService'
import {useReverb, resetEcho} from '../../sockets/useReverb.js'
import {
  BModal, BFormInput,
  BAlert, BSpinner
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

const gameStats       = ref([])
const showScoreBoard  = ref(false)
const gameWinners     = ref([])

// ─── COMPUTED ─────────────────────────────────────────────────────────────────

const isMyTurn = computed(() =>
    currentPlayerId.value === myPlayerId.value &&
    !eliminatedPlayerIds.value.has(myPlayerId.value)
)

const currentPlayerName = computed(() => {
  const p = players.value.find(p => p.id === currentPlayerId.value)
  return p?.pseudo ?? '…'
})

// Mots interdits de MON personnage présents dans la question qu'on me pose.
// Détection purement locale : le serveur ne renvoie mes mots interdits qu'à moi
// (GET /games/{game}/me), donc cette info ne fuite jamais vers le questionneur.
// Même sémantique que l'ancien contrôle serveur : sous-chaîne, insensible à la casse.
const forbiddenWordsInQuestion = computed(() => {
  const text = pendingQuestion.value?.question ?? ''
  if (!text) return []
  const haystack = text.toLowerCase()
  return (myCharacter.value?.forbidden_words ?? [])
      .filter(word => haystack.includes(String(word).toLowerCase()))
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
    {color: 'rgba(224, 163, 28, 0.18)', speed: 1, delay: 0},
    {color: 'rgba(224, 163, 28, 0.10)', speed: 0.85, delay: 180},
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

async function handleGameEnded(data) {
  gameEnded.value  = true
  gameStats.value  = data.stats ?? []
  gameWinners.value = data.winners ?? []

  const iWon = data.winners?.some(w => w.id === myPlayerId.value)
  gameOverTitle.value = iWon ? '🏆 Victoire !' : '💀 Défaite'
  gameOverMsg.value   = iWon
      ? "Votre binôme n'a jamais été découvert. Bien joué !"
      : 'Votre binôme a été découvert. Meilleure chance la prochaine fois !'

  await new Promise(r => setTimeout(r, 4000))
  showScoreBoard.value = true
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
  <div class="round-page arcade-bg">

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
      <div class="loading-blocks"><span></span><span></span><span></span></div>
      <p class="loading-text">Chargement de la partie<span class="caret">_</span></p>
    </div>

    <template v-else>

      <!-- Erreur globale -->
      <div v-if="error" class="arcade-alert round-error">
        <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
      </div>

      <!-- ── MARQUEE : tour en cours ─────────────────────────────────────── -->
      <div class="turn-marquee" :class="isMyTurn ? 'is-mine' : 'is-other'">
        <div class="turn-marquee__inner">
          <span class="turn-marquee__icon">
            <i :class="isMyTurn ? 'fa-solid fa-bolt' : 'fa-solid fa-hourglass-half'"></i>
          </span>
          <div class="turn-marquee__text">
            <span class="turn-marquee__main">
              {{ isMyTurn ? 'Ton tour !' : `Tour de ${currentPlayerName}` }}
            </span>
            <span class="turn-marquee__sub">
              {{ isMyTurn ? 'Question ou accusation' : 'À toi de deviner…' }}
            </span>
          </div>
          <div class="turn-marquee__round">
            <span class="turn-marquee__round-label">Round</span>
            <span class="lcd-code turn-marquee__lcd">
              <span class="lcd-char">{{ currentRound?.number ?? '—' }}</span>
            </span>
          </div>
        </div>
      </div>

      <!-- ── CARTE PERSONNAGE ────────────────────────────────────────────── -->
      <section class="board-card character-board">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>

        <!-- Bouton flou : sur sa propre ligne pour ne jamais chevaucher le titre -->
        <div class="character-board__head">
          <button type="button" class="reveal-btn" @click="isBlurred = !isBlurred">
            <i :class="isBlurred ? 'fa-solid fa-eye' : 'fa-solid fa-eye-slash'"></i>
            {{ isBlurred ? 'Révéler' : 'Masquer' }}
          </button>
        </div>

        <h2 class="board-card__title character-board__title">Ton personnage</h2>

        <div class="character-board__body" :class="{ blurred: isBlurred }">
          <!-- Portrait -->
          <div class="character-portrait">
            <div class="pixel-frame character-portrait__frame">
              <img
                  v-if="myCharacter?.image"
                  :src="myCharacter.image"
                  :alt="myCharacter?.name"
                  class="character-portrait__img"
              />
              <div v-else class="character-portrait__placeholder">
                <span>?</span>
              </div>
            </div>
            <span v-if="myCharacter?.cosmos" class="portrait-badge portrait-badge--cosmos">
              {{ myCharacter.cosmos }}
            </span>
            <span class="portrait-badge portrait-badge--universe">
              {{ myCharacter?.universe ?? '…' }}
            </span>
          </div>

          <!-- Infos personnage -->
          <div class="character-info">
            <p class="character-name">
              {{ myCharacter?.name ?? '???' }}
            </p>

            <!-- Mots interdits -->
            <div class="forbidden-section">
              <p class="forbidden-title">
                <i class="fa-solid fa-skull"></i> Mots interdits
              </p>
              <div class="forbidden-words">
                <span
                    v-for="word in myCharacter?.forbidden_words ?? []"
                    :key="word"
                    class="pixel-chip pixel-chip--danger"
                >{{ word }}</span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- ── NOTIFICATION BINÔME DÉCOUVERT ─────────────────────────────── -->
      <div v-if="binomeNotif" class="binome-notif">
        <span class="binome-notif__icon"><i class="fa-solid fa-magnifying-glass"></i></span>
        <div>
          <p class="binome-notif__title">Binôme découvert !</p>
          <p class="binome-notif__sub">
            {{ binomeNotif.player1 }} ({{ binomeNotif.character1 }})
            &amp; {{ binomeNotif.player2 }} ({{ binomeNotif.character2 }})
          </p>
        </div>
      </div>

      <!-- ── ACTIONS (mon tour) ─────────────────────────────────────────── -->
      <div class="actions-section">
        <template v-if="isMyTurn && !hasPlayed">
          <button type="button" class="cabinet-btn action-cabinet" @click="showQuestionModal = true">
            <span class="cabinet-btn__icon"><i class="fa-solid fa-comment-dots"></i></span>
            <span class="action-cabinet__label">Poser une question</span>
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--danger action-cabinet" @click="showAccusationModal = true">
            <span class="cabinet-btn__icon"><i class="fa-solid fa-crosshairs"></i></span>
            <span class="action-cabinet__label">Faire une accusation</span>
          </button>
        </template>

        <div v-else-if="isMyTurn && hasPlayed" class="action-waiting">
          <i class="fa-solid fa-circle-check"></i>
          Action envoyée — en attente des autres joueurs<span class="caret">_</span>
        </div>

        <div v-else class="action-waiting">
          <i class="fa-solid fa-eye"></i>
          Observe et prépare ta stratégie<span class="caret">_</span>
        </div>
      </div>

      <!-- ── HISTORIQUE DES ACTIONS ─────────────────────────────────────────── -->
      <section v-if="actions.length" class="arcade-panel actions-history">
        <p class="arcade-eyebrow">Historique de la partie</p>
        <div class="history-list">

          <template v-for="group in [...actionsByRound].reverse()" :key="group.round_id">

            <!-- Séparateur de round -->
            <div class="history-round-separator">
              <span class="history-round-line"></span>
              <span class="pixel-chip pixel-chip--gold history-round-badge">
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
      </section>

      <!-- ── LISTE DES JOUEURS ──────────────────────────────────────────── -->
      <section class="board-card players-board">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>

        <h2 class="board-card__title">Joueurs</h2>

        <div class="players-grid">
          <div
              v-for="player in players"
              :key="player.id"
              class="player-token"
              :class="{
                'is-active':     player.id === currentPlayerId && !eliminatedPlayerIds.has(player.id),
                'is-eliminated': eliminatedPlayerIds.has(player.id),
              }"
          >
            <div class="player-token__avatar">
              {{ player.pseudo.slice(0, 2).toUpperCase() }}
            </div>
            <div class="player-token__name">{{ player.pseudo }}</div>
            <div class="player-token__badges">
              <span v-if="player.id === myPlayerId" class="player-token__badge">moi</span>
              <span v-if="player.id === currentPlayerId && !eliminatedPlayerIds.has(player.id)"
                    class="player-token__badge player-token__badge--active">joue</span>
              <span v-if="discoveredPlayerIds.has(player.id)"
                    class="player-token__badge player-token__badge--danger">découvert</span>
              <span v-if="eliminatedPlayerIds.has(player.id)"
                    class="player-token__badge player-token__badge--danger">💀 éliminé</span>
            </div>
          </div>
        </div>
      </section>

      <!-- ── MODAL : Poser une question ─────────────────────────────────── -->
      <BModal v-model="showQuestionModal" title="💬 Poser une question" no-footer class="arcade-modal">
        <div class="mb-3">
          <label class="picker-label">À qui poses-tu la question ?</label>
          <div class="target-picker" role="radiogroup" aria-label="Choisir un joueur">
            <button
                v-for="p in otherPlayers"
                :key="p.id"
                type="button"
                class="target-chip"
                :class="{ 'is-selected': questionTarget === p.id }"
                role="radio"
                :aria-checked="questionTarget === p.id"
                @click="questionTarget = p.id"
            >
              <span class="target-chip__avatar">{{ p.pseudo.slice(0, 2).toUpperCase() }}</span>
              <span class="target-chip__name">{{ p.pseudo }}</span>
              <span v-if="questionTarget === p.id" class="target-chip__check">
                <i class="fa-solid fa-check"></i>
              </span>
            </button>
          </div>
          <p v-if="!otherPlayers.length" class="picker-empty">Aucun joueur disponible.</p>
        </div>
        <div class="mb-3">
          <label class="form-label">Ta question :</label>
          <BFormInput
              v-model="questionText"
              placeholder="Pose ta question !"
              maxlength="200"
              @keyup.enter="submitQuestion"
          />
          <small class="text-muted">Tu ignores les mots interdits de ta cible : si tu en prononces un, elle aura le droit de te mentir.</small>
        </div>
        <div class="text-center">
          <button type="button" class="cabinet-btn cabinet-btn--sm"
                   :disabled="!questionTarget || !questionText.trim() || submitting"
                   @click="submitQuestion">
            <BSpinner v-if="submitting" small class="me-1"/>
            Poser la question
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showQuestionModal = false">Annuler</button>
        </div>
      </BModal>

      <!-- ── MODAL : Fin de partie ──────────────────────────────────────── -->
      <BModal v-model="gameEnded" title="Fin de partie" no-footer class="arcade-modal"
              no-close-on-backdrop no-close-on-esc centered size="lg">
        <div class="text-center py-2">

          <!-- Animation avant le tableau -->
          <div v-if="!showScoreBoard" class="gameover-animation">
            <div class="gameover-title">{{ gameOverTitle }}</div>
            <p class="gameover-sub">{{ gameOverMsg }}</p>
            <div class="loading-blocks"><span></span><span></span><span></span></div>
          </div>

          <!-- Tableau des scores -->
          <div v-else class="scoreboard">
            <h4 class="scoreboard-title">Tableau des scores</h4>

            <div class="scoreboard-list">
              <div
                  v-for="(stat, i) in gameStats"
                  :key="stat.player_pseudo"
                  class="scoreboard-row"
                  :class="{
            'scoreboard-winner':   stat.is_winner,
            'scoreboard-eliminated': stat.is_eliminated,
          }"
              >
                <!-- Rang -->
                <span class="scoreboard-rank">
            {{ i === 0 ? '🥇' : i === 1 ? '🥈' : i === 2 ? '🥉' : `#${i+1}` }}
          </span>

                <!-- Infos joueur -->
                <div class="scoreboard-player">
                  <span class="scoreboard-pseudo">{{ stat.player_pseudo }}</span>
                  <span class="scoreboard-character">{{ stat.character_name }}</span>
                </div>

                <!-- Stats détaillées -->
                <div class="scoreboard-details">
                  <span class="stat-chip">⚔️ {{ stat.eliminations }} élim.</span>
                  <span class="stat-chip">🛡️ {{ stat.rounds_survived }} rounds</span>
                  <span v-if="stat.survived_full_game" class="stat-chip stat-chip-gold">
              ⭐ Survie complète
            </span>
                </div>

                <!-- Score total -->
                <span class="scoreboard-score">{{ stat.score }} pts</span>
              </div>
            </div>

            <button type="button" class="cabinet-btn mt-4" @click="backToHome">
              Retour à l'accueil
            </button>
          </div>

        </div>
      </BModal>

      <!-- ── MODAL : Répondre à une question ───────────────────────────────── -->
      <BModal
          v-model="showAnswerModal"
          title="❓ On te pose une question !"
          no-footer
          class="arcade-modal"
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

          <!-- Piège détecté : la question contient un de MES mots interdits -->
          <div v-if="forbiddenWordsInQuestion.length" class="answer-trap">
            <p class="answer-trap-title">
              <i class="fa-solid fa-skull"></i> Mot interdit prononcé !
            </p>
            <div class="forbidden-words">
              <span
                  v-for="word in forbiddenWordsInQuestion"
                  :key="word"
                  class="pixel-chip pixel-chip--danger"
              >{{ word }}</span>
            </div>
            <p class="answer-trap-hint">
              Tu as le droit de mentir : réponds ce que tu veux, il n'en saura rien.
            </p>
          </div>

          <!-- Rappel mots interdits -->
          <div v-else class="answer-forbidden">
            <p class="forbidden-title">
              <i class="fa-solid fa-skull"></i> Tes mots interdits
            </p>
            <div class="forbidden-words">
              <span
                  v-for="word in myCharacter?.forbidden_words ?? []"
                  :key="word"
                  class="pixel-chip pixel-chip--danger"
              >{{ word }}</span>
            </div>
            <p class="answer-trap-hint">
              Aucun n'a été prononcé : réponds honnêtement.
            </p>
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

      <BModal v-model="showAccusationModal" title="🎯 Faire une accusation" no-footer class="arcade-modal" centered>
        <BAlert variant="warning" class="small">
          ⚠️ Si le joueur confirme, il est éliminé et devient spectateur. Son binôme, lui, reste en jeu et n'est pas révélé.
        </BAlert>
        <div class="mb-3">
          <label class="picker-label">Qui accuses-tu ?</label>
          <div class="target-picker" role="radiogroup" aria-label="Choisir un joueur à accuser">
            <button
                v-for="p in otherPlayers"
                :key="p.id"
                type="button"
                class="target-chip"
                :class="{ 'is-selected': accusationTarget === p.id }"
                role="radio"
                :aria-checked="accusationTarget === p.id"
                @click="accusationTarget = p.id"
            >
              <span class="target-chip__avatar">{{ p.pseudo.slice(0, 2).toUpperCase() }}</span>
              <span class="target-chip__name">{{ p.pseudo }}</span>
              <span v-if="accusationTarget === p.id" class="target-chip__check">
                <i class="fa-solid fa-check"></i>
              </span>
            </button>
          </div>
          <p v-if="!otherPlayers.length" class="picker-empty">Aucun joueur disponible.</p>
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
          <button type="button" class="play-btn play-btn--sm"
                   :disabled="!accusationTarget || !accusationCharacter.trim() || submitting"
                   @click="submitAccusation">
            <BSpinner v-if="submitting" small class="me-1"/>
            Accuser
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showAccusationModal = false">Annuler</button>
        </div>
      </BModal>

      <BModal v-model="showAccusationConfirmModal"
              title="⚔️ Tu es accusé !"
              no-footer
              class="arcade-modal"
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
  min-height: calc(100vh - var(--navbar-height));
  padding: 0 0 6rem;
  font-family: 'Baloo 2', sans-serif;
  color: var(--arcade-beige);
  max-width: 460px;
  margin: 0 auto;
  position: relative;
  text-align: left;
  border-left: 3px solid rgba(71, 87, 95, 0.6);
  border-right: 3px solid rgba(71, 87, 95, 0.6);
  box-shadow: 0 0 60px rgba(0, 0, 0, 0.8);
}

/* ─── LOADING ───────────────────────────────────────────────────────────────── */
.loading-screen {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: calc(100vh - var(--navbar-height));
  gap: 1.5rem;
}

/* Trois blocs pixel qui rebondissent (remplace le spinner arrondi) */
.loading-blocks {
  display: flex;
  gap: 0.5rem;
}

.loading-blocks span {
  width: 14px;
  height: 14px;
  background: var(--arcade-gold);
  box-shadow: 0 3px 0 rgba(0, 0, 0, 0.35);
  animation: pixel-bounce 0.9s steps(2, end) infinite;
}

.loading-blocks span:nth-child(2) { animation-delay: 0.15s; }
.loading-blocks span:nth-child(3) { animation-delay: 0.3s; }

@keyframes pixel-bounce {
  0%, 100% { transform: translateY(0); }
  50%      { transform: translateY(-10px); }
}

.loading-text {
  font-family: 'Press Start 2P', cursive;
  color: var(--arcade-taupe);
  font-size: 0.7rem;
  line-height: 1.8;
  text-align: center;
}

.caret {
  animation: caret-blink 1s steps(2, end) infinite;
}

@keyframes caret-blink {
  0%, 49%   { opacity: 1; }
  50%, 100% { opacity: 0; }
}

/* ─── ERREUR ────────────────────────────────────────────────────────────────── */
.round-error {
  margin: 0.75rem 1rem 0;
  border-radius: 6px;
}

/* ─── MARQUEE DE TOUR ───────────────────────────────────────────────────────── */
.turn-marquee {
  position: sticky;
  /* Se cale juste sous la navbar fixed-top au lieu de passer dessous */
  top: var(--navbar-height);
  z-index: 10;
  padding: 0.7rem 1rem;
  border-bottom: 4px solid var(--arcade-blue-grey-dark);
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.3);
}

.turn-marquee.is-mine {
  background:
      repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.14) 0 8px, transparent 8px 16px),
      linear-gradient(180deg, #ffd876, var(--arcade-gold));
  border-bottom-color: #a8720f;
}

.turn-marquee.is-other {
  background:
      repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.05) 0 8px, transparent 8px 16px),
      var(--arcade-blue-grey-dark);
}

.turn-marquee__inner {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.turn-marquee__icon {
  font-size: 1.15rem;
  width: 2.1rem;
  height: 2.1rem;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px solid currentColor;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.18);
}

.turn-marquee__text {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-width: 0;
}

.turn-marquee__main {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.7rem;
  line-height: 1.5;
  word-break: break-word;
}

.turn-marquee__sub {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 1px;
  opacity: 0.75;
}

.turn-marquee.is-mine .turn-marquee__inner { color: #4a2f00; }
.turn-marquee.is-other .turn-marquee__main { color: var(--arcade-beige); }
.turn-marquee.is-other .turn-marquee__sub,
.turn-marquee.is-other .turn-marquee__icon { color: var(--arcade-taupe); }

.turn-marquee__round {
  margin-left: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.2rem;
  flex-shrink: 0;
}

.turn-marquee__round-label {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.45rem;
  letter-spacing: 1px;
  text-transform: uppercase;
}

.turn-marquee__lcd {
  padding: 0.3rem 0.55rem;
  border-radius: 4px;
}

/* ─── CARTE PERSONNAGE ──────────────────────────────────────────────────────── */
.character-board {
  margin: 1.5rem 1rem;
  padding: 1.6rem 1.1rem 1.25rem;
}

.character-board__title {
  font-size: 0.75rem;
  line-height: 1.6;
  margin-bottom: 1rem;
}

.character-board__body {
  display: flex;
  gap: 1rem;
  align-items: flex-start;
}

.character-portrait {
  position: relative;
  flex-shrink: 0;
  width: 116px;
  padding-bottom: 0.5rem;
}

.character-portrait__frame {
  width: 116px;
  height: 148px;
}

.character-portrait__img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.character-portrait__placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: 'Press Start 2P', cursive;
  font-size: 2.2rem;
  color: var(--arcade-taupe);
}

.portrait-badge {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  font-family: 'Press Start 2P', cursive;
  font-size: 0.45rem;
  line-height: 1.5;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  padding: 4px 7px;
  border-radius: 4px;
  border: 2px solid;
  max-width: 130px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  z-index: 2;
}

.portrait-badge--cosmos {
  top: -9px;
  background: var(--arcade-blue-grey-dark);
  border-color: #34424a;
  color: var(--arcade-beige);
}

.portrait-badge--universe {
  bottom: -3px;
  background: linear-gradient(180deg, #ffd876, var(--arcade-gold));
  border-color: #a8720f;
  color: #4a2f00;
}

.character-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 0.75rem;
}

.character-name {
  font-size: 1.35rem;
  font-weight: 800;
  color: var(--arcade-blue-grey-dark);
  margin: 0;
  line-height: 1.15;
  word-break: break-word;
  text-shadow: 2px 2px 0 rgba(158, 139, 127, 0.35);
}

.forbidden-title {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.5rem;
  line-height: 1.6;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: var(--arcade-danger-dark);
  margin: 0 0 0.5rem;
}

.forbidden-words {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

/* ─── FLOU ──────────────────────────────────────────────────────────────────── */
.blurred {
  filter: blur(8px);
  user-select: none;
  transition: filter 0.3s ease;
}

.character-board__head {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 0.5rem;
}

.reveal-btn {
  font-family: 'Baloo 2', sans-serif;
  font-weight: 700;
  font-size: 0.7rem;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  padding: 4px 9px;
  border-radius: 6px;
  border: 2px solid var(--arcade-blue-grey-dark);
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
  cursor: pointer;
  box-shadow: 0 3px 0 var(--arcade-blue-grey-dark);
  transition: transform 0.08s ease, box-shadow 0.08s ease;
}

.reveal-btn:active {
  transform: translateY(3px);
  box-shadow: 0 0 0 var(--arcade-blue-grey-dark);
}

/* ─── NOTIFICATION BINÔME ───────────────────────────────────────────────────── */
.binome-notif {
  margin: 0 1rem 1.25rem;
  background: linear-gradient(180deg, rgba(63, 122, 78, 0.35), rgba(28, 34, 38, 0.9));
  border: 3px solid var(--arcade-success);
  border-radius: 12px;
  padding: 0.75rem 0.9rem;
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  box-shadow: 0 5px 0 var(--arcade-success-dark);
  animation: slideIn 0.3s ease;
}

.binome-notif__icon {
  flex-shrink: 0;
  width: 1.9rem;
  height: 1.9rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px solid var(--arcade-success);
  border-radius: 6px;
  color: #a9dab5;
}

.binome-notif__title {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.55rem;
  line-height: 1.6;
  color: #b9e6c4;
  margin: 0 0 0.35rem;
}

.binome-notif__sub {
  font-size: 0.8rem;
  color: #8ac298;
  margin: 0;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }
}

/* ─── BOUTONS D'ACTION ──────────────────────────────────────────────────────── */
.actions-section {
  margin: 0 1rem 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.action-cabinet {
  width: 100%;
  justify-content: flex-start;
  padding: 0.9rem 1.1rem;
  gap: 0.9rem;
}

.action-cabinet__label {
  font-size: 1rem;
  font-weight: 700;
}

.action-waiting {
  font-family: 'Press Start 2P', cursive;
  text-align: center;
  padding: 1rem 0.9rem;
  color: var(--arcade-taupe);
  font-size: 0.55rem;
  line-height: 2;
  background: var(--arcade-dark-2);
  border: 3px solid var(--arcade-blue-grey-dark);
  border-radius: 10px;
  box-shadow: inset 0 2px 8px rgba(0, 0, 0, 0.6);
}

/* ─── JOUEURS ───────────────────────────────────────────────────────────────── */
.players-board {
  margin: 0 1rem;
  padding: 1.5rem 1.1rem 1.25rem;
}

.players-board .board-card__title {
  font-size: 0.75rem;
  line-height: 1.6;
  margin-bottom: 1.1rem;
}

.players-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
  gap: 0.75rem;
}

/* ─── HISTORIQUE DES ACTIONS ────────────────────────────────────────────────── */
.actions-history {
  margin: 0 1rem 1.5rem;
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  max-height: 240px;
  overflow-y: auto;
  padding-right: 0.25rem;
}

.history-list::-webkit-scrollbar {
  width: 4px;
}

.history-list::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.3);
}

.history-list::-webkit-scrollbar-thumb {
  background: var(--arcade-taupe);
}

.history-item {
  font-family: 'Baloo 2', sans-serif;
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  padding: 0.5rem 0.7rem;
  border-radius: 6px;
  font-size: 0.8rem;
  border-left: 4px solid;
  line-height: 1.4;
  min-width: 0;
}

.history-question-valid {
  background: rgba(90, 111, 125, 0.28);
  border-color: var(--arcade-blue-grey);
  color: #cdd9e0;
}

.history-question-invalid {
  background: rgba(179, 69, 63, 0.28);
  border-color: var(--arcade-danger);
  color: #f0b8b5;
}

.history-accusation-ok {
  background: rgba(63, 122, 78, 0.28);
  border-color: var(--arcade-success);
  color: #b9e6c4;
}

.history-accusation-ko {
  background: rgba(179, 69, 63, 0.28);
  border-color: var(--arcade-danger);
  color: #f0b8b5;
}

.history-icon {
  flex-shrink: 0;
  font-size: 0.9rem;
  margin-top: 1px;
}

.history-actor {
  font-weight: bold;
  color: var(--arcade-gold);
}

.history-target {
  font-weight: bold;
  color: var(--arcade-beige);
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

/* ─── SÉPARATEUR DE ROUND ───────────────────────────────────────────────────── */
.history-round-separator {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin: 0.5rem 0 0.25rem;
}

.history-round-line {
  flex: 1;
  height: 2px;
  background: repeating-linear-gradient(90deg, rgba(224, 163, 28, 0.35) 0 4px, transparent 4px 8px);
}

.history-round-badge {
  flex-shrink: 0;
}

/* ─── SÉLECTION D'UN JOUEUR (remplace les <select>, pensé pour le tactile) ──── */
.picker-label {
  display: block;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.5rem;
  line-height: 1.7;
  text-transform: uppercase;
  color: var(--arcade-blue-grey-dark);
  margin-bottom: 0.6rem;
}

.target-picker {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 0.5rem;
}

.target-chip {
  position: relative;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  /* 48px de haut minimum : cible tactile confortable au pouce */
  min-height: 48px;
  padding: 0.5rem 0.6rem;
  text-align: left;
  font-family: 'Baloo 2', sans-serif;
  background: #fff;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 10px;
  box-shadow: 0 3px 0 rgba(0, 0, 0, 0.18);
  cursor: pointer;
  transition: transform 0.08s ease, box-shadow 0.08s ease, background 0.12s ease;
}

.target-chip:active {
  transform: translateY(3px);
  box-shadow: none;
}

.target-chip.is-selected {
  background: #fff6de;
  border-color: var(--arcade-gold);
  box-shadow: 0 3px 0 #a8720f;
}

.target-chip__avatar {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.6rem;
  background: var(--arcade-taupe);
  color: #fff;
  box-shadow: inset 0 -3px 0 rgba(0, 0, 0, 0.2);
}

.target-chip.is-selected .target-chip__avatar {
  background: var(--arcade-gold);
  color: #4a2f00;
}

.target-chip__name {
  flex: 1;
  min-width: 0;
  font-weight: 700;
  font-size: 0.9rem;
  line-height: 1.2;
  color: var(--arcade-blue-grey-dark);
  /* Les pseudos longs passent sur 2 lignes plutôt que d'être tronqués trop tôt */
  overflow-wrap: anywhere;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.target-chip__check {
  position: absolute;
  top: -7px;
  right: -7px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--arcade-success);
  color: #fff;
  border: 2px solid #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.6rem;
}

.picker-empty {
  margin: 0.5rem 0 0;
  font-size: 0.85rem;
  font-style: italic;
  color: var(--arcade-taupe);
}

/* ─── MODALE RÉPONSE ────────────────────────────────────────────────────────── */
.answer-modal-body {
  font-family: 'Baloo 2', sans-serif;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  max-height: 70vh; /* ← ne jamais dépasser 70% de l'écran */
  overflow-y: auto; /* ← scroll interne si trop long */
}

.answer-question-box {
  background: rgba(90, 111, 125, 0.1);
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 8px;
  padding: 0.75rem;
}

.answer-from {
  font-size: 0.8rem;
  color: var(--arcade-taupe);
  margin: 0 0 0.4rem;
}

.answer-question-text {
  font-size: 1rem;
  color: var(--arcade-blue-grey-dark);
  font-style: italic;
  margin: 0;
  word-break: break-word; /* ← empêche le texte long de casser le layout */
  overflow-wrap: break-word;
  white-space: normal;
}

.answer-forbidden {
  background: rgba(179, 69, 63, 0.1);
  border: 2px solid var(--arcade-danger);
  border-radius: 8px;
  padding: 0.65rem 0.75rem;
}

.answer-trap {
  background: rgba(179, 69, 63, 0.22);
  border: 3px solid var(--arcade-danger);
  border-radius: 8px;
  padding: 0.65rem 0.75rem;
}

.answer-trap-title {
  font-family: 'Baloo 2', sans-serif;
  font-weight: 800;
  font-size: 0.95rem;
  color: var(--arcade-danger);
  margin: 0 0 0.45rem;
  letter-spacing: 0.02em;
}

.answer-trap-hint {
  font-size: 0.8rem;
  color: var(--arcade-blue-grey-dark);
  margin: 0.5rem 0 0;
  line-height: 1.4;
}

.answer-buttons {
  display: flex;
  gap: 0.5rem;
}

.answer-btn {
  font-family: 'Baloo 2', sans-serif;
  flex: 1;
  padding: 0.75rem 0.25rem;
  border-radius: 10px;
  border: 3px solid;
  font-size: 0.85rem;
  font-weight: bold;
  cursor: pointer;
  transition: transform 0.08s ease, box-shadow 0.08s ease, background 0.15s;
  background: var(--arcade-beige);
  white-space: nowrap;
}

.answer-btn:active:not(:disabled) {
  transform: translateY(3px);
  box-shadow: none;
}

.answer-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.answer-btn-yes {
  border-color: var(--arcade-success);
  color: var(--arcade-success-dark);
  box-shadow: 0 4px 0 var(--arcade-success-dark);
}

.answer-btn-no {
  border-color: var(--arcade-danger);
  color: var(--arcade-danger-dark);
  box-shadow: 0 4px 0 var(--arcade-danger-dark);
}

.answer-btn-dont-know {
  border-color: var(--arcade-taupe);
  color: var(--arcade-taupe);
  box-shadow: 0 4px 0 #7c6c62;
}

/* ─── RÉPONSE DANS L'HISTORIQUE ─────────────────────────────────────────────── */
.answer-yes {
  color: #8fdb9f;
  font-weight: bold;
}

.answer-no {
  color: #ef9a96;
  font-weight: bold;
}

.answer-dont-know {
  color: var(--arcade-gold);
  font-weight: bold;
}

/* ─── TRANSITION ROUND ──────────────────────────────────────────────────────── */
.round-transition-overlay {
  position: absolute;
  inset: 0;
  z-index: 1000;
  background: rgba(36, 36, 36, 0.94);
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
  gap: 0.5rem;
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
  font-family: 'Press Start 2P', cursive;
  font-size: 0.7rem;
  letter-spacing: 0.4em;
  color: var(--arcade-gold);
  text-transform: uppercase;
}

.round-transition-number {
  font-size: 5rem;
  color: var(--arcade-beige);
  font-family: 'Press Start 2P', cursive;
  line-height: 1;
  text-shadow:
      4px 4px 0 var(--arcade-taupe),
      0 0 40px rgba(224, 163, 28, 0.6),
      0 0 80px rgba(224, 163, 28, 0.3);
}

/* ─── FIN DE PARTIE ANIMATION ───────────────────────────────────────────────── */
.gameover-animation {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.25rem;
  padding: 2rem 0;
}

.gameover-title {
  font-family: 'Press Start 2P', cursive;
  font-size: 1.3rem;
  line-height: 1.5;
  color: var(--arcade-gold);
  text-shadow: 3px 3px 0 rgba(0, 0, 0, 0.3);
  animation: titlePulse 1s ease infinite alternate;
}

@keyframes titlePulse {
  from { text-shadow: 3px 3px 0 rgba(0, 0, 0, 0.3), 0 0 20px rgba(224, 163, 28, 0.4); }
  to   { text-shadow: 3px 3px 0 rgba(0, 0, 0, 0.3), 0 0 60px rgba(224, 163, 28, 0.9); }
}

.gameover-sub {
  font-family: 'Baloo 2', sans-serif;
  color: var(--arcade-taupe);
  font-size: 0.95rem;
  font-weight: 600;
  text-align: center;
  margin: 0;
}

/* ─── TABLEAU DES SCORES ────────────────────────────────────────────────────── */
.scoreboard { text-align: left; font-family: 'Baloo 2', sans-serif; }

.scoreboard-title {
  text-align: center;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.8rem;
  line-height: 1.7;
  color: var(--arcade-blue-grey-dark);
  margin-bottom: 1.25rem;
}

.scoreboard-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.scoreboard-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.65rem 0.75rem;
  background: #fff;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 10px;
  box-shadow: 0 3px 0 rgba(0, 0, 0, 0.18);
  flex-wrap: wrap;
}

.scoreboard-winner {
  background: #fff6de;
  border-color: var(--arcade-gold);
  box-shadow: 0 3px 0 #a8720f;
}

.scoreboard-eliminated {
  opacity: 0.55;
  filter: grayscale(0.4);
}

.scoreboard-rank   { font-size: 1.2rem; flex-shrink: 0; }
.scoreboard-player { display: flex; flex-direction: column; flex: 1; min-width: 0; }

.scoreboard-pseudo {
  font-weight: 800;
  color: var(--arcade-blue-grey-dark);
  font-size: 0.95rem;
}

.scoreboard-character {
  font-size: 0.75rem;
  color: var(--arcade-taupe);
  font-style: italic;
}

.scoreboard-details {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}

.stat-chip {
  font-size: 0.7rem;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(90, 111, 125, 0.12);
  border: 1px solid var(--arcade-blue-grey);
  color: var(--arcade-blue-grey-dark);
  font-family: 'Baloo 2', sans-serif;
  white-space: nowrap;
}

.stat-chip-gold {
  background: rgba(224, 163, 28, 0.2);
  border-color: var(--arcade-gold);
  color: #8a6410;
}

.scoreboard-score {
  font-family: 'Press Start 2P', cursive;
  color: var(--arcade-blue-grey-dark);
  font-size: 0.7rem;
  flex-shrink: 0;
}

.scoreboard-winner .scoreboard-score {
  color: #8a6410;
}

/* ─── PETITS ÉCRANS ─────────────────────────────────────────────────────────── */
@media (max-width: 380px) {
  .character-board__body {
    flex-direction: column;
    align-items: center;
  }

  .character-info {
    width: 100%;
  }

  .character-name {
    text-align: center;
  }

  .forbidden-words {
    justify-content: center;
  }

  .forbidden-title {
    text-align: center;
  }
}
</style>
