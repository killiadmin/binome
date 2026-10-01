<script setup>
import {ref, computed, onMounted, onUnmounted} from 'vue'
import {useRoute, useRouter} from 'vue-router'
import {gameService} from '../../services/gameService'
import {useReverb, resetEcho} from '../../sockets/useReverb.js'
import {useGameNotes} from '../../composables/useGameNotes'
import QuickReactions from '../../components/game/QuickReactions.vue'
import GameNotepad from '../../components/game/GameNotepad.vue'
import GameRecap from '../../components/game/GameRecap.vue'
import PlayerAvatar from '../../components/player/PlayerAvatar.vue'
import RoundTransition from '../../components/game/RoundTransition.vue'
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

// Erreur affichée DANS la modale d'action : la bannière `error` de la page est
// masquée par le backdrop tant qu'une modale est ouverte, le joueur ne la voit pas.
const modalError = ref(null)

// Doit rester aligné sur PlayQuestionRequest (min:5) côté backend.
const QUESTION_MIN = 5
const QUESTION_MAX = 200
const CHARACTER_MAX = 100

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
let finishRoundTransition = null

const pendingAccusation    = ref(null)
const showAccusationConfirmModal = ref(false)
const submittingConfirm    = ref(false)

const gameStats       = ref([])
const showScoreBoard  = ref(false)
const recap           = ref(null)

// Fiche joueur : consultation des échanges adressés à un joueur donné
const selectedPlayerId = ref(null)
const showPlayerSheet  = ref(false)

// Nombre de joueurs impair : un orphelin joue sans binôme. Tout le monde sait
// qu'il existe, personne ne sait qui c'est — pas même lui, jusqu'à la fin.
const hasOrphan      = ref(false)
const orphanPseudos  = ref(new Set())

// Bloc-notes privé (localStorage uniquement, voir composables/useGameNotes.js)
const notes = useGameNotes(gameId, myPlayerId.value)
const showNotepad = ref(false)

// Réactions rapides reçues, en cours d'animation
const reactions = ref([])
let reactionSeq = 0

// ─── PRÉSENCE / JOUEURS DÉCONNECTÉS ───────────────────────────────────────────

// Membres connectés au channel game.{id}, joueurs et spectateurs : id → pseudo
const members = ref(new Map())
// id → horodatage de la déconnexion. Absent = en ligne.
const offlineSince = ref({})
const leaveTimers = new Map()
const now = ref(Date.now())
let clock = null

// Un rechargement de page produit un leaving suivi d'un joining : on attend un
// peu avant d'afficher le joueur hors ligne.
const LEAVE_GRACE_MS = 3000
// Délai avant de proposer à l'hôte de passer son tour / de l'exclure : évite
// une décision hâtive sur un simple rechargement ou un écran verrouillé.
const AFK_GRACE_SECONDS = 30

const AFK_REASONS = {
  turn:       "C'est son tour de jouer",
  answer:     'Il doit répondre à une question',
  accusation: 'Il doit confirmer une accusation',
}

// L'hôte du salon décide pour un joueur déconnecté (GET /games/{game} → host_id)
const hostId = ref(null)
const skipping = ref(false)
const excluding = ref(false)
const excludeTarget = ref(null)   // joueur dont on confirme l'exclusion
const showExcludeModal = ref(false)

// Message éphémère (tour passé, réactions trop rapides…)
const notice = ref(null)
let noticeTimer = null

// WebSocket coupé (écran verrouillé, Wi-Fi qui saute) : Pusher se reconnecte
// seul, mais les events émis pendant la coupure sont perdus → resync().
const connectionLost = ref(false)
let stopConnectionWatch = null
let hiddenAt = null
let syncing = false

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

const trimmedQuestion = computed(() => questionText.value.trim())

const questionTooShort = computed(() => trimmedQuestion.value.length < QUESTION_MIN)

const eliminatedPlayerIds = computed(() => {
  const ids = new Set()
  players.value.forEach(p => { if (p.is_eliminated) ids.add(p.id) })
  return ids
})

// Spectateur : membre du salon sans personnage (arrivé après le lancement).
// Un joueur éliminé garde sa carte mais ne joue plus.
const isSpectator = computed(() =>
    !loading.value && !players.value.some(p => p.id === myPlayerId.value)
)

const amEliminated = computed(() => eliminatedPlayerIds.value.has(myPlayerId.value))

function isOnline(playerId) {
  return !(playerId in offlineSince.value)
}

const spectators = computed(() =>
    [...members.value]
        .filter(([id]) => !players.value.some(p => p.id === id))
        .map(([id, pseudo]) => ({id, pseudo}))
)

// Qui la partie attend-elle ? Miroir de ActionService::findBlocker() côté serveur.
const blocker = computed(() => {
  if (gameEnded.value || !currentRound.value) return null
  const cp = currentPlayerId.value
  const last = actions.value
      .filter(a => a.round_id === currentRound.value.id && a.player?.id === cp)
      .at(-1)

  if (!last) return {playerId: cp, reason: 'turn'}
  if (last.type === 'question' && last.is_valid && last.answer == null) {
    return {playerId: last.target_player?.id, reason: 'answer'}
  }
  if (last.type === 'accusation' && last.accusation_confirmed == null) {
    return {playerId: last.target_player?.id, reason: 'accusation'}
  }
  return null
})

const isHost = computed(() => hostId.value !== null && hostId.value === myPlayerId.value)
const hostPseudo = computed(() =>
    players.value.find(p => p.id === hostId.value)?.pseudo ?? members.value.get(hostId.value) ?? "l'hôte"
)

// Le joueur attendu est hors ligne : la partie est en pause. Seul l'hôte
// décide, après AFK_GRACE_SECONDS : passer son tour (sauf s'il doit répondre à
// une question — une réponse est obligatoire) ou l'exclure. Le serveur
// revérifie la présence via Reverb.
const afkBlocker = computed(() => {
  const b = blocker.value
  if (!b?.playerId || isOnline(b.playerId)) return null
  const player = players.value.find(p => p.id === b.playerId)
  if (!player) return null
  const seconds = Math.max(0, Math.floor((now.value - offlineSince.value[b.playerId]) / 1000))
  const decides = isHost.value && b.playerId !== myPlayerId.value
  return {
    ...b,
    player,
    seconds,
    waitLeft: Math.max(0, AFK_GRACE_SECONDS - seconds),
    canSkip: decides && b.reason !== 'answer',
    canExclude: decides,
    hostOffline: hostId.value !== null && !isOnline(hostId.value),
  }
})

const amExcluded = computed(() =>
    !!players.value.find(p => p.id === myPlayerId.value)?.is_excluded
)

// Photo d'un joueur par id (panneau « Historique de la partie »)
const avatarById = computed(() =>
    Object.fromEntries(players.value.map(p => [p.id, p.avatar_url ?? null]))
)

const selectedPlayer = computed(() =>
    players.value.find(p => p.id === selectedPlayerId.value) ?? null
)

