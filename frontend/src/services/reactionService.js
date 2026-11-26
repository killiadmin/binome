import { api } from './api'

// Emojis de réaction créés par les joueurs. L'image part en data URI JPEG
// carrée ; le serveur ne renvoie qu'une URL signée (services/social.rs).
// Un emoji naît privé (seul son créateur l'envoie) ; un profil validé peut le
// rendre public (dans la barre de tous).
export const reactionService = {
    /**
     * GET /api/reactions — la barre : Unicode, emojis publics et mes emojis privés
     * [{ id, name, url, emoji, created_by, is_public }]
     */
    async list() {
        const res = await api.get('/reactions')
        return res.data.reactions
    },

    /**
     * POST /api/reactions — isPublic réservé aux profils validés (403 sinon)
     */
    async create(playerId, name, dataUrl, isPublic = false) {
        const res = await api.post('/reactions', {
            player_id: playerId,
            name,
            image: dataUrl,
            is_public: isPublic,
        })
        return res.data.reaction
    },

    /**
     * GET /api/profile/reactions — mes emojis image, masqués compris (+ is_enabled)
     */
    async mine() {
        const res = await api.get('/profile/reactions')
        return res.data.reactions
    },

    /**
     * PATCH /api/reactions/{id} — rendre mon emoji public (profil validé) ou privé
     */
    async setPublic(id, playerId, isPublic) {
        const res = await api.patch(`/reactions/${id}`, {
            player_id: playerId,
            is_public: isPublic,
        })
        return res.data
    },
}
