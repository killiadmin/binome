<script setup>
import {ref, watch, onUnmounted} from 'vue'
import {DECODE_MS, HIDE_MS} from '../../composables/useDecipherText'

// Voile anti-regard posé sur la carte personnage : une mosaïque de pixels
// opaques aux couleurs de la borne recouvre un contenu très flouté dont la
// teinte tourne en continu (la couleur dominante du perso ne trahit rien).
// Au dévoilement, les pixels s'éteignent un à un dans un ordre aléatoire
// pendant que le flou décroît par paliers : effet « déchiffrage ».
const props = defineProps({
  hidden: {type: Boolean, default: true},
  cols: {type: Number, default: 12},
  rows: {type: Number, default: 9},
})

const PALETTE = ['#47575F', '#5A6F7D', '#9E8B7F', '#34424a', '#e0a31c', '#b3453f', '#3f7a4e', '#242424']

function randomTiles() {
  return Array.from({length: props.cols * props.rows}, () => ({
    color: PALETTE[Math.floor(Math.random() * PALETTE.length)],
    // Les derniers pixels tombent vers 80 % de la durée, la fin sert au net
    delay: Math.round(Math.random() * DECODE_MS * 0.7),
    hideDelay: Math.round(Math.random() * HIDE_MS * 0.6),
    flicker: Math.round(Math.random() * 2400),
  }))
}

const tiles = ref(randomTiles())
// hidden → decoding → clear → hiding → hidden
const phase = ref(props.hidden ? 'hidden' : 'clear')
let timer = null

watch(() => props.hidden, hidden => {
  clearTimeout(timer)
  if (hidden) {
    phase.value = 'hiding'
    timer = setTimeout(() => { phase.value = 'hidden' }, HIDE_MS)
  } else {
    // Nouvel ordre d'extinction à chaque dévoilement
    tiles.value = randomTiles()
    phase.value = 'decoding'
    timer = setTimeout(() => { phase.value = 'clear' }, DECODE_MS)
  }
})

onUnmounted(() => clearTimeout(timer))
</script>

<template>
  <div class="pixel-veil" :class="`pixel-veil--${phase}`">
    <div class="pixel-veil__content">
      <slot/>
    </div>
    <div
        class="pixel-veil__grid"
        aria-hidden="true"
        :style="{
          gridTemplateColumns: `repeat(${cols}, 1fr)`,
          gridTemplateRows: `repeat(${rows}, 1fr)`,
          '--decode-ms': `${DECODE_MS}ms`,
          '--hide-ms': `${HIDE_MS}ms`,
        }"
    >
      <span
          v-for="(t, i) in tiles"
          :key="i"
          class="pixel-veil__tile"
          :style="{
            '--c': t.color,
            '--d': `${t.delay}ms`,
            '--hd': `${t.hideDelay}ms`,
            '--f': `${t.flicker}ms`,
          }"
      ></span>
    </div>
  </div>
</template>

<style scoped>
/* Padding + marge négative : le flou déborde un peu de la carte sans être
   rogné net au bord ; overflow hidden pour que la mosaïque reste un bloc. */
.pixel-veil {
  position: relative;
  padding: 10px;
  margin: -10px;
  border-radius: 8px;
  overflow: hidden;
}

.pixel-veil__grid {
  position: absolute;
  inset: 0;
  display: grid;
  pointer-events: none;
}

.pixel-veil__tile {
  background: var(--c);
  opacity: 0;
}

/* ─── CACHÉ ─── */
.pixel-veil--hidden .pixel-veil__content,
.pixel-veil--hiding .pixel-veil__content {
  user-select: none;
  animation: veil-scramble 7s linear infinite;
}

.pixel-veil--hidden .pixel-veil__tile {
  opacity: 1;
  /* Grésillement léger : quelques pixels clignotent, la mosaïque « vit » */
  animation: tile-static 2.4s steps(1) var(--f) infinite;
}

/* ─── MASQUAGE : les pixels reviennent vite ─── */
.pixel-veil--hiding .pixel-veil__tile {
  animation: tile-in calc(var(--hide-ms) * 0.4) steps(2) var(--hd) both;
}

/* ─── DÉCHIFFRAGE ─── */
.pixel-veil--decoding .pixel-veil__content {
  user-select: none;
  animation: veil-decode var(--decode-ms) steps(8, end) both;
}

.pixel-veil--decoding .pixel-veil__tile {
  animation: tile-out 420ms steps(3, end) var(--d) both;
}

@keyframes veil-scramble {
  from { filter: blur(18px) saturate(2) contrast(1.6) hue-rotate(0deg); }
  to   { filter: blur(18px) saturate(2) contrast(1.6) hue-rotate(360deg); }
}

@keyframes veil-decode {
  0%   { filter: blur(18px) saturate(2) contrast(1.6) hue-rotate(160deg); }
  60%  { filter: blur(6px) saturate(1.4) contrast(1.25) hue-rotate(60deg); }
  100% { filter: blur(0) saturate(1) contrast(1) hue-rotate(0deg); }
}

@keyframes tile-static {
  0%, 88%  { opacity: 1; }
  92%      { opacity: 0.55; }
  96%      { opacity: 0.85; }
}

@keyframes tile-in {
  from { opacity: 0; }
  to   { opacity: 1; }
}

/* Le pixel clignote avant de s'éteindre, comme un octet qu'on décode */
@keyframes tile-out {
  0%   { opacity: 1; }
  35%  { opacity: 0.3; }
  65%  { opacity: 0.9; }
  100% { opacity: 0; }
}

@media (prefers-reduced-motion: reduce) {
  .pixel-veil--hidden .pixel-veil__content,
  .pixel-veil--hiding .pixel-veil__content {
    animation: none;
    filter: blur(18px) saturate(2) contrast(1.6) hue-rotate(150deg);
  }

  .pixel-veil--hidden .pixel-veil__tile {
    animation: none;
  }
}
</style>
