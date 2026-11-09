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
        return res.data.character
    },

    /**
     * POST /games/{game}/character-vote
     * Accepter son personnage, ou demander un nouveau tirage pour tout le monde.
     * characterId = le personnage affiché : périmé après un retirage → 409.
     */
    async voteCharacter(gameId, playerId, characterId, accepted) {
        const res = await api.post(`/games/${gameId}/character-vote`, {
            player_id:    playerId,
            character_id: characterId,
            accepted,
        })
        return res.data
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
     * POST /games/{game}/rounds/{round}/actions/{action}/draw
     * Après un « Je ne sais pas », le questionneur retourne une carte face
     * cachée (index 0..card_count-1) → { word, reveal_seconds }, à lui seul.
     */
    async drawCard(gameId, roundId, actionId, playerId, card) {
        const res = await api.post(
            `/games/${gameId}/rounds/${roundId}/actions/${actionId}/draw`,
            { player_id: playerId, card }
        )
        return res.data
    },

    /**
     * POST /games/{game}/rounds/{round}/actions/{action}/draw/decision
     * Montrer le mot tiré à tous sauf la cible (share = true), ou le garder.
     */
    async decideCardDraw(gameId, roundId, actionId, playerId, share) {
        const res = await api.post(
            `/games/${gameId}/rounds/${roundId}/actions/${actionId}/draw/decision`,
            { player_id: playerId, share }
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
     * Vue omnisciente d'un joueur éliminé (option de l'hôte)
     * GET /api/games/{game}/reveal?player_id=
     */
    async reveal(gameId, playerId) {
        const res = await api.get(`/games/${gameId}/reveal`, { params: { player_id: playerId } })
        return res.data.binomes
    },
    /**
     * Vote d'un éliminé pour la meilleure question d'un round
     * POST /api/games/{game}/question-votes
     */
    async voteQuestion(gameId, playerId, actionId) {
        const res = await api.post(`/games/${gameId}/question-votes`, {
            player_id: playerId,
            action_id: actionId,
        })
        return res.data
    },

    /**
     * POST /games/{game}/reactions
     * Réaction rapide diffusée à toute la partie.
     * reaction : { reaction_id } (voir GET /reactions)
     */
    async react(gameId, playerId, reaction) {
        const res = await api.post(`/games/${gameId}/reactions`, {
            player_id: playerId,
            ...reaction,
        })
        return res.data
    },

    /**
     * POST /games/{game}/actions/{action}/correct-answer
     * L'hôte corrige une réponse oui / non.
     */
    async correctAnswer(gameId, actionId, playerId, answer) {
        const res = await api.post(`/games/${gameId}/actions/${actionId}/correct-answer`, {
            player_id: playerId,
            answer,
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
