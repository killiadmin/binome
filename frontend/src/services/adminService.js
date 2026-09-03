import { api } from './api'

// Le token d'accès (X-Characters-Access) est injecté automatiquement par
// l'intercepteur de requête d'`api.js` — mêmes identifiants que la gestion
// des personnages.
export const adminService = {
    /**
     * Liste de toutes les parties (en cours + terminées)
     * GET /api/admin/games
     */
    listGames() {
        return api.get('/admin/games')
    },

    /**
     * Détail complet d'une partie (binomes + personnages révélés + actions)
     * GET /api/admin/games/{id}
     */
    getGame(id) {
        return api.get(`/admin/games/${id}`)
    },

    /**
     * Supprime une partie (cascade DB : binomes, rounds, actions, stats)
     * DELETE /api/admin/games/{id}
     */
    deleteGame(id) {
        return api.delete(`/admin/games/${id}`)
    },
}