// Fil « conversation » d'un joueur : toutes les actions qui lui ont été adressées,
// de la plus ancienne à la plus récente (actions.value est déjà chronologique).
// `showRound` ne vaut true qu'au changement de round, pour n'afficher le
// séparateur qu'une fois par round.
const selectedPlayerThread = computed(() => {
  if (!selectedPlayerId.value) return []
  let lastRound = null
  return actions.value
      .filter(a => a.target_player?.id === selectedPlayerId.value)
      .map((action, i) => {
        const round = action.round_number ?? action.round_id
        const showRound = round !== lastRound
        lastRound = round
        return {action, key: action.action_id ?? action.id ?? i, round, showRound}
      })
})

// Nombre d'échanges reçus par joueur, affiché sur son jeton dans la liste
const threadCountByPlayer = computed(() => {
  const counts = {}
  actions.value.forEach(a => {
    const id = a.target_player?.id
    if (id) counts[id] = (counts[id] ?? 0) + 1
  })
  return counts
})

// ─── INIT ─────────────────────────────────────────────────────────────────────

// État complet de la partie depuis l'API. Sert au chargement initial et à la
// resynchronisation après une coupure (events manqués pendant la déconnexion).
async function loadGameState() {
  const game = await gameService.show(gameId)
  actions.value = game.actions ?? []

  // Le backend renvoie les joueurs à plat : la composition des binômes n'est
  // exposée qu'une fois un binôme découvert (sinon l'orphelin serait trahi).
  players.value = (game.players ?? []).map(p => ({
    ...p,
    is_eliminated: p.is_eliminated ?? false,
  }))
  hasOrphan.value = !!game.has_orphan
  hostId.value = game.host_id ?? null
  currentRound.value = game.current_round ?? null
  currentPlayerId.value = game.current_round?.current_player_id ?? null
  availableCharacters.value = game.characters ?? []
  discoveredBinomes.value = (game.binomes ?? [])
      .map(b => ({
        player1_id: b.players[0]?.id,
        player2_id: b.players[1]?.id,
      }))

  hasPlayed.value = !!currentRound.value && actions.value.some(a =>
      a.round_id === currentRound.value.id && a.player?.id === myPlayerId.value
  )

  // Un spectateur n'a pas de personnage : /me répondrait 404.
  const inGame = players.value.some(p => p.id === myPlayerId.value)
  if (inGame && !myCharacter.value) {
    myCharacter.value = await gameService.myCharacter(gameId, myPlayerId.value)
  }

  restorePendingPrompts()

  if (game.status === 'finished' && !gameEnded.value) {
    await handleGameEnded(null, {animate: false})
  }
}

// Rouvre (ou referme) les modales qui attendent une réponse de MA part.
function restorePendingPrompts() {
  const me = myPlayerId.value

  const question = actions.value.find(a =>
      a.type === 'question' && a.is_valid && a.target_player?.id === me && a.answer == null
  )
  pendingQuestion.value = question ?? null
  showAnswerModal.value = !!question

  const accusation = actions.value.find(a =>
      a.type === 'accusation' && a.target_player?.id === me && a.accusation_confirmed == null
  )
  pendingAccusation.value = accusation ?? null
  showAccusationConfirmModal.value = !!accusation
}

async function resync() {
  if (syncing) return
  syncing = true
  try {
    await loadGameState()
  } catch {
    /* la prochaine reconnexion ou le prochain retour sur l'onglet réessaiera */
  } finally {
    syncing = false
  }
}

function watchConnection(onConnectionChange) {
  let connectedOnce = false
  stopConnectionWatch = onConnectionChange(({current}) => {
    if (current === 'connected') {
      if (connectedOnce) resync()
      connectedOnce = true
      connectionLost.value = false
    } else if (connectedOnce) {
      connectionLost.value = true
    }
  })
}

// Téléphone verrouillé puis rallumé : la socket peut se croire encore
// connectée alors que des events ont été perdus. On resynchronise au retour.
function handleVisibilityChange() {
  if (document.visibilityState === 'hidden') {
    hiddenAt = Date.now()
    return
  }
  if (hiddenAt && Date.now() - hiddenAt > 3000) resync()
  hiddenAt = null
}

onMounted(async () => {
  if (!gameId || !myPlayerId.value) {
    router.push({name: 'Home'})
    return
  }

  try {
    await loadGameState()
  } catch (e) {
    error.value = 'Impossible de charger la partie.'
  } finally {
    loading.value = false
  }

  resetEcho()
  const {joinGame, onConnectionChange} = useReverb(myPlayerId.value)
  watchConnection(onConnectionChange)
  joinGame(gameId, {
    onHere:               handleHere,
    onJoining:            handleJoining,
    onLeaving:            handleLeaving,
    onRoundStarted:       handleRoundStarted,
    onActionPlayed:       handleActionPlayed,
    onAnswerGiven:        handleAnswerGiven,
    onAccusationConfirmed: handleAccusationConfirmed,
    onBinomeDiscovered:   handleBinomeDiscovered,
    onGameEnded:          handleGameEnded,
    onPlayerEliminated: handlePlayerEliminated,
    onReactionSent:       handleReactionSent,
    onTurnSkipped:        handleTurnSkipped,
    onPlayerExcluded:     handlePlayerExcluded,
    onPlayerAvatarUpdated: (data) => {
      players.value = players.value.map(p => p.id === data.player_id ? {...p, avatar_url: data.avatar_url} : p)
    },
    onError: () => { error.value = 'Connexion WebSocket perdue.' },
  })

  clock = setInterval(() => { now.value = Date.now() }, 1000)
  document.addEventListener('visibilitychange', handleVisibilityChange)
})

onUnmounted(() => {
  stopConnectionWatch?.()
  clearInterval(clock)
  clearTimeout(noticeTimer)
  leaveTimers.forEach(timer => clearTimeout(timer))
  document.removeEventListener('visibilitychange', handleVisibilityChange)
  const {leaveGame} = useReverb(myPlayerId.value)
  leaveGame(gameId)
})


// ─── WEBSOCKET HANDLERS ───────────────────────────────────────────────────────

function handleHere(list) {
  members.value = new Map(list.map(m => [m.id, m.pseudo]))
  const stamp = Date.now()
  const offline = {}
  players.value.forEach(p => {
    if (!members.value.has(p.id)) offline[p.id] = offlineSince.value[p.id] ?? stamp
  })
  offlineSince.value = offline
}

function handleJoining(member) {
  clearTimeout(leaveTimers.get(member.id))
  leaveTimers.delete(member.id)
  members.value = new Map(members.value).set(member.id, member.pseudo)
  const {[member.id]: _, ...rest} = offlineSince.value
  offlineSince.value = rest
}

