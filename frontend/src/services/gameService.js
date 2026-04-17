import { api } from './api'

export const gameService = {

    /**
     * POST /rooms/{room}/start
     * Lance la partie (hôte uniquement)
     */
    async start(roomId, playerId) {
        const res = await api.post(`/rooms/${roomId}/start`, {
            player_id: playerId,
        })
        return res.data  // { message, game_id, status }
    },

    /**
     * GET /games/{game}
     * État courant de la partie (binomes, round actuel)
     * Ne retourne pas les personnages des joueurs
     */
    async show(gameId) {
        const res = await api.get(`/games/${gameId}`)
        console.log('[GameService] show', res.data)
        return res.data
    },

    /**
     * GET /games/{game}/me?player_id=X
     * Personnage secret du joueur connecté + mots interdits
     */
    async myCharacter(gameId, playerId) {
        const res = await api.get(`/games/${gameId}/me`, {
            params: { player_id: playerId },
        })
        console.log('[GameService] myCharacter', res.data)
        return res.data.character
    },

    /**
     * POST /games/{game}/rounds/{round}/question
     * Poser une question à un joueur
     */
    async playQuestion(gameId, roundId, playerId, targetPlayerId, question) {
        const res = await api.post(`/games/${gameId}/rounds/${roundId}/question`, {
            player_id: playerId,
            target_player_id: targetPlayerId,
            question,
        })
        return res.data
    },

    /**
     * POST /games/{game}/rounds/{round}/accusation
     * Accuser un joueur d'être un personnage
     */
    async playAccusation(gameId, roundId, playerId, targetPlayerId, characterId) {
        const res = await api.post(`/games/${gameId}/rounds/${roundId}/accusation`, {
            player_id:        playerId,
            target_player_id: targetPlayerId,
            character_id:     characterId,
        })
        return res.data
    },

    /**
     *
     * @param gameId
     * @param roundId
     * @param actionId
     * @param playerId
     * @param answer
     * @returns {Promise<any>}
     */
    async playAnswer(gameId, roundId, actionId, playerId, answer) {
        const res = await api.post(
            `/games/${gameId}/rounds/${roundId}/actions/${actionId}/answer`,
            { player_id: playerId, answer }
        )
        return res.data
    },
}
