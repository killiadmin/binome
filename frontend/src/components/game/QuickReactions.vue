<script setup>
import {ref} from 'vue'

// Réactions rapides : un bouton flottant qui déplie la barre d'emojis, et une
// couche d'animation où montent les réactions reçues (les siennes comprises,
// elles reviennent par le broadcast comme celles des autres).

// À garder aligné sur GameController::REACTIONS côté backend.
const EMOJIS = ['😂', '🤔', '😱', '👏', '🔥', '🤫', '😈', '💀']

// Délai minimal entre deux envois, en plus du throttle serveur.
const COOLDOWN_MS = 1200

defineProps({
  // [{ id, emoji, pseudo, x }] — x : position horizontale en %
  reactions: {type: Array, default: () => []},
})

const emit = defineEmits(['send'])

const open = ref(false)
const coolingDown = ref(false)

function send(emoji) {
  if (coolingDown.value) return
  emit('send', emoji)
  coolingDown.value = true
  setTimeout(() => { coolingDown.value = false }, COOLDOWN_MS)
}
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
      <span class="reaction-float__emoji">{{ r.emoji }}</span>
      <span class="reaction-float__pseudo">{{ r.pseudo }}</span>
    </div>
  </div>

  <!-- Envoi -->
  <div class="reaction-dock">
    <div v-if="open" class="reaction-bar" role="toolbar" aria-label="Réactions rapides">
      <button
          v-for="emoji in EMOJIS"
          :key="emoji"
          type="button"
          class="reaction-bar__btn"
          :disabled="coolingDown"
          :aria-label="`Réagir ${emoji}`"
          @click="send(emoji)"
      >{{ emoji }}</button>
    </div>
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
  bottom: calc(var(--navbar-height) + 4rem);
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
  /* au-dessus du footer fixe */
  bottom: calc(var(--navbar-height) + 12px);
  z-index: 20;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 0.4rem;
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

.reaction-fab:active {
  transform: translateY(2px);
  box-shadow: 0 2px 0 rgba(0, 0, 0, 0.35);
}

.reaction-bar {
  /* 4 × 2 au-dessus du bouton : ~196px, tient à 320px */
  display: grid;
  grid-template-columns: repeat(4, 44px);
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

.reaction-bar__btn:disabled {
  opacity: 0.4;
}
</style>
