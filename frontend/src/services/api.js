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

api.interceptors.response.use(
    (response) => response,
    (error) => {
        const status = error.response?.status

        if (status === 401) {
            console.warn('[API] Non authentifié')
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
