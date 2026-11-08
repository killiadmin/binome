<script setup>
import {computed, onMounted, onUnmounted, ref} from 'vue'
import PlayerAvatar from '../player/PlayerAvatar.vue'

// Annonce du (des) grand(s) gagnant(s) en fin de partie : carte du
// personnage, photo du joueur, et feu d'artifice pixel-art en fond.
//
// winners : [{ pseudo, avatar_url, character, character_image, universe, score, is_orphan }]
const props = defineProps({
  winners: {type: Array, required: true},
})

const title = computed(() => props.winners.length > 1 ? 'Grands gagnants' : 'Grand gagnant')

// Couronne 9×6 en pixels : X = or, h = reflet, r / b = gemmes
const CROWN = [
  'X...X...X',
  'XX.XhX.XX',
  'XhXXXXXhX',
  'XXXXXXXXX',
  'XrXXbXXrX',
  'XXXXXXXXX',
]
const CROWN_COLORS = {X: '#e0a31c', h: '#ffd876', r: '#b3453f', b: '#5A9BC2'}
const crownPixels = CROWN.flatMap((row, y) =>
    [...row].map((c, x) => ({x, y, fill: CROWN_COLORS[c]})).filter(p => p.fill))

// ── Feu d'artifice ───────────────────────────────────────────────────────────
// Le canvas est dessiné à basse résolution (1 pixel logique = PX pixels CSS)
// puis agrandi sans lissage : chaque étincelle est un vrai « gros pixel ».
const PX = 4
const DURATION_MS = 9000
const GRAVITY = 0.012
const COLORS = ['#e0a31c', '#ffd876', '#F5F5DC', '#e5675f', '#7fd18f', '#8fc3e0', '#c9b5a6']

const canvas = ref(null)
let ctx = null
let raf = null
let resizeObserver = null
let rockets = []
let sparks = []
let startedAt = 0
let lastFrame = 0
let nextLaunch = 0

const pick = arr => arr[Math.floor(Math.random() * arr.length)]
const rand = (min, max) => min + Math.random() * (max - min)

function resize() {
  const el = canvas.value
  if (!el) return
  el.width = Math.max(1, Math.floor(el.clientWidth / PX))
  el.height = Math.max(1, Math.floor(el.clientHeight / PX))
  ctx = el.getContext('2d')
  ctx.imageSmoothingEnabled = false
}

function launch() {
  const {width: w, height: h} = canvas.value
  rockets.push({
    x: rand(w * 0.1, w * 0.9),
    y: h,
    vy: -rand(0.9, 1.25),
    targetY: rand(h * 0.12, h * 0.45),
    color: pick(COLORS),
  })
}

function explode(r) {
  const count = Math.floor(rand(22, 34))
  const speed = rand(0.45, 0.75)
  // Une gerbe sur trois est bicolore
  const second = Math.random() < 0.33 ? pick(COLORS) : r.color
  for (let i = 0; i < count; i++) {
    const angle = (i / count) * Math.PI * 2 + rand(-0.08, 0.08)
    const v = speed * rand(0.75, 1)
    const life = rand(55, 85)
    sparks.push({
      x: r.x, y: r.y, px: r.x, py: r.y,
      vx: Math.cos(angle) * v,
      vy: Math.sin(angle) * v,
      life, maxLife: life,
      color: i % 2 ? second : r.color,
    })
  }
  // Cœur blanc qui scintille au point d'éclatement
  for (let i = 0; i < 6; i++) {
    sparks.push({
      x: r.x, y: r.y, px: r.x, py: r.y,
      vx: rand(-0.15, 0.15), vy: rand(-0.15, 0.15),
      life: 20, maxLife: 20, color: '#ffffff',
    })
  }
}

