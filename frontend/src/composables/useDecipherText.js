import {ref, watch, onUnmounted} from 'vue'

// Durées partagées avec PixelVeil pour que texte et mosaïque se décodent ensemble
export const DECODE_MS = 1800
export const HIDE_MS = 300

const GLYPHS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#%&@$?!<>/\\=+*'

function randomGlyph() {
  return GLYPHS[Math.floor(Math.random() * GLYPHS.length)]
}

// Les espaces restent des espaces pour garder la forme du texte au décodage
function scramble(text) {
  return [...text].map(ch => (ch === ' ' ? ' ' : randomGlyph())).join('')
}

/**
 * Texte « chiffré » tant que `hidden` est vrai : le DOM ne contient que des
 * glyphes aléatoires (rien à lire sous le flou). Au dévoilement, les lettres
 * se résolvent de gauche à droite en défilant, sur DECODE_MS.
 *
 * @param {() => string[]} source  textes à afficher (nom, mots interdits…)
 * @param {import('vue').Ref<boolean>} hidden
 * @returns {import('vue').Ref<string[]>}
 */
export function useDecipherText(source, hidden) {
  const display = ref([])
  let frame = null

  function stop() {
    if (frame) cancelAnimationFrame(frame)
    frame = null
  }

  function decode() {
    stop()
    const targets = source()
    // Laisse les premiers pixels tomber avant que les lettres ne se fixent
    const start = performance.now() + DECODE_MS * 0.15
    const span = DECODE_MS * 0.8
    let lastTick = 0

    const step = now => {
      const progress = Math.min(1, Math.max(0, (now - start) / span))
      // ~20 rafraîchissements/s : défilement lisible plutôt que bouillie
      if (now - lastTick >= 50 || progress === 1) {
        lastTick = now
        display.value = targets.map(text => {
          const fixed = Math.floor(text.length * progress)
          return text.slice(0, fixed) + scramble(text.slice(fixed))
        })
      }
      frame = progress < 1 ? requestAnimationFrame(step) : null
    }
    frame = requestAnimationFrame(step)
  }

  function sync() {
    stop()
    display.value = hidden.value ? source().map(scramble) : source()
  }

  watch(hidden, isHidden => (isHidden ? sync() : decode()))
  // Nouveau perso (retirage, resync) : on rechiffre ou on affiche directement
  watch(() => source().join('\u0000'), sync, {immediate: true})

  onUnmounted(stop)
  return display
}
