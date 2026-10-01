<script setup>
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { chatService } from '../../services/chatService'
import { getAnonId } from '../../services/chatIdentity'
import { useReverb, onEchoReset, hasEcho } from '../../sockets/useReverb'
import { copyToClipboard } from '../../services/copyToClipboard'

const props = defineProps({
  // null tant que le visiteur n'a ni créé ni rejoint de salon.
  playerId: { type: [Number, String], default: null },
  isInRoom: { type: Boolean, default: false },
})

const MAX_LENGTH = 500
const MAX_MESSAGES = 100
// Durée de vie d'un message, alignée sur la rétention du backend
// (ChatController::RETENTION_MINUTES). Le serveur ne sert plus les messages
// plus vieux ; on les retire aussi côté client pour qu'ils disparaissent de
// l'écran d'un onglet resté ouvert, sans rechargement ni notification.
const MESSAGE_TTL_MS = 60 * 60 * 1000
const EXPIRY_SWEEP_MS = 30 * 1000

const anonId = getAnonId()

const messages = ref([])
const draft = ref('')
const sending = ref(false)
const error = ref(null)
// Déplié d'office pour un visiteur sans salon ; replié dès qu'il en rejoint un
// (l'écran est alors occupé par la carte du salon).
const collapsed = ref(props.isInRoom)
const unread = ref(0)
const scroller = ref(null)
const copiedKey = ref(null)
let copiedTimer = null
let expiryTimer = null

// Identité telle que le backend la calcule : sert à repérer ses propres messages.
const myKey = computed(() => (props.playerId ? `p:${props.playerId}` : `a:${anonId}`))

const remaining = computed(() => MAX_LENGTH - draft.value.length)
const canSend = computed(() => draft.value.trim().length > 0 && !sending.value)

// ─── AFFICHAGE DES MESSAGES ───────────────────────────────────────────────────

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/**
 * Découpe le corps du message en segments texte / code de partie.
 * `codes` est calculé côté serveur (codes confrontés à la table `rooms`), un
 * simple mot de six lettres ne devient donc pas une puce copiable par erreur.
 */
function segments(message) {
  const codes = message.codes ?? []
  if (!codes.length) return [{ type: 'text', value: message.body }]

  const pattern = new RegExp(`(${codes.map(escapeRegExp).join('|')})`, 'gi')
  const isCode = (part) => codes.some((c) => c.toLowerCase() === part.toLowerCase())

  return message.body
      .split(pattern)
      .filter((part) => part !== '')
      .map((part) => (isCode(part)
          ? { type: 'code', value: part.toUpperCase() }
          : { type: 'text', value: part }))
}

function formatTime(iso) {
  if (!iso) return ''
  return new Date(iso).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })
}

async function copyCode(code, key) {
  const ok = await copyToClipboard(code)
  if (!ok) return
  copiedKey.value = key
  clearTimeout(copiedTimer)
  copiedTimer = setTimeout(() => { copiedKey.value = null }, 2000)
}

// ─── FLUX DE MESSAGES ─────────────────────────────────────────────────────────

function isNearBottom() {
  const el = scroller.value
  if (!el) return true
  return el.scrollHeight - el.scrollTop - el.clientHeight < 80
}

function scrollToBottom() {
  nextTick(() => {
    if (scroller.value) scroller.value.scrollTop = scroller.value.scrollHeight
  })
}

function pushMessage(message) {
  // Le message posté arrive deux fois : réponse HTTP + diffusion WebSocket.
  if (messages.value.some((m) => m.id === message.id)) return

  const stick = isNearBottom()
  messages.value.push(message)
  if (messages.value.length > MAX_MESSAGES) {
    messages.value.splice(0, messages.value.length - MAX_MESSAGES)
  }

  if (collapsed.value && message.author_key !== myKey.value) {
    unread.value += 1
  } else if (stick) {
    scrollToBottom()
  }
}

/**
 * Retire les messages qui ont dépassé leur durée de vie. Silencieux : aucun
 * message système, ils s'effacent simplement de la liste.
 */
