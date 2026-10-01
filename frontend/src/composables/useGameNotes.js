import {reactive, watch} from 'vue'

// Bloc-notes privé d'une partie : n'existe que dans le localStorage de ce
// navigateur, jamais envoyé au serveur. Une clé par (partie, joueur) pour que
// deux joueurs qui partagent un appareil ne lisent pas les notes l'un de l'autre.
//
// Forme : { general: '', players: { [playerId]: '' } }

const PREFIX = 'binome:notes:'

function read(key) {
  try {
    const parsed = JSON.parse(localStorage.getItem(key) ?? 'null')
    return {general: parsed?.general ?? '', players: parsed?.players ?? {}}
  } catch {
    return {general: '', players: {}}
  }
}

function write(key, notes) {
  try {
    localStorage.setItem(key, JSON.stringify(notes))
  } catch {
    /* stockage plein ou bloqué : les notes restent en mémoire pour la session */
  }
}

// Les notes d'une partie ne servent plus une fois qu'on en joue une autre.
function purgeOtherGames(gameId) {
  try {
    Object.keys(localStorage)
        .filter(k => k.startsWith(PREFIX) && !k.startsWith(`${PREFIX}${gameId}:`))
        .forEach(k => localStorage.removeItem(k))
  } catch {
    /* accès au stockage refusé : rien à nettoyer */
  }
}

export function useGameNotes(gameId, playerId) {
  const key = `${PREFIX}${gameId}:${playerId}`
  purgeOtherGames(gameId)

  const notes = reactive(read(key))
  watch(notes, () => write(key, notes), {deep: true})

  return notes
}
