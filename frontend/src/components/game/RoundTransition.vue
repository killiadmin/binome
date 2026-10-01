<script setup>
/*
  Cinématique « nouveau round », style borne d'arcade.

  Chronologie :
    0 → T_IN          mosaïque de pixels qui recouvre l'écran (des bords vers le centre)
    T_IN → T_IMPACT   hyperespace : les étoiles-pixels accélèrent, « ROUND » tombe lettre par lettre
    T_IMPACT          le numéro s'écrase : flash, onde de choc, gerbe de pixels, secousse
    T_OUT_START       extinction façon écran CRT + mosaïque qui se dissout depuis le centre

  Le canvas est rendu en basse résolution (1 pixel logique = PX pixels CSS) puis
  agrandi avec `image-rendering: pixelated` : tout est net et « gros pixel » sans
  coût de rendu. Les délais CSS sont dérivés des mêmes constantes (variables CSS).
*/
import {ref, onMounted, onUnmounted} from 'vue'

defineProps({
  round: {type: [Number, String], required: true},
})
const emit = defineEmits(['done'])

const T_IN = 450
const T_IMPACT = 950
const T_OUT_START = 2750
const T_OUT = 550
const TOTAL = T_OUT_START + T_OUT + 50

const PX = 4      // pixels CSS par pixel logique
const BLOCK = 6   // pixels logiques par bloc de mosaïque
const STAR_COUNT = 160
const PARTICLE_COUNT = 90
const PALETTE = ['#F5F5DC', '#e0a31c', '#9E8B7F', '#5A6F7D']

const timing = {
  '--t-in': `${T_IN}ms`,
  '--t-impact': `${T_IMPACT}ms`,
  '--t-out-start': `${T_OUT_START}ms`,
  '--t-out': `${T_OUT}ms`,
}

const canvas = ref(null)
const reducedMotion = typeof window !== 'undefined'
    && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches

let raf = 0
let doneTimer = 0
let onResize = null

const clamp01 = v => Math.min(1, Math.max(0, v))
const easeOutCubic = t => 1 - Math.pow(1 - t, 3)
const pick = arr => arr[(Math.random() * arr.length) | 0]