function expireOldMessages() {
  const cutoff = Date.now() - MESSAGE_TTL_MS
  const kept = messages.value.filter((m) => {
    const at = m.created_at ? new Date(m.created_at).getTime() : NaN
    return Number.isNaN(at) || at > cutoff
  })

  if (kept.length === messages.value.length) return

  messages.value = kept
  // Des non-lus ont pu expirer : le compteur ne peut pas dépasser ce qui reste.
  unread.value = Math.min(unread.value, kept.length)
}

async function loadHistory() {
  try {
    const res = await chatService.list()
    messages.value = res.data.messages ?? []
    expireOldMessages()
    scrollToBottom()
  } catch {
    error.value = 'Impossible de charger le chat.'
  }
}

async function handleSend() {
  if (!canSend.value) return
  sending.value = true
  error.value = null

  try {
    const res = await chatService.send({
      body: draft.value.trim(),
      playerId: props.playerId,
      anonId,
    })
    draft.value = ''
    pushMessage(res.data.message)
    scrollToBottom()
  } catch (e) {
    error.value = e.response?.status === 429
        ? 'Doucement ! Trop de messages d\'un coup.'
        : e.response?.data?.message || 'Le message n\'a pas pu être envoyé.'
  } finally {
    sending.value = false
  }
}

// ─── WEBSOCKET ────────────────────────────────────────────────────────────────

function subscribe() {
  const { joinLobbyChat } = useReverb(props.playerId)
  joinLobbyChat({
    onChatMessage: (data) => pushMessage(data.message),
  })
}

// resetEcho() est appelé par RoomPage à chaque changement d'identité ; on se
// rebranche au tick suivant, une fois que la page a recréé le singleton avec le
// bon X-Player-Id (sinon on le recréerait ici avec l'ancien et le channel de
// présence du salon se ferait refuser).
const stopResetHook = onEchoReset(() => {
  nextTick(subscribe)
})

// ─── UI ───────────────────────────────────────────────────────────────────────

function toggle() {
  collapsed.value = !collapsed.value
  if (!collapsed.value) {
    unread.value = 0
    scrollToBottom()
  }
}

// Entrer / sortir d'un salon remet la visibilité à sa valeur par défaut.
watch(() => props.isInRoom, (inRoom) => {
  collapsed.value = inRoom
  if (!inRoom) unread.value = 0
})

onMounted(() => {
  loadHistory()
  subscribe()
  expiryTimer = setInterval(expireOldMessages, EXPIRY_SWEEP_MS)
})

onUnmounted(() => {
  stopResetHook()
  clearTimeout(copiedTimer)
  clearInterval(expiryTimer)
  // Pas de useReverb() si le singleton a été détruit entre-temps : cela
  // rouvrirait une connexion WebSocket uniquement pour la refermer.
  if (hasEcho()) {
    useReverb(props.playerId).leaveLobbyChat()
  }
})
</script>