function frame(now) {
  // dt en « images à 60 Hz », borné pour qu'un onglet en veille ne fasse
  // pas tout exploser d'un coup au retour.
  const dt = Math.min(3, (now - lastFrame) / (1000 / 60))
  lastFrame = now
  const elapsed = now - startedAt

  if (elapsed < DURATION_MS && now >= nextLaunch) {
    launch()
    // Salve d'ouverture plus nourrie
    nextLaunch = now + (elapsed < 1500 ? rand(180, 320) : rand(450, 850))
  }

  const {width: w, height: h} = canvas.value
  ctx.clearRect(0, 0, w, h)

  rockets = rockets.filter(r => {
    r.y += r.vy * dt
    // Traînée de la fusée
    ctx.globalAlpha = 0.45
    ctx.fillStyle = '#ffd876'
    ctx.fillRect(Math.round(r.x), Math.round(r.y) + 2, 1, 2)
    ctx.globalAlpha = 1
    ctx.fillStyle = r.color
    ctx.fillRect(Math.round(r.x), Math.round(r.y), 1, 2)
    if (r.y <= r.targetY) {
      explode(r)
      return false
    }
    return true
  })

  sparks = sparks.filter(s => {
    s.px = s.x
    s.py = s.y
    s.vy += GRAVITY * dt
    s.vx *= 0.985
    s.vy *= 0.985
    s.x += s.vx * dt
    s.y += s.vy * dt
    s.life -= dt
    if (s.life <= 0) return false

    const t = s.life / s.maxLife
    // En fin de vie, les étincelles clignotent avant de s'éteindre
    if (t < 0.3 && Math.random() < 0.4) return true
    ctx.globalAlpha = 0.35 * t
    ctx.fillStyle = s.color
    ctx.fillRect(Math.round(s.px), Math.round(s.py), 1, 1)
    ctx.globalAlpha = Math.min(1, t * 1.6)
    ctx.fillRect(Math.round(s.x), Math.round(s.y), 1, 1)
    return true
  })
  ctx.globalAlpha = 1

  if (elapsed < DURATION_MS || rockets.length || sparks.length) {
    raf = requestAnimationFrame(frame)
  } else {
    raf = null
  }
}

onMounted(() => {
  if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return
  resize()
  if (!ctx) return
  resizeObserver = new ResizeObserver(resize)
  resizeObserver.observe(canvas.value)
  startedAt = lastFrame = nextLaunch = performance.now()
  raf = requestAnimationFrame(frame)
})

onUnmounted(() => {
  if (raf) cancelAnimationFrame(raf)
  resizeObserver?.disconnect()
})
</script>

<template>
  <section class="winner-stage" :aria-label="title">
    <div class="winner-stage__rays" aria-hidden="true"></div>
    <canvas ref="canvas" class="winner-stage__fireworks" aria-hidden="true"></canvas>

    <div class="winner-stage__content">
      <p class="winner-stage__title">{{ title }}</p>

      <div class="winner-cards">
        <article
            v-for="(w, i) in winners"
            :key="w.pseudo"
            class="winner-card"
            :style="{'--delay': `${0.35 + i * 0.25}s`}"
        >
          <svg class="winner-card__crown" viewBox="0 0 9 6" shape-rendering="crispEdges" aria-hidden="true">
            <rect v-for="p in crownPixels" :key="`${p.x}-${p.y}`" :x="p.x" :y="p.y" width="1" height="1" :fill="p.fill"/>
          </svg>

          <div class="winner-card__portrait">
            <div class="pixel-frame winner-card__frame">
              <img v-if="w.character_image" :src="w.character_image" :alt="w.character ?? ''" class="winner-card__img"/>
              <div v-else class="winner-card__placeholder"><span>?</span></div>
            </div>
            <span class="winner-card__avatar">
              <PlayerAvatar :pseudo="w.pseudo" :url="w.avatar_url"/>
            </span>
          </div>

          <p class="winner-card__pseudo">{{ w.pseudo }}</p>
          <p class="winner-card__character">{{ w.character ?? '???' }}</p>
          <span v-if="w.universe" class="winner-card__universe">{{ w.universe }}</span>
          <span v-if="w.score != null" class="winner-card__score">{{ w.score }} pts</span>
          <span v-if="w.is_orphan" class="winner-card__orphan">🚷 Orphelin</span>
        </article>
      </div>
    </div>
  </section>
</template>

<style scoped>
.winner-stage {
  position: relative;
  overflow: hidden;
  border: 3px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 1.25rem 0.5rem 1.5rem;
  margin-bottom: 1.25rem;
  background:
      radial-gradient(circle at 50% 30%, rgba(224, 163, 28, 0.22), transparent 60%),
      repeating-linear-gradient(0deg, rgba(255, 255, 255, 0.03) 0 1px, transparent 1px 16px),
      repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.03) 0 1px, transparent 1px 16px),
      var(--arcade-dark-2);
  box-shadow: inset 0 0 0 2px #34424a, 0 4px 0 rgba(0, 0, 0, 0.3);
}