function handleLeaving(member) {
  clearTimeout(leaveTimers.get(member.id))
  leaveTimers.set(member.id, setTimeout(() => {
    leaveTimers.delete(member.id)
    const next = new Map(members.value)
    next.delete(member.id)
    members.value = next
    offlineSince.value = {...offlineSince.value, [member.id]: Date.now() - LEAVE_GRACE_MS}
  }, LEAVE_GRACE_MS))
}

function handleReactionSent(data) {
  const id = ++reactionSeq
  reactions.value.push({id, emoji: data.emoji, pseudo: data.player?.pseudo, x: 12 + Math.random() * 76})
  // Plafond : une rafale ne doit pas couvrir l'écran
  if (reactions.value.length > 12) reactions.value.shift()
  setTimeout(() => {
    reactions.value = reactions.value.filter(r => r.id !== id)
  }, 2700)
}

async function sendReaction(emoji) {
  try {
    await gameService.react(gameId, myPlayerId.value, emoji)
  } catch (e) {
    if (e.response?.status === 429) showNotice('Doucement sur les réactions 😅')
  }
}

function handleTurnSkipped(data) {
  // Reçu par le joueur « déconnecté » lui-même s'il revient pile à ce moment.
  if (data.player?.id === myPlayerId.value && data.reason === 'accusation') {
    showAccusationConfirmModal.value = false
    pendingAccusation.value = null
  }

  const outcome = data.reason === 'accusation'
      ? "l'accusation est tranchée automatiquement"
      : 'son tour est passé'

  showNotice(`⏭️ ${data.player?.pseudo} est déconnecté : ${outcome} (décision de l'hôte).`)
}

function handlePlayerExcluded(data) {
  const removed = [data.player?.id, data.partner?.id].filter(Boolean)
  players.value = players.value.map(p => removed.includes(p.id)
      ? {...p, is_eliminated: true, is_excluded: p.id === data.player?.id || p.is_excluded}
      : p)

  // Question / accusation devenue sans objet : retirée, son auteur rejoue.
  if (data.cancelled_action_id) {
    const cancelled = actions.value.find(a => (a.action_id ?? a.id) === data.cancelled_action_id)
    actions.value = actions.value.filter(a => (a.action_id ?? a.id) !== data.cancelled_action_id)
    if (cancelled?.player?.id === myPlayerId.value) hasPlayed.value = false
    if ((pendingQuestion.value?.action_id ?? pendingQuestion.value?.id) === data.cancelled_action_id) {
      showAnswerModal.value = false
      pendingQuestion.value = null
    }
    if ((pendingAccusation.value?.action_id ?? pendingAccusation.value?.id) === data.cancelled_action_id) {
      showAccusationConfirmModal.value = false
      pendingAccusation.value = null
    }
  }

  showNotice(data.partner
      ? `🚪 ${data.player.pseudo} a été exclu par l'hôte : son binôme ${data.partner.pseudo} est éliminé avec lui.`
      : `🚪 ${data.player.pseudo} a été exclu par l'hôte.`)
}

function askExclude(player) {
  excludeTarget.value = player
  showExcludeModal.value = true
}

async function confirmExclude() {
  if (excluding.value || !excludeTarget.value) return
  excluding.value = true
  error.value = null
  try {
    await gameService.excludePlayer(gameId, excludeTarget.value.id, myPlayerId.value)
  } catch (e) {
    error.value = apiErrorMessage(e, "Impossible d'exclure ce joueur.")
  } finally {
    excluding.value = false
    showExcludeModal.value = false
    excludeTarget.value = null
  }
}

function showNotice(text) {
  notice.value = text
  clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => { notice.value = null }, 5000)
}

async function skipBlockedTurn() {
  if (skipping.value) return
  skipping.value = true
  error.value = null
  try {
    await gameService.skipTurn(gameId, myPlayerId.value)
  } catch (e) {
    error.value = apiErrorMessage(e, 'Impossible de passer ce tour.')
  } finally {
    skipping.value = false
  }
}

function formatDuration(seconds) {
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return m ? `${m} min ${String(s).padStart(2, '0')} s` : `${s} s`
}

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
  // Répondue ailleurs (autre onglet, tour passé) : la modale n'a plus lieu d'être.
  if ((pendingQuestion.value?.action_id ?? pendingQuestion.value?.id) === data.action_id) {
    showAnswerModal.value = false
    pendingQuestion.value = null
  }
}

// Résolue par l'event `done` de RoundTransition (la durée vit dans le composant).
async function playRoundTransition(roundNumber) {
  finishRoundTransition?.()
  transitionRoundNumber.value = roundNumber
  showRoundTransition.value = true
  let finish
  await new Promise(resolve => { finish = finishRoundTransition = resolve })
  // Un round plus récent a pu prendre la main entre-temps : on ne le coupe pas.
  if (finishRoundTransition !== finish) return
  finishRoundTransition = null
  showRoundTransition.value = false
}

