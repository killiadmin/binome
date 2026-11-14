import axios from 'axios'

export const api = axios.create({
    // Same-origin: the Vite dev server proxies <base>/api to the backend container.
    baseURL: `${import.meta.env.BASE_URL}api`,
    headers: {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'X-Requested-With': 'XMLHttpRequest',
    },
})

// Joint le token d'accès « gestion des personnages » quand il est présent en
// session (clé partagée avec charactersAccess.js, lue ici sans import pour
// éviter une dépendance circulaire).
api.interceptors.request.use((config) => {
    const token = localStorage.getItem('charactersAccess')
    if (token) {
        config.headers['X-Characters-Access'] = token
    }
    // Jeton du profil connecté (clé partagée avec services/profile.js)
    try {
        const profile = JSON.parse(localStorage.getItem('profile') ?? 'null')
        if (profile?.token) config.headers['X-Profile-Token'] = profile.token
    } catch { /* clé illisible : requête sans profil */ }
    return config
})

// 401 : le jeton de profil manque ou n'est plus valide (mot de passe changé
// depuis un autre appareil, session antérieure aux jetons…). Le serveur refuse
// alors toute action au nom du joueur : on oublie profil et session de jeu, et
// on renvoie vers la connexion, qui ramène dans le salon / la partie en cours.
// Imports dynamiques : profile.js et le routeur dépendent (indirectement) d'api.js.
let redirecting = false
function sessionExpired() {
    if (redirecting) return
    redirecting = true
    let pseudo = null
    try {
        pseudo = JSON.parse(localStorage.getItem('profile') ?? 'null')?.player?.pseudo ?? null
    } catch { /* clé illisible */ }
    localStorage.removeItem('session')
    Promise.all([import('./profile'), import('../router')])
        .then(([{ logout }, { default: router }]) => {
            logout()
            if (router.currentRoute.value.name === 'Login') return
            return router.replace({ name: 'Login', query: { ...(pseudo ? { pseudo } : {}), expired: '1' } })
        })
        .finally(() => { redirecting = false })
}

api.interceptors.response.use(
    (response) => response,
    (error) => {
        const status = error.response?.status

        if (status === 401) {
            console.warn('[API] Non authentifié')
            sessionExpired()
        }

        if (status === 403) {
            console.warn('[API] Action non autorisée')
        }

        if (status === 422) {
            console.warn('[API] Erreur de validation :', error.response.data.errors)
        }

        if (status >= 500) {
            console.error('[API] Erreur serveur :', error.response.data.message)
        }

        return Promise.reject(error)
    }
)
