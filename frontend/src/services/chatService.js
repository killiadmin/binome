import { api } from './api'

export const chatService = {
    /**
     * Historique récent du chat commun
     * GET /api/chat/messages
     */
    list() {
        return api.get('/chat/messages')
    },

    /**
     * Poster un message
     * POST /api/chat/messages
     * body: { body, player_id? , anon_id? }
     *
     * Le nom d'auteur est décidé par le serveur : pseudo du joueur s'il est
     * dans un salon, sinon `Anonyme#{anonId}`.
     */
    send({ body, playerId = null, anonId = null }) {
        return api.post('/chat/messages', {
            body,
            player_id: playerId,
            anon_id: anonId,
        })
    },
}
