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
     * Supprime une partie (binomes, rounds, actions, stats). Exige le mot de
     * passe de suppression, distinct de l'accès admin (en-tête X-Delete-Password).
     * DELETE /api/admin/games/{id}
     */
    deleteGame(id, deletePassword) {
        return api.delete(`/admin/games/${id}`, { headers: { 'X-Delete-Password': deletePassword } })
    },

    /**
     * Joueurs inscrits (pseudo, parties jouées, victoires, salon actuel)
     * GET /api/admin/players
     */
    listPlayers() {
        return api.get('/admin/players')
    },

    /**
     * Toutes les réactions (Unicode + images), masquées comprises
     * GET /api/admin/reactions
     */
    listReactions() {
        return api.get('/admin/reactions')
    },

    /**
     * Renomme / masque / réactive un emoji
     * PATCH /api/admin/reactions/{id}
     */
    updateReaction(id, payload) {
        return api.patch(`/admin/reactions/${id}`, payload)
    },

    /**
     * DELETE /api/admin/reactions/{id}
     */
    deleteReaction(id) {
        return api.delete(`/admin/reactions/${id}`)
    },

    /**
     * Ajoute une réaction : { name, emoji } (Unicode) ou { name, image } (data URI JPEG)
     * POST /api/admin/reactions
     */
    createReaction(payload) {
        return api.post('/admin/reactions', payload)
    },

    /**
     * Retire un joueur de son salon (hôte transféré, salon vide supprimé)
     * DELETE /api/admin/players/{id}/room
     */
    removePlayerFromRoom(id) {
        return api.delete(`/admin/players/${id}/room`)
    },

    /**
     * Supprime un joueur ; withGames supprime aussi les parties qu'il a jouées
     * (sinon 409 + games_count s'il en a) et exige alors le mot de passe de suppression.
     * DELETE /api/admin/players/{id}
     */
    deletePlayer(id, withGames = false, deletePassword = '') {
        return api.delete(`/admin/players/${id}`, {
            params: withGames ? { with_games: 1 } : {},
            headers: withGames ? { 'X-Delete-Password': deletePassword } : {},
        })
    },
}