function run(el) {
  const ctx = el.getContext('2d')
  let w, h, cx, cy, maxR, blocks

  function resize() {
    const rect = el.getBoundingClientRect()
    w = Math.max(1, Math.ceil(rect.width / PX))
    h = Math.max(1, Math.ceil(rect.height / PX))
    el.width = w
    el.height = h
    cx = w / 2
    cy = h / 2
    maxR = Math.hypot(cx, cy)

    // Chaque bloc a un seuil d'apparition lié à sa distance au centre (+ bruit),
    // et une teinte légèrement plus claire au centre : vignette tramée.
    blocks = []
    const cols = Math.ceil(w / BLOCK)
    const rows = Math.ceil(h / BLOCK)
    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < cols; x++) {
        const d = Math.hypot(x * BLOCK + BLOCK / 2 - cx, y * BLOCK + BLOCK / 2 - cy) / maxR
        const shade = 1 - d
        const n = (Math.random() * 6) | 0
        blocks.push({
          x: x * BLOCK,
          y: y * BLOCK,
          t: Math.min(1, d * 0.8 + Math.random() * 0.2),
          color: `rgb(${Math.round(26 + shade * 18 + n)},${Math.round(31 + shade * 24 + n)},${Math.round(36 + shade * 28 + n)})`,
        })
      }
    }
  }

  resize()
  onResize = resize
  window.addEventListener('resize', onResize)

  const stars = Array.from({length: STAR_COUNT}, () => resetStar({}, true))
  function resetStar(s, anywhere) {
    s.a = Math.random() * Math.PI * 2
    s.r = anywhere ? Math.random() * maxR : Math.random() * 6
    s.v = 0.4 + Math.random() * 0.8
    s.c = pick(PALETTE)
    s.len = 1
    return s
  }

  let particles = null
  const rings = [
    {start: T_IMPACT, dur: 650, color: '#e0a31c', size: 3},
    {start: T_IMPACT + 110, dur: 780, color: '#F5F5DC', size: 2},
  ]

  // Vitesse de l'hyperespace : accélère jusqu'à l'impact puis retombe en croisière.
  function warp(e) {
    if (e < T_IMPACT) return 0.05 + Math.pow(e / T_IMPACT, 2)
    return 0.08 + 0.97 * Math.exp(-(e - T_IMPACT) / 260)
  }

  const start = performance.now()
  let last = start

  function frame(now) {
    const e = now - start
    const dt = Math.min(50, now - last)
    last = now
    if (e > TOTAL) {
      ctx.clearRect(0, 0, w, h)
      return
    }

    ctx.clearRect(0, 0, w, h)
    ctx.globalAlpha = 1

    // ── Mosaïque de fond ────────────────────────────────────────────────
    const pIn = clamp01(e / T_IN) * 1.1
    const pOut = clamp01((e - T_OUT_START) / T_OUT) * 1.1
    const edgeIn = e < T_IN
    const edgeOut = e > T_OUT_START
    for (const b of blocks) {
      const mIn = pIn - (1 - b.t)
      const mOut = b.t - pOut
      if (mIn < 0 || mOut < 0) continue
      if (edgeIn && mIn < 0.12) ctx.fillStyle = mIn < 0.05 ? '#F5F5DC' : '#e0a31c'
      else if (edgeOut && mOut < 0.1) ctx.fillStyle = mOut < 0.04 ? '#F5F5DC' : '#5A6F7D'
      else ctx.fillStyle = b.color
      ctx.fillRect(b.x, b.y, BLOCK, BLOCK)
    }

    const fx = clamp01(e / T_IN) * clamp01(1 - pOut * 1.6)

    // ── Hyperespace ─────────────────────────────────────────────────────
    const speed = warp(e)
    for (const s of stars) {
      const step = s.v * speed * (1 + s.r * 0.04) * dt * 0.06
      s.r += step
      s.len = Math.max(1, Math.min(40, step * 3))
      if (s.r - s.len > maxR) resetStar(s, false)
      const cos = Math.cos(s.a)
      const sin = Math.sin(s.a)
      ctx.fillStyle = s.c
      for (let k = 0; k < s.len; k++) {
        ctx.globalAlpha = fx * (k === 0 ? 1 : 0.45 * (1 - k / s.len))
        ctx.fillRect((cx + cos * (s.r - k)) | 0, (cy + sin * (s.r - k)) | 0, 1, 1)
      }
    }

    // ── Impact : flash, ondes de choc, gerbe de pixels ─────────────────
    if (e >= T_IMPACT) {
      const pf = (e - T_IMPACT) / 220
      if (pf < 1) {
        ctx.globalAlpha = 0.55 * (1 - pf) * fx
        ctx.fillStyle = '#F5F5DC'
        ctx.fillRect(0, 0, w, h)
      }

      for (const ring of rings) {
        const p = (e - ring.start) / ring.dur
        if (p < 0 || p >= 1) continue
        const r = easeOutCubic(p) * maxR * 1.05
        const size = Math.max(1, Math.round(ring.size * (1 - p)) + 1)
        const count = Math.max(12, Math.ceil(Math.PI * 2 * r))
        ctx.globalAlpha = (1 - p * 0.7) * fx
        ctx.fillStyle = ring.color
        for (let i = 0; i < count; i++) {
          const a = (i / count) * Math.PI * 2
          ctx.fillRect((cx + Math.cos(a) * r) | 0, (cy + Math.sin(a) * r) | 0, size, size)
        }
      }

      if (!particles) {
        particles = Array.from({length: PARTICLE_COUNT}, () => {
          const a = Math.random() * Math.PI * 2
          const v = 0.08 + Math.random() * 0.3
          return {
            x: cx, y: cy,
            vx: Math.cos(a) * v,
            vy: Math.sin(a) * v - 0.06,
            life: 900 + Math.random() * 700,
            age: 0,
            size: Math.random() < 0.3 ? 2 : 1,
            c: pick(PALETTE),
          }
        })
      }
      for (const pt of particles) {
        pt.age += dt
        if (pt.age >= pt.life) continue
        pt.vy += 0.0004 * dt
        pt.x += pt.vx * dt
        pt.y += pt.vy * dt
        // Scintillement « 8 bits » : la particule clignote en fin de vie.
        const k = pt.age / pt.life
        if (k > 0.6 && ((pt.age / 70) | 0) % 2) continue
        ctx.globalAlpha = fx
        ctx.fillStyle = pt.c
        ctx.fillRect(pt.x | 0, pt.y | 0, pt.size, pt.size)
      }
    }

    raf = requestAnimationFrame(frame)
  }

  raf = requestAnimationFrame(frame)
}

