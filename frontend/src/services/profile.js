import { reactive, computed } from 'vue'
import { charactersAccess } from './charactersAccess'

// Profil connecté, persisté dans localStorage sous la clé `profile` :
// { token, player: { id, pseudo, avatar_url, has_password, password_prompted, is_admin } }.
// api.js lit cette clé directement (sans import, pour éviter une dépendance
// circulaire) et envoie le jeton dans l'en-tête X-Profile-Token.
export const PROFILE_KEY = 'profile'

function read() {
    try {
        const saved = JSON.parse(localStorage.getItem(PROFILE_KEY) ?? 'null')
        return saved?.token && saved?.player ? saved : null
    } catch {
        return null
    }
}

const state = reactive(read() ?? { token: null, player: null })

function persist() {
    if (state.token) localStorage.setItem(PROFILE_KEY, JSON.stringify({ token: state.token, player: state.player }))
    else localStorage.removeItem(PROFILE_KEY)
}

export const profilePlayer = computed(() => state.player)
export const isLoggedIn = computed(() => !!state.token)
export const isAdminProfile = computed(() => !!state.token && !!state.player?.is_admin)

/** Connexion (onglet Connexion, ou automatique après création / arrivée dans un salon). */
export function setProfile(token, player) {
    // Changement de joueur sur ce navigateur : l'accès admin ne passe pas au suivant
    if (state.player && state.player.id !== player.id && state.player.is_admin) charactersAccess.lock()
    state.token = token
    state.player = player
    persist()
}

/** Mise à jour des infos du joueur connecté (photo, pseudo…), jeton inchangé. */
export function updateProfilePlayer(patch) {
    if (!state.player) return
    state.player = { ...state.player, ...patch }
    persist()
}

export function logout() {
    if (state.player?.is_admin) charactersAccess.lock()
    state.token = null
    state.player = null
    persist()
}

export function getProfileToken() {
    return state.token
}