<template>
  <section class="lobby-chat">
    <header class="lobby-chat__head">
      <h2 class="lobby-chat__title">
        <i class="fa-solid fa-comments"></i> Chat du hall
      </h2>
      <button
          type="button"
          class="lobby-chat__toggle"
          :aria-expanded="!collapsed"
          @click="toggle"
      >
        <i class="fa-solid" :class="collapsed ? 'fa-eye' : 'fa-eye-slash'"></i>
        {{ collapsed ? 'Afficher' : 'Masquer' }}
        <span v-if="collapsed && unread" class="lobby-chat__unread">{{ unread }}</span>
      </button>
    </header>

    <div v-show="!collapsed" class="lobby-chat__body">
      <p class="lobby-chat__hint">
        <i class="fa-solid fa-circle-info"></i>
        Tout le monde te lit ici. Colle le code de ton salon pour recruter —
        il deviendra copiable en un clic.
      </p>

      <div ref="scroller" class="lobby-chat__stream">
        <p v-if="!messages.length" class="lobby-chat__empty">
          Personne n'a encore parlé. Lance-toi !
        </p>

        <div
            v-for="message in messages"
            :key="message.id"
            class="chat-msg"
            :class="{ 'is-mine': message.author_key === myKey }"
        >
          <div class="chat-msg__meta">
            <span class="chat-msg__author" :class="{ 'is-anon': message.is_anonymous }">
              <i class="fa-solid" :class="message.is_anonymous ? 'fa-user-secret' : 'fa-user'"></i>
              {{ message.author_name }}
            </span>
            <span class="chat-msg__time">{{ formatTime(message.created_at) }}</span>
          </div>
          <p class="chat-msg__body">
            <template v-for="(part, i) in segments(message)" :key="i">
              <button
                  v-if="part.type === 'code'"
                  type="button"
                  class="chat-code"
                  :class="{ 'is-copied': copiedKey === `${message.id}-${i}` }"
                  :title="`Copier le code ${part.value}`"
                  @click="copyCode(part.value, `${message.id}-${i}`)"
              >
                <i class="fa-solid" :class="copiedKey === `${message.id}-${i}` ? 'fa-check' : 'fa-copy'"></i>
                {{ part.value }}
              </button>
              <template v-else>{{ part.value }}</template>
            </template>
          </p>
        </div>
      </div>

      <p v-if="error" class="lobby-chat__error">
        <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
      </p>

      <form class="lobby-chat__form" @submit.prevent="handleSend">
        <input
            v-model="draft"
            type="text"
            class="lobby-chat__input"
            :maxlength="MAX_LENGTH"
            placeholder="Écris un message…"
            aria-label="Message à envoyer dans le chat du hall"
        />
        <button
            type="submit"
            class="cabinet-btn cabinet-btn--sm lobby-chat__send"
            :disabled="!canSend"
            aria-label="Envoyer"
        >
          <i class="fa-solid fa-paper-plane"></i>
        </button>
      </form>

      <p class="lobby-chat__footnote">
        Tu écris en tant que <strong>{{ playerId ? 'ton pseudo' : `Anonyme#${anonId}` }}</strong>
        · {{ remaining }} caractère{{ remaining > 1 ? 's' : '' }} restant{{ remaining > 1 ? 's' : '' }}
      </p>
    </div>
  </section>
</template>

<style scoped>
/* Mobile first : les valeurs de base visent un écran de 320 px, les media
   queries `min-width` ne font qu'élargir ensuite. Rien ici ne doit pouvoir
   dépasser la largeur du conteneur — d'où les `min-width: 0` sur les enfants
   flex (qui, sinon, refusent de rétrécir sous leur contenu) et les
   `overflow-wrap` sur les textes libres. */
.lobby-chat {
  position: relative;
  width: 100%;
  max-width: 640px;
  margin: 0 auto 2rem;
  padding: 0.9rem;
  background: var(--arcade-beige);
  border: 3px dashed var(--arcade-taupe);
  border-radius: 16px;
  box-shadow: 0 10px 0 rgba(0, 0, 0, 0.25), 0 14px 20px rgba(0, 0, 0, 0.35);
  /* #app impose text-align: center à toute la page ; un chat se lit à gauche. */
  text-align: left;
  overflow: hidden;
}

.lobby-chat__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.lobby-chat__title {
  min-width: 0;
  margin: 0;
  font-size: 0.85rem;
  font-weight: 800;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: var(--arcade-blue-grey-dark);
}

.lobby-chat__toggle {
  flex: 0 0 auto;
  /* 44px : cible tactile minimale confortable au pouce. */
  min-height: 44px;
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 10px;
  background: transparent;
  color: var(--arcade-blue-grey-dark);
  font-family: inherit;
  font-weight: 700;
  font-size: 0.78rem;
  padding: 0.3rem 0.6rem;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.lobby-chat__toggle:hover {
  background: var(--arcade-blue-grey-dark);
  color: #fff;
}

.lobby-chat__unread {
  display: inline-block;
  min-width: 1.25rem;
  margin-left: 0.35rem;
  padding: 0 0.3rem;
  border-radius: 999px;
  background: var(--arcade-danger);
  color: #fff;
  font-size: 0.75rem;
  line-height: 1.25rem;
  text-align: center;
}

.lobby-chat__body {
  margin-top: 0.8rem;
}

.lobby-chat__hint {
  margin: 0 0 0.6rem;
  font-size: 0.75rem;
  color: var(--arcade-taupe);
}

.lobby-chat__stream {
  height: 210px;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0.5rem;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.45);
  overscroll-behavior: contain;
}

.lobby-chat__empty {
  margin: 0;
  padding-top: 2rem;
  text-align: center;
  color: var(--arcade-taupe);
  font-size: 0.85rem;
}