onMounted(() => {
  // Le `done` ne dépend pas de requestAnimationFrame : un onglet en arrière-plan
  // (rAF gelé) ne doit pas bloquer la partie.
  doneTimer = setTimeout(() => emit('done'), TOTAL)
  if (!reducedMotion && canvas.value) run(canvas.value)
})

onUnmounted(() => {
  cancelAnimationFrame(raf)
  clearTimeout(doneTimer)
  if (onResize) window.removeEventListener('resize', onResize)
})
</script>

<template>
  <div class="rt" :style="timing" role="status" aria-live="polite">
    <div class="rt-stage">
      <canvas ref="canvas" class="rt-canvas" aria-hidden="true"></canvas>
      <div class="rt-crt" aria-hidden="true"></div>

      <div class="rt-content" aria-hidden="true">
        <div class="rt-label">
          <span v-for="(letter, i) in 'ROUND'" :key="i" :style="{'--i': i}">{{ letter }}</span>
        </div>
        <div class="rt-number" :data-text="round">{{ round }}</div>
        <div class="rt-ready">PRETS ?</div>
      </div>
    </div>
    <span class="visually-hidden">Round {{ round }}</span>
  </div>
</template>

<style scoped>
.rt {
  position: fixed;
  inset: 0;
  z-index: 1100;
  overflow: hidden;
  pointer-events: none;
}

.rt-stage {
  position: absolute;
  inset: 0;
  animation: rt-shake 380ms steps(1, end) var(--t-impact);
}

/* Débord de 16px : la secousse ne laisse jamais voir la page derrière. */
.rt-canvas {
  position: absolute;
  top: -16px;
  left: -16px;
  width: calc(100% + 32px);
  height: calc(100% + 32px);
  image-rendering: pixelated;
  image-rendering: crisp-edges;
}

/* Scanlines + vignette d'écran cathodique. */
.rt-crt {
  position: absolute;
  inset: 0;
  opacity: 0;
  background:
      radial-gradient(ellipse at center, transparent 55%, rgba(0, 0, 0, 0.55) 100%),
      repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.22) 0 2px, transparent 2px 4px);
  animation:
      rt-fade-in 200ms linear calc(var(--t-in) * 0.5) forwards,
      rt-fade-out 250ms linear var(--t-out-start) forwards;
}

.rt-content {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1rem;
  font-family: 'Press Start 2P', cursive;
  text-align: center;
  animation: rt-crt-off 420ms steps(1, end) var(--t-out-start) forwards;
}

/* ── « ROUND » : chute lettre par lettre ──────────────────────────────── */
.rt-label {
  display: flex;
  gap: 0.35em;
  font-size: clamp(0.85rem, 4.5vw, 1.4rem);
  color: var(--arcade-gold);
  text-shadow: 3px 3px 0 var(--arcade-dark);
}

.rt-label span {
  display: inline-block;
  opacity: 0;
  animation: rt-drop 340ms steps(6, end) forwards;
  animation-delay: calc(var(--t-in) * 0.7 + var(--i) * 70ms);
}

/* ── Numéro : écrasement + séparation des couleurs ──────────────────── */
.rt-number {
  position: relative;
  font-size: clamp(4.5rem, 24vw, 9rem);
  line-height: 1;
  color: var(--arcade-beige);
  opacity: 0;
  text-shadow:
      5px 5px 0 var(--arcade-taupe),
      10px 10px 0 rgba(0, 0, 0, 0.45),
      0 0 40px rgba(224, 163, 28, 0.55);
  animation:
      rt-slam 300ms cubic-bezier(0.2, 0.9, 0.3, 1.35) calc(var(--t-impact) - 300ms) forwards,
      rt-glow 900ms steps(2, end) calc(var(--t-impact) + 300ms) infinite alternate;
}

