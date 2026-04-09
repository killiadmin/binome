import axios from 'axios'

export const api = axios.create({
    baseURL: import.meta.env.VITE_API_URL ?? 'http://127.0.0.1:8000/api',
    headers: {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'X-Requested-With': 'XMLHttpRequest',
    },
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
