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
     * Valide (ou retire la validation) d'un profil : accès à l'historique et aux
     * emojis personnalisés. PATCH /api/admin/players/{id}/validation — { validated }
     */
    setPlayerValidation(id, validated) {
        return api.patch(`/admin/players/${id}/validation`, { validated })
    },

    /**
     * Désigne ou retire un administrateur (propriétaire seulement ; profil avec
     * mot de passe). PATCH /api/admin/players/{id}/admin — { admin }
     */
    setPlayerAdmin(id, admin) {
        return api.patch(`/admin/players/${id}/admin`, { admin })
    },

    /**
     * Modes de jeu, cosmos et binômes proposés aux joueurs, avec leur état
     * GET /api/admin/settings → { game_modes, cosmos, binomes }
     */
    getSettings() {
        return api.get('/admin/settings')
    },

    /** PATCH /api/admin/settings/game-modes/{mode} — { enabled } */
    setGameMode(mode, enabled) {
        return api.patch(`/admin/settings/game-modes/${mode}`, { enabled })
    },

    /** PATCH /api/admin/cosmos/{id} — { is_enabled } */
    setCosmos(id, isEnabled) {
        return api.patch(`/admin/cosmos/${id}`, { is_enabled: isEnabled })
    },

    /** PATCH /api/admin/binomes — { universe_id, level_affectation, is_enabled } */
    setBinome(universeId, level, isEnabled) {
        return api.patch('/admin/binomes', { universe_id: universeId, level_affectation: level, is_enabled: isEnabled })
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