.rt-number::before,
.rt-number::after {
  content: attr(data-text);
  position: absolute;
  inset: 0;
  z-index: -1;
  opacity: 0;
  text-shadow: none;
  animation: rt-split 460ms steps(7, end) var(--t-impact) forwards;
}

.rt-number::before {
  color: var(--arcade-gold);
  --dx: -16px;
}

.rt-number::after {
  color: var(--arcade-blue-grey);
  --dx: 16px;
}

.rt-ready {
  font-size: clamp(0.6rem, 3vw, 0.85rem);
  letter-spacing: 0.2em;
  color: var(--arcade-beige);
  opacity: 0;
  animation: rt-blink 520ms steps(1, end) calc(var(--t-impact) + 380ms) infinite;
}

/* ── Keyframes ───────────────────────────────────────────────────────── */
@keyframes rt-drop {
  0%   { opacity: 1; transform: translateY(-180%); }
  65%  { transform: translateY(15%); }
  100% { opacity: 1; transform: translateY(0); }
}

@keyframes rt-slam {
  0%   { opacity: 0; transform: scale(3.6); }
  40%  { opacity: 1; }
  100% { opacity: 1; transform: scale(1); }
}

@keyframes rt-glow {
  from { text-shadow: 5px 5px 0 var(--arcade-taupe), 10px 10px 0 rgba(0, 0, 0, 0.45), 0 0 30px rgba(224, 163, 28, 0.4); }
  to   { text-shadow: 5px 5px 0 var(--arcade-taupe), 10px 10px 0 rgba(0, 0, 0, 0.45), 0 0 70px rgba(224, 163, 28, 0.85); }
}

@keyframes rt-split {
  0%   { opacity: 0.9; transform: translate(var(--dx), 3px); }
  100% { opacity: 0; transform: translate(0, 0); }
}

@keyframes rt-blink {
  0%  { opacity: 1; }
  50% { opacity: 0; }
}

@keyframes rt-shake {
  0%   { transform: translate(7px, -5px); }
  15%  { transform: translate(-7px, 5px); }
  30%  { transform: translate(5px, 4px); }
  45%  { transform: translate(-4px, -3px); }
  60%  { transform: translate(3px, -2px); }
  80%  { transform: translate(-1px, 1px); }
  100% { transform: translate(0, 0); }
}

/* Extinction d'écran cathodique : glitch en tranches puis écrasement en ligne. */
@keyframes rt-crt-off {
  0%   { transform: translate(0, 0); clip-path: inset(0 0 0 0); opacity: 1; }
  18%  { transform: translate(-10px, 0); clip-path: inset(8% 0 55% 0); }
  36%  { transform: translate(12px, 0); clip-path: inset(45% 0 18% 0); }
  54%  { transform: translate(-4px, 0) scaleY(0.6); clip-path: inset(28% 0 30% 0); }
  72%  { transform: scale(1.6, 0.06); clip-path: inset(0 0 0 0); }
  88%  { transform: scale(0.2, 0.03); }
  100% { transform: scale(0, 0); opacity: 0; }
}

@keyframes rt-fade-in {
  to { opacity: 1; }
}

@keyframes rt-fade-out {
  from { opacity: 1; }
  to   { opacity: 0; }
}

/* Mouvement réduit : pas de canvas ni de secousse, un simple fondu lisible. */
@media (prefers-reduced-motion: reduce) {
  .rt {
    background: rgba(28, 34, 38, 0.96);
    opacity: 0;
    animation:
        rt-fade-in 250ms linear forwards,
        rt-fade-out 300ms linear var(--t-out-start) forwards;
  }

  .rt *,
  .rt *::before,
  .rt *::after {
    animation: none !important;
  }

  .rt-label span,
  .rt-number,
  .rt-ready {
    opacity: 1;
  }
}
</style>
