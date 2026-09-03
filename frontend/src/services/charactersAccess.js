import { reactive, computed } from 'vue'
import { api } from './api'

// Clé localStorage partagée avec l'intercepteur de requête d'`api.js`
// (qui la lit directement pour éviter un import circulaire).
export const CHARACTERS_ACCESS_KEY = 'charactersAccess'

const state = reactive({
    token: localStorage.getItem(CHARACTERS_ACCESS_KEY) || null,
})

// Computed exporté séparément : dans un template, un ref imbriqué dans un objet
// n'est pas déballé automatiquement, on l'importe donc directement.
export const isCharactersUnlocked = computed(() => !!state.token)

export const charactersAccess = {
    /**
     * Échange le mot de passe contre un token et le persiste en session.
     * Lève l'erreur axios (422) si le mot de passe est incorrect.
     */
    async unlock(password) {
        const { data } = await api.post('/access/characters', { password })
        state.token = data.token
        localStorage.setItem(CHARACTERS_ACCESS_KEY, data.token)
    },

    /** Reverrouille l'accès (oublie le token). */
    lock() {
        state.token = null
        localStorage.removeItem(CHARACTERS_ACCESS_KEY)
    },
}

// Accès direct au token pour la garde de route (hors contexte réactif).
export function getCharactersAccessToken() {
    return state.token
}
