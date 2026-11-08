<script setup>
import {ref, watch, onMounted, onUnmounted} from 'vue'
import {isImmersive, toggleImmersive, exitImmersive} from '../../composables/useImmersiveMode'
import {reactionService} from '../../services/reactionService'
import ReactionEditor from './ReactionEditor.vue'

// Réactions rapides : un bouton flottant qui déplie la barre d'emojis, et une
// couche d'animation où montent les réactions reçues (les siennes comprises,
// elles reviennent par le broadcast comme celles des autres).
// Toutes les réactions viennent de GET /reactions (gérées dans l'onglet
// Emojis de l'admin) : emojis Unicode puis images créées par les joueurs,
// et enfin le bouton « + » pour en créer une (ReactionEditor).

// Délai minimal entre deux envois, en plus du throttle serveur.
const COOLDOWN_MS = 1200

defineProps({
  // [{ id, emoji, url, name, pseudo, x }] — emoji (Unicode) ou url (image) ;
  // x : position horizontale en %
  reactions: {type: Array, default: () => []},
  playerId: {type: [Number, String], default: null},
})

// Payload : { reaction_id }
const emit = defineEmits(['send'])

const open = ref(false)
const coolingDown = ref(false)
const available = ref([])
const loadFailed = ref(false)
const showEditor = ref(false)

async function loadReactions() {
  try {
    available.value = await reactionService.list()
    loadFailed.value = false
  } catch {
    loadFailed.value = available.value.length === 0
  }
}

// Rechargées aussi à chaque ouverture : un emoji ajouté ou supprimé
// entre-temps apparaît sans recharger la page.
onMounted(loadReactions)
watch(open, (isOpen) => isOpen && loadReactions())

function send(reaction) {
  if (coolingDown.value) return
  emit('send', reaction)
  coolingDown.value = true
  setTimeout(() => { coolingDown.value = false }, COOLDOWN_MS)
}

function onCreated(reaction) {
  available.value = [...available.value, reaction]
}

// Le bouton plein écran n'existe que pendant la partie : en la quittant, on
// rend navbar et footer, sinon les autres pages n'auraient plus de navigation.
onUnmounted(exitImmersive)
</script>

<template>
  <!-- Réactions reçues -->
  <div class="reaction-layer" aria-live="polite">
    <div
        v-for="r in reactions"
        :key="r.id"
        class="reaction-float"
        :style="{ left: `${r.x}%` }"
    >
      <img v-if="r.url" :src="r.url" :alt="r.name" class="reaction-float__img" />
      <span v-else class="reaction-float__emoji">{{ r.emoji }}</span>
      <span class="reaction-float__pseudo">{{ r.pseudo }}</span>
    </div>
  </div>

  <!-- Envoi -->
  <div class="reaction-dock">
    <div v-if="open" class="reaction-bar" role="toolbar" aria-label="Réactions rapides">
      <button
          v-for="r in available"
          :key="r.id"
          type="button"
          class="reaction-bar__btn"
          :disabled="coolingDown"
          :aria-label="`Réagir ${r.name}`"
          :title="r.created_by ? `${r.name} — par ${r.created_by}` : r.name"
          @click="send({ reaction_id: r.id })"
      >
        <img v-if="r.url" :src="r.url" :alt="r.name" class="reaction-bar__img" loading="lazy" />
        <template v-else>{{ r.emoji }}</template>
      </button>
      <p v-if="loadFailed" class="reaction-bar__empty">Réactions indisponibles</p>
      <button
          type="button"
          class="reaction-bar__btn reaction-bar__btn--add"
          aria-label="Créer un emoji"
          title="Créer un emoji"
          @click="showEditor = true"
      ><i class="fa-solid fa-plus"></i></button>
    </div>
    <div class="reaction-dock__row">
      <button
          type="button"
          class="immersive-fab"
          :aria-pressed="isImmersive"
          :aria-label="isImmersive ? 'Quitter le plein écran' : 'Plein écran'"
          :title="isImmersive ? 'Quitter le plein écran' : 'Plein écran'"
          @click="toggleImmersive"
      >
        <i :class="isImmersive ? 'fa-solid fa-compress' : 'fa-solid fa-expand'"></i>
      </button>
      <button
          type="button"
          class="reaction-fab"
          :class="{ 'is-open': open }"
          :aria-expanded="open"
          aria-label="Réactions rapides"
          @click="open = !open"
      >
        <i :class="open ? 'fa-solid fa-xmark' : 'fa-regular fa-face-smile'"></i>
      </button>
    </div>
  </div>

  <ReactionEditor v-model="showEditor" :player-id="playerId" @created="onCreated" />
</template>

<style scoped>
/* ─── Couche d'animation ─────────────────────────────────────────────────── */
.reaction-layer {
  position: fixed;
  inset: 0;
  pointer-events: none;
  overflow: hidden;
  z-index: 1060; /* au-dessus des modales Bootstrap (1055) */
}

