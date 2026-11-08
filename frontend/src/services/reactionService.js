import { api } from './api'

// Emojis de réaction créés par les joueurs. L'image part en data URI JPEG
// carrée ; le serveur ne renvoie qu'une URL (voir CustomReactionController).
export const reactionService = {
    /**
     * GET /api/reactions — emojis actifs [{ id, name, url, created_by }]
     */
    async list() {
        const res = await api.get('/reactions')
        return res.data.reactions
    },

    /**
     * POST /api/reactions
     */
    async create(playerId, name, dataUrl) {
        const res = await api.post('/reactions', {
            player_id: playerId,
            name,
            image: dataUrl,
        })
        return res.data.reaction
    },
}