/* Rayons de lumière qui tournent lentement derrière les cartes */
.winner-stage__rays {
  position: absolute;
  left: 50%;
  top: 58%;
  width: 640px;
  height: 640px;
  margin: -320px 0 0 -320px;
  background: repeating-conic-gradient(rgba(224, 163, 28, 0.13) 0deg 10deg, transparent 10deg 30deg);
  -webkit-mask-image: radial-gradient(circle, #000 15%, transparent 60%);
  mask-image: radial-gradient(circle, #000 15%, transparent 60%);
  animation: ws-rays 24s linear infinite;
  pointer-events: none;
}

.winner-stage__fireworks {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  image-rendering: pixelated;
  pointer-events: none;
}

.winner-stage__content {
  position: relative;
  z-index: 1;
}

.winner-stage__title {
  font-family: 'Press Start 2P', cursive;
  font-size: clamp(0.8rem, 4.2vw, 1.15rem);
  line-height: 1.5;
  text-align: center;
  text-transform: uppercase;
  color: var(--arcade-gold);
  text-shadow: 3px 3px 0 #4a2f00, 0 0 18px rgba(224, 163, 28, 0.6);
  margin: 0 0 1.4rem;
  animation: ws-slam 0.5s steps(5) both, ws-blink 1.2s steps(2) 0.6s 3;
}

.winner-cards {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.6rem;
}

.winner-card {
  position: relative;
  width: 112px;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.3rem;
  padding: 1.1rem 0.5rem 0.75rem;
  background: linear-gradient(180deg, #fff6de, var(--arcade-beige));
  border: 3px solid var(--arcade-gold);
  border-radius: 10px;
  box-shadow: 0 5px 0 #a8720f, 0 0 26px rgba(224, 163, 28, 0.45);
  text-align: center;
  animation: ws-drop 0.6s steps(6) var(--delay) both;
}

.winner-card__crown {
  position: absolute;
  top: -22px;
  left: 50%;
  width: 36px;
  height: 24px;
  transform: translateX(-50%);
  filter: drop-shadow(2px 2px 0 rgba(0, 0, 0, 0.45));
  animation: ws-bob 1.2s steps(2) calc(var(--delay) + 0.6s) infinite;
}

.winner-card__portrait {
  position: relative;
  margin-bottom: 0.5rem;
}

.winner-card__frame {
  width: 84px;
  height: 108px;
}

.winner-card__img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.winner-card__placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: 'Press Start 2P', cursive;
  font-size: 2rem;
  color: var(--arcade-taupe);
}

/* Photo du joueur en médaillon, à cheval sur le coin du portrait */
.winner-card__avatar {
  position: absolute;
  right: -14px;
  bottom: -12px;
  z-index: 2;
  width: 46px;
  height: 46px;
  border-radius: 50%;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--arcade-blue-grey);
  border: 3px solid var(--arcade-gold);
  box-shadow: 0 3px 0 rgba(0, 0, 0, 0.35);
  font-family: 'Press Start 2P', cursive;
  font-size: 0.65rem;
  color: var(--arcade-beige);
}

.winner-card__pseudo {
  margin: 0;
  max-width: 100%;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.55rem;
  line-height: 1.5;
  color: var(--arcade-dark);
  overflow-wrap: anywhere;
}

.winner-card__character {
  margin: 0;
  max-width: 100%;
  font-family: 'Baloo 2', sans-serif;
  font-weight: 800;
  font-size: 0.95rem;
  line-height: 1.15;
  color: var(--arcade-blue-grey-dark);
  overflow-wrap: anywhere;
}

.winner-card__universe {
  max-width: 100%;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.42rem;
  line-height: 1.5;
  text-transform: uppercase;
  padding: 3px 6px;
  border: 2px solid #a8720f;
  border-radius: 4px;
  background: linear-gradient(180deg, #ffd876, var(--arcade-gold));
  color: #4a2f00;
  overflow-wrap: anywhere;
}

.winner-card__score {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.6rem;
  color: var(--arcade-success-dark);
  margin-top: 0.15rem;
}

.winner-card__orphan {
  font-family: 'Baloo 2', sans-serif;
  font-weight: 700;
  font-size: 0.8rem;
  color: var(--arcade-blue-grey-dark);
}

@media (min-width: 380px) {
  .winner-stage { padding-inline: 0.75rem; }
  .winner-cards { gap: 0.9rem; }
  .winner-card { width: 132px; }
  .winner-card__frame { width: 96px; height: 122px; }
  .winner-card__pseudo { font-size: 0.62rem; }
}

@keyframes ws-rays {
  to { transform: rotate(360deg); }
}

@keyframes ws-slam {
  from { transform: scale(2.2); opacity: 0; }
  to   { transform: scale(1); opacity: 1; }
}

@keyframes ws-blink {
  50% { color: var(--arcade-beige); }
}

@keyframes ws-drop {
  0%   { transform: translateY(-40px); opacity: 0; }
  70%  { transform: translateY(4px); opacity: 1; }
  100% { transform: translateY(0); opacity: 1; }
}

@keyframes ws-bob {
  50% { transform: translate(-50%, -3px); }
}

@media (prefers-reduced-motion: reduce) {
  .winner-stage__rays,
  .winner-stage__title,
  .winner-card,
  .winner-card__crown {
    animation: none;
  }
}
</style>