.chat-msg {
  margin-bottom: 0.6rem;
  padding: 0.35rem 0.5rem;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.8);
  border-left: 4px solid var(--arcade-blue-grey);
}

.chat-msg.is-mine {
  border-left-color: var(--arcade-success);
  background: rgba(63, 122, 78, 0.12);
}

.chat-msg__meta {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.4rem;
  font-size: 0.75rem;
}

.chat-msg__author {
  min-width: 0;
  font-weight: 800;
  color: var(--arcade-blue-grey-dark);
  overflow-wrap: anywhere;
}

.chat-msg__author.is-anon {
  color: var(--arcade-taupe);
}

.chat-msg__time {
  flex: 0 0 auto;
  color: var(--arcade-taupe);
}

.chat-msg__body {
  margin: 0.15rem 0 0;
  font-size: 0.9rem;
  color: var(--arcade-dark);
  /* Un pavé collé sans espace ne doit pas pousser la carte hors de l'écran. */
  overflow-wrap: anywhere;
}

/* ── Code de partie copiable ──────────────────────────────────── */
.chat-code {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  max-width: 100%;
  margin: 0.1rem 0.15rem;
  padding: 0.05rem 0.4rem;
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 8px;
  background: var(--arcade-dark-2);
  color: #7CFF9E;
  font-family: 'Courier New', monospace;
  font-size: 0.85rem;
  font-weight: 700;
  letter-spacing: 1px;
  cursor: pointer;
  transition: box-shadow 0.15s ease, transform 0.1s ease;
}

.chat-code:hover {
  box-shadow: 0 0 10px rgba(124, 255, 158, 0.55);
}

.chat-code:active {
  transform: translateY(1px);
}

.chat-code.is-copied {
  border-color: #7CFF9E;
  box-shadow: 0 0 12px rgba(124, 255, 158, 0.75);
}

/* ── Saisie ───────────────────────────────────────────────────── */
.lobby-chat__error {
  margin: 0.5rem 0 0;
  font-size: 0.78rem;
  color: var(--arcade-danger);
}

.lobby-chat__form {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.6rem;
}

.lobby-chat__input {
  /* min-width: 0 est indispensable : sans lui, un input flex refuse de
     descendre sous sa largeur intrinsèque et déborde sur petit écran. */
  flex: 1 1 auto;
  min-width: 0;
  padding: 0.5rem 0.7rem;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 10px;
  background: #fff;
  /* style.css déclare `color-scheme: light dark` sur :root. Un <input> nu sans
     `color` explicite retombe sur le mot-clé système `fieldtext`, qui vaut
     BLANC quand l'OS est en thème sombre — texte invisible sur fond blanc.
     Les autres champs de l'app y échappent parce qu'ils passent par
     .form-control (Bootstrap), qui fixe la couleur. */
  color: var(--arcade-dark);
  color-scheme: light;
  font-family: inherit;
  font-size: 16px; /* < 16px déclenche le zoom automatique de Safari iOS */
}

.lobby-chat__input::placeholder {
  color: var(--arcade-taupe);
  opacity: 1;
}

.lobby-chat__input:focus {
  outline: none;
  border-color: var(--arcade-blue-grey-dark);
}

.lobby-chat__send {
  flex: 0 0 auto;
  min-width: auto;
  margin: 0;
  padding: 0.5rem 0.8rem;
}

.lobby-chat__footnote {
  margin: 0.45rem 0 0;
  font-size: 0.75rem;
  color: var(--arcade-taupe);
  text-align: right;
  overflow-wrap: anywhere;
}

/* ── À partir du format tablette ──────────────────────────────── */
@media (min-width: 576px) {
  .lobby-chat {
    margin-bottom: 2.5rem;
    padding: 1.1rem 1.25rem;
    border-radius: 18px;
  }

  .lobby-chat__title {
    font-size: 0.95rem;
  }

  .lobby-chat__toggle {
    font-size: 0.8rem;
    padding: 0.35rem 0.75rem;
  }

  .lobby-chat__stream {
    height: 260px;
    padding: 0.6rem;
  }

  .chat-msg__body {
    font-size: 0.92rem;
  }
}
</style>