function onRoundTransitionDone() {
  finishRoundTransition?.()
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

// GameEnded est diffusé depuis la transaction qui clôt la partie : le récap
// peut répondre 409 (« pas terminée ») quelques instants, d'où les reprises.
async function fetchRecap(attempts = 3) {
  for (let i = 0; i < attempts; i++) {
    try {
      return await gameService.recap(gameId)
    } catch (e) {
      if (e.response?.status !== 409 || i === attempts - 1) return null
      await new Promise(r => setTimeout(r, 800))
    }
  }
  return null
}

// data : payload GameEnded, ou null quand on découvre la fin au chargement
// (rechargement de page, reconnexion après la fin).
async function handleGameEnded(data, {animate = true} = {}) {
  if (gameEnded.value) return
  gameEnded.value = true
  showAnswerModal.value = false
  showAccusationConfirmModal.value = false
  showQuestionModal.value = false
  showAccusationModal.value = false

  recap.value = await fetchRecap()
  gameStats.value = recap.value?.stats ?? data?.stats ?? []

  const winnerPseudos = data?.winners?.map(w => w.pseudo) ?? recap.value?.winners ?? []

  // Tout est révélé à la fin, orphelin compris.
  orphanPseudos.value = new Set(
      (recap.value?.binomes ?? data?.all_binomes ?? [])
          .filter(b => b.is_orphan)
          .flatMap(b => (b.players ?? []).map(p => p.pseudo))
  )

  const myPseudo   = players.value.find(p => p.id === myPlayerId.value)?.pseudo
  const iWasOrphan = !!myPseudo && orphanPseudos.value.has(myPseudo)
  const iWon = !!myPseudo && winnerPseudos.includes(myPseudo)

  if (isSpectator.value) {
    gameOverTitle.value = '🏁 Partie terminée'
    gameOverMsg.value = winnerPseudos.length
        ? `Victoire de ${winnerPseudos.join(' & ')} !`
        : 'Personne ne l’emporte.'
  } else {
    gameOverTitle.value = iWon ? '🏆 Victoire !' : '💀 Défaite'

    if (iWasOrphan) {
      gameOverMsg.value = iWon
          ? "Tu étais l'orphelin : seul depuis le début, et dernier debout. Tu marques comme un binôme entier !"
          : "Tu étais l'orphelin : tu jouais seul depuis le début, sans le savoir."
    } else {
      gameOverMsg.value = iWon
          ? "Votre binôme n'a jamais été découvert. Bien joué !"
          : 'Votre binôme a été découvert. Meilleure chance la prochaine fois !'
    }
  }

  if (animate) await new Promise(r => setTimeout(r, 4000))
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
  if ((pendingAccusation.value?.action_id ?? pendingAccusation.value?.id) === data.action_id) {
    showAccusationConfirmModal.value = false
    pendingAccusation.value = null
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

// Message lisible à partir d'une erreur API : une 422 Laravel porte le détail
// dans `errors` (un tableau de messages par champ), les autres cas n'ont que
// `message`.
function apiErrorMessage(e, fallback) {
  const data = e.response?.data
  const fieldErrors = Object.values(data?.errors ?? {}).flat()
  if (fieldErrors.length) return fieldErrors.join(' ')
  return data?.message || fallback
}

async function submitQuestion() {
  if (!questionTarget.value || submitting.value) return

  // Même règle que le backend, mais expliquée avant l'envoi : sinon le refus 422
  // n'apparaissait que dans la console.
  if (questionTooShort.value) {
    modalError.value = `La question doit faire au moins ${QUESTION_MIN} caractères.`
    return
  }

  submitting.value = true
  modalError.value = null
  error.value = null
  try {
    await gameService.playQuestion(gameId, currentRound.value.id, myPlayerId.value, questionTarget.value, trimmedQuestion.value)
    showQuestionModal.value = false
    resetForms()
  } catch (e) {
    modalError.value = apiErrorMessage(e, "Erreur lors de l'envoi de la question.")
  } finally {
    submitting.value = false
  }
}

async function submitAccusation() {
  if (!accusationTarget.value || !accusationCharacter.value.trim() || submitting.value) return
  submitting.value = true
  modalError.value = null
  error.value = null
  try {
    await gameService.playAccusation(gameId, currentRound.value.id, myPlayerId.value, accusationTarget.value, accusationCharacter.value.trim())
    showAccusationModal.value = false
    resetForms()
  } catch (e) {
    modalError.value = apiErrorMessage(e, "Erreur lors de l'accusation.")
  } finally {
    submitting.value = false
  }
}

// Ouvre une modale d'action en repartant d'un état d'erreur propre
function openActionModal(which) {
  modalError.value = null
  if (which === 'question') showQuestionModal.value = true
  else showAccusationModal.value = true
}

// ─── FICHE JOUEUR ─────────────────────────────────────────────────────────────

function openPlayerSheet(playerId) {
  selectedPlayerId.value = playerId
  showPlayerSheet.value = true
}

const ANSWER_LABELS = {
  yes:       'Oui ✅',
  no:        'Non ❌',
  dont_know: 'Je ne sais pas 🤷',
}

function answerLabel(answer) {
  return ANSWER_LABELS[answer] ?? answer
}

// Résultat d'une accusation, tolérant aux deux formes de payload :
// le broadcast envoie accusation_confirmed, GET /games/{game} ne l'expose pas.
function accusationOutcome(action) {
  const confirmed = action.accusation_confirmed ?? null
  const correct   = action.accusation_correct ?? null

  if (confirmed === null && correct === null) {
    return {icon: '⏳', tone: 'pending', text: 'en attente de confirmation…'}
  }
  if (correct) {
    return {icon: '🎯', tone: 'ok', text: 'accusation correcte !'}
  }
  if (confirmed === false) {
    return {icon: '🛡️', tone: 'ko', text: 'nié par le joueur.'}
  }
  return {icon: '❌', tone: 'ko', text: 'mauvaise accusation.'}
}

function resetForms() {
  questionTarget.value = ''
  questionText.value = ''
  accusationTarget.value = ''
  accusationCharacter.value = ''
  modalError.value = null
}

function backToHome() {
  localStorage.removeItem('session')
  router.push({name: 'Home'})
}
</script>

<template>
  <div class="round-page arcade-bg">

    <!-- ── ANIMATION TRANSITION ROUND ─────────────────────────────────────── -->
    <RoundTransition
        v-if="showRoundTransition"
        :key="transitionRoundNumber"
        :round="transitionRoundNumber"
        @done="onRoundTransitionDone"
    />

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

      <!-- Connexion WebSocket coupée : Pusher se reconnecte seul, puis resync() -->
      <div v-if="connectionLost" class="net-banner" role="status">
        <i class="fa-solid fa-wifi"></i>
        Connexion perdue — reconnexion en cours<span class="caret">_</span>
      </div>

      <!-- Message éphémère (tour passé…) -->
      <div v-if="notice" class="turn-notice" role="status">{{ notice }}</div>

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
              {{ isMyTurn ? 'Question ou accusation' : isSpectator ? 'Tu regardes la partie' : 'À toi de deviner…' }}
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

      <!-- ── SPECTATEUR : pas de personnage ─────────────────────────────── -->
      <div v-if="isSpectator" class="info-banner info-banner--spectator">
        <span class="info-banner__icon"><i class="fa-solid fa-eye"></i></span>
        <div>
          <p class="info-banner__title">Mode spectateur</p>
          <p class="info-banner__sub">
            Tu es arrivé après le lancement : tu suis la partie sans personnage.
            Tu joueras à la prochaine !
          </p>
        </div>
      </div>

      <!-- ── CARTE PERSONNAGE ────────────────────────────────────────────── -->
      <section v-else class="board-card character-board">
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

      <!-- ── ÉLIMINÉ : il regarde la fin ───────────────────────────────── -->
      <div v-if="amEliminated && !gameEnded" class="info-banner info-banner--eliminated">
        <span class="info-banner__icon"><i class="fa-solid fa-skull"></i></span>
        <div>
          <p class="info-banner__title">{{ amExcluded ? 'Tu as été exclu' : 'Tu es éliminé' }}</p>
          <p class="info-banner__sub">
            {{ amExcluded
              ? "L'hôte t'a exclu pendant ta déconnexion, ton binôme est éliminé avec toi."
              : 'Tu suis la fin de la partie en spectateur.' }}
            Ne souffle rien à personne 🤫
          </p>
        </div>
      </div>

      <!-- ── ANNONCE : UN ORPHELIN EST EN JEU ──────────────────────────── -->
      <div v-if="hasOrphan && !gameEnded" class="orphan-banner">
        <span class="orphan-banner__icon"><i class="fa-solid fa-user-slash"></i></span>
        <div>
          <p class="orphan-banner__title">Un orphelin dans la partie</p>
          <p class="orphan-banner__sub">
            Vous êtes en nombre impair : l'un de vous joue sans binôme.
            Personne ne sait qui — pas même lui.
          </p>
        </div>
      </div>

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

      <!-- ── PAUSE : le joueur attendu est déconnecté, l'hôte décide ────── -->
      <div v-if="afkBlocker" class="info-banner info-banner--afk" role="status">
        <span class="info-banner__icon"><i class="fa-solid fa-pause"></i></span>
        <div class="info-banner__body">
          <p class="info-banner__title">Partie en pause</p>
          <p class="info-banner__sub">
            <strong>{{ afkBlocker.player.pseudo }}</strong> est déconnecté depuis
            {{ formatDuration(afkBlocker.seconds) }}. {{ AFK_REASONS[afkBlocker.reason] }}.
          </p>

          <!-- L'hôte décide -->
          <template v-if="afkBlocker.canExclude">
            <p class="info-banner__sub afk-hint">
              <template v-if="afkBlocker.waitLeft > 0">
                Laisse-lui une chance de revenir : décision possible dans {{ afkBlocker.waitLeft }} s.
              </template>
              <template v-else-if="!afkBlocker.canSkip">
                Il doit répondre : attends son retour, ou exclus-le.
              </template>
              <template v-else>Tu es l'hôte : à toi de décider.</template>
            </p>
            <div class="afk-actions">
              <button
                  v-if="afkBlocker.canSkip"
                  type="button"
                  class="cabinet-btn cabinet-btn--sm afk-skip"
                  :disabled="afkBlocker.waitLeft > 0 || skipping"
                  @click="skipBlockedTurn"
              >
                <BSpinner v-if="skipping" small class="me-1"/>
                <i v-else class="fa-solid fa-forward"></i>
                Passer son tour
              </button>
              <button
                  type="button"
                  class="cabinet-btn cabinet-btn--sm cabinet-btn--danger afk-exclude"
                  :disabled="afkBlocker.waitLeft > 0 || excluding"
                  @click="askExclude(afkBlocker.player)"
              >
                <i class="fa-solid fa-door-open"></i>
                Exclure
              </button>
            </div>
          </template>

          <p v-else class="info-banner__sub afk-hint">
            <template v-if="afkBlocker.playerId === hostId">
              C'est l'hôte : la partie attend son retour.
            </template>
            <template v-else-if="afkBlocker.hostOffline">
              L'hôte ({{ hostPseudo }}) est déconnecté lui aussi : en attente de leur retour.
            </template>
            <template v-else>
              En attente de son retour, ou de la décision de l'hôte ({{ hostPseudo }}).
            </template>
          </p>
        </div>
      </div>

      <!-- ── MODAL : confirmation d'exclusion (hôte) ────────────────────── -->
      <BModal v-model="showExcludeModal" title="🚪 Exclure un joueur" no-footer class="arcade-modal" centered>
        <p>
          Exclure <strong>{{ excludeTarget?.pseudo }}</strong> de la partie ?
        </p>
        <BAlert :model-value="true" variant="danger" class="small">
          Son binôme sera éliminé avec lui, et il marquera 0 point.
          C'est définitif, même s'il se reconnecte.
        </BAlert>
        <div class="text-center">
          <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm"
                  :disabled="excluding" @click="confirmExclude">
            <BSpinner v-if="excluding" small class="me-1"/>
            Exclure
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm"
                  @click="showExcludeModal = false">Annuler</button>
        </div>
      </BModal>

      <!-- ── ACTIONS (mon tour) ─────────────────────────────────────────── -->
      <div class="actions-section">
        <template v-if="isMyTurn && !hasPlayed">
          <button type="button" class="cabinet-btn action-cabinet" @click="openActionModal('question')">
            <span class="cabinet-btn__icon"><i class="fa-solid fa-comment-dots"></i></span>
            <span class="action-cabinet__label">Poser une question</span>
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--danger action-cabinet" @click="openActionModal('accusation')">
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
          {{ isSpectator || amEliminated ? 'Profite du spectacle' : 'Observe et prépare ta stratégie' }}<span class="caret">_</span>
        </div>

        <button type="button" class="notepad-btn" @click="showNotepad = true">
          <i class="fa-solid fa-note-sticky"></i> Mon bloc-notes
        </button>
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
                <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="action.player?.pseudo" :url="avatarById[action.player?.id]" /></span><span class="history-actor">{{ action.player?.pseudo }}</span>

                <!-- Question valide -->
                <template v-if="action.type === 'question' && action.is_valid">
                  &nbsp;demande à
                  <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="action.target_player?.pseudo" :url="avatarById[action.target_player?.id]" /></span><span class="history-target">{{ action.target_player?.pseudo }}</span>
                  : <em>« {{ action.question }} »</em>
                  <span v-if="action.answer !== null && action.answer !== undefined"
                        :class="{
            'answer-yes':       action.answer === 'yes',
            'answer-no':        action.answer === 'no',
            'answer-dont-know': action.answer === 'dont_know',
          }">
      → {{ answerLabel(action.answer) }}
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
                  <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="action.target_player?.pseudo" :url="avatarById[action.target_player?.id]" /></span><span class="history-target">{{ action.target_player?.pseudo }}</span>
                  d'être <em>« {{ action.character_name }} »</em>
                  <span class="history-muted"> → en attente de confirmation…</span>
                </template>

                <!-- Accusation correcte -->
                <template v-else-if="action.type === 'accusation' && action.accusation_correct">
                  &nbsp;a correctement accusé
                  <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="action.target_player?.pseudo" :url="avatarById[action.target_player?.id]" /></span><span class="history-target">{{ action.target_player?.pseudo }}</span>
                  d'être <em>{{ action.character_name }}</em> ! 🎯
                </template>

                <!-- Accusation niée ou incorrecte -->
                <template v-else-if="action.type === 'accusation' && action.accusation_confirmed === false">
                  &nbsp;a accusé
                  <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="action.target_player?.pseudo" :url="avatarById[action.target_player?.id]" /></span><span class="history-target">{{ action.target_player?.pseudo }}</span>
                  d'être <em>{{ action.character_name }}</em>
                  — <span class="history-muted">nié par le joueur.</span>
                </template>

                <!-- Fallback -->
                <template v-else>
                  &nbsp;a accusé
                  <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="action.target_player?.pseudo" :url="avatarById[action.target_player?.id]" /></span><span class="history-target">{{ action.target_player?.pseudo }}</span>
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

        <p class="players-hint">
          <i class="fa-solid fa-hand-pointer"></i>
          Touche un joueur pour relire tout ce qu'on lui a demandé.
        </p>

        <div class="players-grid">
          <button
              v-for="player in players"
              :key="player.id"
              type="button"
              class="player-token player-token--clickable"
              :class="{
                'is-active':     player.id === currentPlayerId && !eliminatedPlayerIds.has(player.id),
                'is-eliminated': eliminatedPlayerIds.has(player.id),
                'is-offline':    !isOnline(player.id),
              }"
              :aria-label="`Voir la fiche de ${player.pseudo}`"
              @click="openPlayerSheet(player.id)"
          >
            <span class="player-token__thread"
                  :class="{ 'is-empty': !threadCountByPlayer[player.id] }">
              <i class="fa-solid fa-comments"></i>{{ threadCountByPlayer[player.id] ?? 0 }}
            </span>
            <div class="player-token__avatar">
              <PlayerAvatar :pseudo="player.pseudo" :url="player.avatar_url" />
            </div>
            <div class="player-token__name">{{ player.pseudo }}</div>
            <div class="player-token__badges">
              <span v-if="player.id === myPlayerId" class="player-token__badge">moi</span>
              <span v-if="player.id === currentPlayerId && !eliminatedPlayerIds.has(player.id)"
                    class="player-token__badge player-token__badge--active">joue</span>
              <span v-if="discoveredPlayerIds.has(player.id)"
                    class="player-token__badge player-token__badge--danger">découvert</span>
              <span v-if="player.is_excluded"
                    class="player-token__badge player-token__badge--danger">🚪 exclu</span>
              <span v-else-if="eliminatedPlayerIds.has(player.id)"
                    class="player-token__badge player-token__badge--danger">💀 éliminé</span>
              <span v-if="!isOnline(player.id) && !eliminatedPlayerIds.has(player.id)"
                    class="player-token__badge player-token__badge--offline">hors ligne</span>
              <span v-if="notes.players[player.id]?.trim()"
                    class="player-token__badge" title="Tu as des notes sur ce joueur">📝</span>
            </div>
          </button>
        </div>

        <p v-if="spectators.length" class="spectators-line">
          <i class="fa-solid fa-eye"></i>
          Spectateur{{ spectators.length > 1 ? 's' : '' }} :
          {{ spectators.map(sp => sp.pseudo).join(', ') }}
        </p>
      </section>

      <!-- ── MODAL : Fiche joueur (conversation) ────────────────────────── -->
      <BModal
          v-model="showPlayerSheet"
          :title="`🗂️ Fiche de ${selectedPlayer?.pseudo ?? ''}`"
          no-footer
          class="arcade-modal"
          centered
          scrollable
      >
        <div class="sheet-body">

          <!-- En-tête : qui on consulte -->
          <div class="sheet-head">
            <span class="sheet-avatar">
              <PlayerAvatar :pseudo="selectedPlayer?.pseudo" :url="selectedPlayer?.avatar_url" />
            </span>
            <div class="sheet-head__info">
              <p class="sheet-name">{{ selectedPlayer?.pseudo }}</p>
              <div class="player-token__badges">
                <span v-if="selectedPlayer?.id === myPlayerId" class="player-token__badge">moi</span>
                <span v-if="selectedPlayer?.id === currentPlayerId && !eliminatedPlayerIds.has(selectedPlayer?.id)"
                      class="player-token__badge player-token__badge--active">joue</span>
                <span v-if="discoveredPlayerIds.has(selectedPlayer?.id)"
                      class="player-token__badge player-token__badge--danger">découvert</span>
                <span v-if="selectedPlayer?.is_excluded"
                      class="player-token__badge player-token__badge--danger">🚪 exclu</span>
                <span v-else-if="eliminatedPlayerIds.has(selectedPlayer?.id)"
                      class="player-token__badge player-token__badge--danger">💀 éliminé</span>
                <span class="player-token__badge">
                  {{ selectedPlayerThread.length }} échange{{ selectedPlayerThread.length > 1 ? 's' : '' }}
                </span>
              </div>
            </div>
          </div>

          <!-- Mes notes privées sur ce joueur (aussi dans le bloc-notes) -->
          <div v-if="selectedPlayer && selectedPlayer.id !== myPlayerId" class="sheet-notes">
            <label class="sheet-notes__label" for="sheet-notes">
              <i class="fa-solid fa-note-sticky"></i> Mes notes <span>(privées)</span>
            </label>
            <textarea
                id="sheet-notes"
                v-model="notes.players[selectedPlayer.id]"
                class="form-control"
                rows="2"
                maxlength="500"
                placeholder="Personnage ? Univers ?"
            ></textarea>
          </div>

          <p v-if="!selectedPlayerThread.length" class="sheet-empty">
            Personne ne lui a encore rien demandé.
          </p>

          <!-- Conversation : tout ce qui lui a été adressé, du plus ancien au plus récent -->
          <div v-else class="chat">
            <template v-for="entry in selectedPlayerThread" :key="entry.key">

              <div v-if="entry.showRound" class="chat-round">
                <span class="chat-round__line"></span>
                <span class="pixel-chip pixel-chip--gold">Round {{ entry.round }}</span>
                <span class="chat-round__line"></span>
              </div>

              <!-- Question posée au joueur, puis sa réponse -->
              <template v-if="entry.action.type === 'question' && entry.action.is_valid">
                <div class="chat-row chat-row--in">
                  <div class="chat-bubble chat-bubble--in">
                    <p class="chat-author">{{ entry.action.player?.pseudo }}</p>
                    <p class="chat-text">{{ entry.action.question }}</p>
                  </div>
                </div>
                <div class="chat-row chat-row--out">
                  <div
                      v-if="entry.action.answer !== null && entry.action.answer !== undefined"
                      class="chat-bubble chat-bubble--out"
                      :class="`chat-bubble--${entry.action.answer}`"
                  >
                    <p class="chat-text">{{ answerLabel(entry.action.answer) }}</p>
                  </div>
                  <div v-else class="chat-bubble chat-bubble--out chat-bubble--pending">
                    <p class="chat-text">en attente de réponse…</p>
                  </div>
                </div>
              </template>

              <!-- Question refusée (mot interdit) -->
              <div v-else-if="entry.action.type === 'question'" class="chat-system chat-system--ko">
                ❌ <span class="chat-author">{{ entry.action.player?.pseudo }}</span>
                a utilisé un mot interdit — tour perdu.
              </div>

              <!-- Accusation dont il a fait l'objet -->
              <div v-else
                   class="chat-system"
                   :class="`chat-system--${accusationOutcome(entry.action).tone}`">
                {{ accusationOutcome(entry.action).icon }}
                <span class="chat-author">{{ entry.action.player?.pseudo }}</span>
                l'accuse
                <template v-if="entry.action.character_name">
                  d'être <em>« {{ entry.action.character_name }} »</em>
                </template>
                — {{ accusationOutcome(entry.action).text }}
              </div>

            </template>
          </div>
        </div>
      </BModal>

      <!-- ── MODAL : Poser une question ─────────────────────────────────── -->
      <BModal v-model="showQuestionModal" title="💬 Poser une question" no-footer class="arcade-modal">
        <div v-if="modalError" class="modal-error" role="alert">
          <i class="fa-solid fa-triangle-exclamation"></i>
          <span>{{ modalError }}</span>
        </div>
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
              <span class="target-chip__avatar"><PlayerAvatar :pseudo="p.pseudo" :url="p.avatar_url" /></span>
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
              :maxlength="QUESTION_MAX"
              :state="trimmedQuestion.length && questionTooShort ? false : null"
              @update:model-value="modalError = null"
              @keyup.enter="submitQuestion"
          />
          <div class="input-hint" :class="{ 'is-warning': trimmedQuestion.length && questionTooShort }">
            <span v-if="!trimmedQuestion.length">Minimum {{ QUESTION_MIN }} caractères.</span>
            <span v-else-if="questionTooShort">
              Encore {{ QUESTION_MIN - trimmedQuestion.length }}
              caractère{{ QUESTION_MIN - trimmedQuestion.length > 1 ? 's' : '' }}
              avant de pouvoir envoyer.
            </span>
            <span v-else></span>
            <span class="input-hint__count">{{ trimmedQuestion.length }}/{{ QUESTION_MAX }}</span>
          </div>
          <small class="text-muted">Tu ignores les mots interdits de ta cible : si tu en prononces un, elle aura le droit de te mentir.</small>
        </div>
        <div class="text-center">
          <button type="button" class="cabinet-btn cabinet-btn--sm"
                   :disabled="!questionTarget || questionTooShort || submitting"
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
                  <span v-if="orphanPseudos.has(stat.player_pseudo)" class="stat-chip stat-chip-orphan">
              🚷 Orphelin
            </span>
                  <span v-if="stat.is_excluded" class="stat-chip stat-chip-excluded">
              🚪 Exclu
            </span>
                </div>

                <!-- Score total -->
                <span class="scoreboard-score">{{ stat.score }} pts</span>
              </div>
            </div>

            <GameRecap v-if="recap" :recap="recap" />

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
        <div v-if="modalError" class="modal-error" role="alert">
          <i class="fa-solid fa-triangle-exclamation"></i>
          <span>{{ modalError }}</span>
        </div>
        <BAlert :model-value="true" variant="warning" class="small">
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
              <span class="target-chip__avatar"><PlayerAvatar :pseudo="p.pseudo" :url="p.avatar_url" /></span>
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
              :maxlength="CHARACTER_MAX"
              :disabled="!accusationTarget"
              @update:model-value="modalError = null"
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

          <BAlert :model-value="true" variant="danger" class="small mb-0">
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

      <GameNotepad
          v-model="showNotepad"
          :notes="notes"
          :players="players"
          :my-player-id="myPlayerId"
      />

      <QuickReactions :reactions="reactions" @send="sendReaction" />

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

/* Annonce d'un orphelin : informatif, volontairement plus sobre que la
   notification de binôme découvert — ce n'est pas un événement de jeu. */
.orphan-banner {
  margin: 0 1rem 1.25rem;
  background: linear-gradient(180deg, rgba(90, 111, 125, 0.28), rgba(28, 34, 38, 0.9));
  border: 3px solid var(--arcade-blue-grey);
  border-radius: 12px;
  padding: 0.75rem 0.9rem;
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  box-shadow: 0 5px 0 var(--arcade-blue-grey-dark);
}

.orphan-banner__icon {
  flex-shrink: 0;
  width: 1.9rem;
  height: 1.9rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 6px;
  color: #c3d2db;
}

.orphan-banner__title {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.55rem;
  line-height: 1.6;
  color: #d3e0e8;
  margin: 0 0 0.35rem;
}

.orphan-banner__sub {
  font-size: 0.8rem;
  color: #a7bac6;
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

/* Bloc-notes privé : secondaire, donc volontairement discret */
.notepad-btn {
  align-self: center;
  min-height: 44px;
  padding: 0 1rem;
  font-family: 'Baloo 2', sans-serif;
  font-weight: 700;
  font-size: 0.9rem;
  color: var(--arcade-beige);
  background: transparent;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 8px;
}

.notepad-btn:active {
  transform: translateY(1px);
}

/* ─── BANNIÈRES D'ÉTAT (spectateur, éliminé, joueur déconnecté) ─────────────── */
.info-banner {
  margin: 0.75rem 1rem 1.25rem;
  padding: 0.75rem 0.9rem;
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  border: 3px solid var(--arcade-blue-grey);
  border-radius: 12px;
  background: linear-gradient(180deg, rgba(90, 111, 125, 0.28), rgba(28, 34, 38, 0.9));
  box-shadow: 0 5px 0 var(--arcade-blue-grey-dark);
}

.info-banner__icon {
  flex-shrink: 0;
  width: 1.9rem;
  height: 1.9rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px solid currentColor;
  border-radius: 6px;
  color: #c3d2db;
}

.info-banner__body {
  min-width: 0;
  flex: 1;
}

.info-banner__title {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.55rem;
  line-height: 1.6;
  color: #d3e0e8;
  margin: 0 0 0.35rem;
  overflow-wrap: anywhere;
}

.info-banner__sub {
  font-size: 0.8rem;
  color: #a7bac6;
  margin: 0;
}

.info-banner--eliminated {
  border-color: var(--arcade-danger);
  background: linear-gradient(180deg, rgba(179, 69, 63, 0.3), rgba(28, 34, 38, 0.9));
  box-shadow: 0 5px 0 var(--arcade-danger-dark);
}

.info-banner--eliminated .info-banner__icon { color: #e8a29e; }
.info-banner--eliminated .info-banner__title { color: #f3c2bf; }
.info-banner--eliminated .info-banner__sub { color: #d99a96; }

.info-banner--afk {
  border-color: var(--arcade-gold);
  background: linear-gradient(180deg, rgba(224, 163, 28, 0.25), rgba(28, 34, 38, 0.9));
  box-shadow: 0 5px 0 #a8720f;
  animation: slideIn 0.3s ease;
}

.info-banner--afk .info-banner__icon { color: #ffd876; }
.info-banner--afk .info-banner__title { color: #ffe4a3; }
.info-banner--afk .info-banner__sub { color: #e6c98a; }

.afk-hint {
  margin-top: 0.35rem;
}

.afk-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-top: 0.6rem;
}

/* Connexion WebSocket coupée */
.net-banner {
  margin: 0.75rem 1rem 0;
  padding: 0.5rem 0.75rem;
  font-size: 0.85rem;
  font-weight: 700;
  color: #4a2f00;
  background: var(--arcade-gold);
  border: 2px solid #a8720f;
  border-radius: 8px;
}

/* Message éphémère */
.turn-notice {
  margin: 0.75rem 1rem 0;
  padding: 0.5rem 0.75rem;
  font-size: 0.85rem;
  color: var(--arcade-beige);
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 8px;
  animation: slideIn 0.3s ease;
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

.players-hint {
  font-size: 0.75rem;
  color: var(--arcade-taupe);
  margin: -0.6rem 0 0.9rem;
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

/* Jeton cliquable : ouvre la fiche (conversation) du joueur */
.player-token--clickable {
  appearance: none;
  width: 100%;
  font: inherit;
  cursor: pointer;
  text-align: center;
  transition: transform 0.15s ease, box-shadow 0.15s ease, border-color 0.15s ease, opacity 0.2s ease;
}

.player-token--clickable:hover,
.player-token--clickable:focus-visible {
  transform: translateY(-3px);
  border-color: var(--arcade-gold);
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.35);
  outline: none;
}

.player-token--clickable:active {
  transform: translateY(0);
  box-shadow: none;
}

/* Pastille "nombre d'échanges reçus" */
.player-token__thread {
  position: absolute;
  top: -9px;
  right: -7px;
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  padding: 0.1rem 0.4rem;
  border-radius: 999px;
  border: 2px solid #fff;
  background: var(--arcade-gold);
  color: var(--arcade-dark);
  font-size: 0.65rem;
  font-weight: 800;
  box-shadow: 0 2px 0 rgba(0, 0, 0, 0.25);
}

.player-token.is-offline .player-token__avatar {
  opacity: 0.45;
}

.player-token__badge--offline {
  background: var(--arcade-taupe);
  color: #fff;
}

.spectators-line {
  margin: 0.9rem 0 0;
  font-size: 0.8rem;
  color: var(--arcade-blue-grey-dark);
  text-align: center;
  overflow-wrap: anywhere;
}

.player-token__thread.is-empty {
  background: var(--arcade-taupe);
  color: #fff;
}

/* ─── FICHE JOUEUR (MODALE CONVERSATION) ────────────────────────────────────── */
.sheet-body {
  font-family: 'Baloo 2', sans-serif;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.sheet-head {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding-bottom: 0.75rem;
  border-bottom: 2px dashed rgba(90, 111, 125, 0.35);
}

.sheet-avatar {
  flex-shrink: 0;
  width: 46px;
  height: 46px;
  border-radius: 50%;
  background: var(--arcade-blue-grey);
  color: #fff;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.8rem;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: inset 0 -3px 0 rgba(0, 0, 0, 0.2);
}

.sheet-name {
  margin: 0;
  font-size: 1.05rem;
  font-weight: 800;
  color: var(--arcade-blue-grey-dark);
  word-break: break-word;
}

.sheet-head__info .player-token__badges {
  margin-top: 0.3rem;
}

.sheet-notes {
  margin-bottom: 0.9rem;
}

.sheet-notes__label {
  display: block;
  margin-bottom: 0.25rem;
  font-weight: 700;
  font-size: 0.85rem;
}

.sheet-notes__label span {
  font-weight: 400;
  color: #6c6259;
}

.sheet-empty {
  margin: 0;
  padding: 1.25rem 0;
  text-align: center;
  font-style: italic;
  color: var(--arcade-taupe);
}

.chat {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.chat-round {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin: 0.6rem 0 0.3rem;
}

.chat-round__line {
  flex: 1;
  height: 2px;
  background: repeating-linear-gradient(90deg, rgba(224, 163, 28, 0.45) 0 4px, transparent 4px 8px);
}

.chat-row {
  display: flex;
}

.chat-row--in  { justify-content: flex-start; }
.chat-row--out { justify-content: flex-end; }

.chat-bubble {
  max-width: 82%;
  padding: 0.5rem 0.7rem;
  border: 2px solid;
  border-radius: 12px;
  font-size: 0.85rem;
  line-height: 1.35;
  word-break: break-word;
  overflow-wrap: break-word;
}

.chat-bubble--in {
  background: rgba(90, 111, 125, 0.12);
  border-color: var(--arcade-blue-grey);
  border-bottom-left-radius: 4px;
  color: var(--arcade-blue-grey-dark);
}

.chat-bubble--out {
  border-bottom-right-radius: 4px;
  font-weight: 700;
}

.chat-bubble--yes {
  background: rgba(63, 122, 78, 0.15);
  border-color: var(--arcade-success);
  color: var(--arcade-success-dark);
}

.chat-bubble--no {
  background: rgba(179, 69, 63, 0.15);
  border-color: var(--arcade-danger);
  color: var(--arcade-danger-dark);
}

.chat-bubble--dont_know {
  background: rgba(158, 139, 127, 0.2);
  border-color: var(--arcade-taupe);
  color: #6b5c52;
}

.chat-bubble--pending {
  background: transparent;
  border-style: dashed;
  border-color: var(--arcade-taupe);
  color: var(--arcade-taupe);
  font-style: italic;
  font-weight: 600;
}

.chat-author {
  margin: 0 0 0.15rem;
  font-size: 0.7rem;
  font-weight: 800;
  color: #8a6510;
}

.chat-text {
  margin: 0;
}

.chat-system {
  font-size: 0.8rem;
  padding: 0.45rem 0.6rem;
  border-radius: 8px;
  border-left: 4px solid var(--arcade-blue-grey);
  background: rgba(90, 111, 125, 0.1);
  color: var(--arcade-blue-grey-dark);
  word-break: break-word;
}

.chat-system--ok {
  background: rgba(63, 122, 78, 0.15);
  border-left-color: var(--arcade-success);
  color: var(--arcade-success-dark);
}

.chat-system--ko {
  background: rgba(179, 69, 63, 0.15);
  border-left-color: var(--arcade-danger);
  color: var(--arcade-danger-dark);
}

.chat-system--pending {
  background: rgba(158, 139, 127, 0.15);
  border-left-color: var(--arcade-taupe);
  color: #6b5c52;
  font-style: italic;
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

/* Photo (ou initiales) devant un pseudo — voir PlayerAvatar */
.mini-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 1.5rem;
  height: 1.5rem;
  margin-right: 0.3rem;
  vertical-align: middle;
  border-radius: 50%;
  overflow: hidden;
  background: var(--arcade-taupe);
  color: #fff;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.5rem;
  line-height: 1;
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

/* ─── ERREUR & AIDE DANS LES MODALES D'ACTION ───────────────────────────────── */
/* La bannière .round-error de la page est masquée par le backdrop de la modale :
   les refus de l'API doivent donc être affichés ici, sous les yeux du joueur. */
.modal-error {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  margin-bottom: 1rem;
  padding: 0.6rem 0.75rem;
  border: 2px solid var(--arcade-danger);
  border-radius: 8px;
  background: rgba(179, 69, 63, 0.12);
  color: var(--arcade-danger-dark);
  font-size: 0.85rem;
  font-weight: 700;
  line-height: 1.35;
}

.input-hint {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  margin-top: 0.3rem;
  font-size: 0.75rem;
  color: var(--arcade-taupe);
}

.input-hint.is-warning {
  color: var(--arcade-danger-dark);
  font-weight: 700;
}

.input-hint__count {
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
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
  flex-wrap: wrap;
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
  /* Passe à la ligne plutôt que de déborder à 320px */
  min-width: max-content;
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

.stat-chip-excluded {
  background: rgba(179, 69, 63, 0.12);
  border-color: var(--arcade-danger);
  color: var(--arcade-danger-dark);
}

.stat-chip-orphan {
  background: rgba(90, 111, 125, 0.2);
  border-color: var(--arcade-blue-grey-dark);
  color: var(--arcade-blue-grey-dark);
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
