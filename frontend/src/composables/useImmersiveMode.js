import {ref} from 'vue'

// Mode plein écran de la partie : masque navbar et footer (App.vue) pour que
// la page occupe tout l'écran. État partagé au niveau du module.
//
// On demande aussi le plein écran du navigateur (Fullscreen API) quand il
// existe, pour masquer en plus la barre d'adresse — en bonus seulement. Sur
// iPhone, Safari ne le permet pas hors <video> ; sur mobile, un simple geste de
// défilement vers le bas suffit à le quitter. Le mode ne dépend donc PAS de
// l'événement fullscreenchange : seul le bouton le quitte, sinon il sautait
// dès qu'on remontait dans la page.

export const isImmersive = ref(false)

const canFullscreen = () =>
  typeof document.documentElement.requestFullscreen === 'function' && document.fullscreenEnabled

export async function enterImmersive() {
  isImmersive.value = true
  if (canFullscreen() && !document.fullscreenElement) {
    try {
      await document.documentElement.requestFullscreen({navigationUI: 'hide'})
    } catch {
      /* refusé par le navigateur : le masquage navbar/footer suffit */
    }
  }
}

export async function exitImmersive() {
  isImmersive.value = false
  if (document.fullscreenElement) {
    try {
      await document.exitFullscreen()
    } catch {
      /* déjà sorti */
    }
  }
}

export function toggleImmersive() {
  return isImmersive.value ? exitImmersive() : enterImmersive()
}