.reaction-float {
  position: absolute;
  bottom: calc(var(--footer-height) + 4rem);
  display: flex;
  flex-direction: column;
  align-items: center;
  transform: translateX(-50%);
  animation: reaction-rise 2.6s ease-out forwards;
}

.reaction-float__emoji {
  font-size: 2.4rem;
  line-height: 1;
  filter: drop-shadow(0 3px 0 rgba(0, 0, 0, 0.35));
}

.reaction-float__img {
  width: 2.6rem;
  height: 2.6rem;
  border-radius: 50%;
  object-fit: cover;
  border: 2px solid var(--arcade-beige);
  filter: drop-shadow(0 3px 0 rgba(0, 0, 0, 0.35));
}

.reaction-float__pseudo {
  margin-top: 0.15rem;
  padding: 0 0.35rem;
  font-size: 0.7rem;
  font-weight: 700;
  white-space: nowrap;
  color: var(--arcade-beige);
  background: rgba(36, 36, 36, 0.8);
}

@keyframes reaction-rise {
  0%   { opacity: 0; transform: translate(-50%, 20px) scale(0.6); }
  15%  { opacity: 1; transform: translate(-50%, 0) scale(1.1); }
  25%  { transform: translate(-50%, -10px) scale(1); }
  80%  { opacity: 1; }
  100% { opacity: 0; transform: translate(-50%, -45vh) scale(1); }
}

@media (prefers-reduced-motion: reduce) {
  .reaction-float {
    animation: reaction-fade 2.6s linear forwards;
  }

  @keyframes reaction-fade {
    0%, 80% { opacity: 1; }
    100%    { opacity: 0; }
  }
}

/* ─── Bouton + barre d'emojis ────────────────────────────────────────────── */
.reaction-dock {
  position: fixed;
  right: 12px;
  /* Ancré par le haut et non par `bottom` : sur mobile, la barre d'adresse se
     replie au défilement et déplace le bas de l'écran — un dock posé avec
     `bottom` suivait ce mouvement. 100svh (hauteur barre d'adresse visible)
     ne varie pas au défilement ; le dock se termine 12px au-dessus du footer
     fixe et les boutons sont poussés en bas (justify-content). */
  top: 0;
  height: calc(100vh - var(--footer-height) - 12px);
  height: calc(100svh - var(--footer-height) - 12px);
  z-index: 20;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  align-items: flex-end;
  gap: 0.4rem;
  /* Le dock couvre toute la hauteur : seuls ses boutons captent les clics. */
  pointer-events: none;
}

.reaction-dock > * {
  pointer-events: auto;
}

.reaction-fab {
  flex: none;
  display: grid;
  place-items: center;
  width: 52px;
  height: 52px;
  padding: 0;
  font-size: 1.5rem;
  line-height: 1;
  color: #4a2f00;
  background: var(--arcade-gold);
  border: 3px solid #a8720f;
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.35);
}

.reaction-dock__row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

/* Même gabarit que le bouton réactions, couleurs de la navbar. */
.immersive-fab {
  flex: none;
  display: grid;
  place-items: center;
  width: 52px;
  height: 52px;
  padding: 0;
  font-size: 1.4rem;
  line-height: 1;
  color: var(--arcade-beige);
  background: var(--arcade-blue-grey);
  border: 3px solid var(--arcade-blue-grey-dark);
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.35);
}

.immersive-fab:active,
.reaction-fab:active {
  transform: translateY(2px);
  box-shadow: 0 2px 0 rgba(0, 0, 0, 0.35);
}

.reaction-bar {
  /* 4 colonnes au-dessus du bouton : ~196px, tient à 320px. Avec les emojis
     des joueurs, la grille défile au-delà de 5 lignes. */
  display: grid;
  grid-template-columns: repeat(4, 44px);
  max-height: calc(5 * 44px + 4 * 2px + 8px + 6px);
  overflow-y: auto;
  overscroll-behavior: contain;
  gap: 2px;
  padding: 4px;
  background: var(--arcade-dark-2);
  border: 3px solid var(--arcade-blue-grey-dark);
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.35);
}

.reaction-bar__btn {
  width: 44px;
  height: 44px;
  padding: 0;
  font-size: 1.4rem;
  line-height: 1;
  background: transparent;
  border: none;
}

.reaction-bar__btn:active:not(:disabled) {
  transform: scale(1.2);
}

.reaction-bar__img {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  object-fit: cover;
  pointer-events: none;
}

.reaction-bar__empty {
  grid-column: 1 / -1;
  margin: 0;
  padding: 0.25rem;
  font-size: 0.75rem;
  color: var(--arcade-taupe);
  text-align: center;
}

.reaction-bar__btn--add {
  font-size: 1.1rem;
  color: var(--arcade-gold);
}

.reaction-bar__btn:disabled {
  opacity: 0.4;
}
</style>
