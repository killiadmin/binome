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
    async playAccusation(gameId, roundId, playerId, targetPlayerId, characterName) {
        const res = await api.post(`/games/${gameId}/rounds/${roundId}/accusation`, {
            player_id:        playerId,
            target_player_id: targetPlayerId,
            character_name:   characterName,
        })
        return res.data
    },

    /**
     *
     * @param gameId
     * @param roundId
     * @param actionId
     * @param playerId
     * @param confirmed
     * @returns {Promise<any>}
     */
    async confirmAccusation(gameId, roundId, actionId, playerId, confirmed) {
        const res = await api.post(
            `/games/${gameId}/rounds/${roundId}/actions/${actionId}/confirm`,
            { player_id: playerId, confirmed }
        )
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

    /**
     * GET /games/{game}/recap
     * Récap de fin de partie (409 tant qu'elle n'est pas terminée)
     */
    async recap(gameId) {
        const res = await api.get(`/games/${gameId}/recap`)
        return res.data
    },

    /**
     * POST /games/{game}/reactions
     * Réaction rapide (emoji) diffusée à toute la partie
     */
    async react(gameId, playerId, emoji) {
        const res = await api.post(`/games/${gameId}/reactions`, {
            player_id: playerId,
            emoji,
        })
        return res.data
    },

    /**
     * POST /games/{game}/skip-turn
     * L'hôte passe le tour du joueur déconnecté qui met la partie en pause
     */
    async skipTurn(gameId, playerId) {
        const res = await api.post(`/games/${gameId}/skip-turn`, {
            player_id: playerId,
        })
        return res.data
    },

    /**
     * POST /games/{game}/players/{player}/exclude
     * L'hôte exclut un joueur déconnecté (lui et son binôme sont éliminés)
     */
    async excludePlayer(gameId, targetPlayerId, playerId) {
        const res = await api.post(`/games/${gameId}/players/${targetPlayerId}/exclude`, {
            player_id: playerId,
        })
        return res.data
    },
}
