import { api } from './api'

// Joueurs d'une partie : minimum pour la lancer, maximum d'un salon.
// Mêmes valeurs que MIN_PLAYERS / MAX_PLAYERS côté serveur (services/common.rs).
export const MIN_PLAYERS = 3
export const MAX_PLAYERS = 12

export const roomService = {
    /**
     * Créer un salon
     * POST /api/rooms
     * body: { pseudo, is_private?, max_players? }
     */
    create(pseudo, options = {}) {
        return api.post('/rooms', {
            pseudo,
            is_private:  options.isPrivate  ?? false,
            max_players: options.maxPlayers  ?? MAX_PLAYERS,
        })
    },

    /**
     * Rejoindre un salon avec un code
     * POST /api/rooms/join
     * body: { pseudo, code }
     */
    join(code, pseudo) {
        return api.post('/rooms/join', {
            pseudo,
            code: code.toUpperCase(),
        })
    },

    /**
     * État du salon (liste joueurs + statut ready)
     * GET /api/rooms/{room}
     */
    get(roomId) {
        return api.get(`/rooms/${roomId}`)
    },

    /**
     * Se mettre prêt
     * PATCH /api/rooms/{room}/ready
     * body: { player_id }
     */
    ready(roomId, playerId) {
        return api.patch(`/rooms/${roomId}/ready`, {
            player_id: playerId,
        })
    },

    /**
     * Configurer le mode de jeu (hôte uniquement)
     * PATCH /api/rooms/{room}/settings
     * body: { player_id, game_mode: 'random' | 'cosmos', cosmos_id?, reveal_to_eliminated?, best_question_vote? }
     */
    updateSettings(roomId, playerId, { gameMode, cosmosId, revealToEliminated, bestQuestionVote }) {
        return api.patch(`/rooms/${roomId}/settings`, {
            player_id: playerId,
            game_mode: gameMode,
            cosmos_id: cosmosId ?? null,
            reveal_to_eliminated: revealToEliminated,
            best_question_vote: bestQuestionVote,
        })
    },

    /**
     * Lancer la partie (créateur uniquement)
     * POST /api/rooms/{room}/start
     * body: { player_id }
     */
    start(roomId, playerId) {
        return api.post(`/rooms/${roomId}/start`, {
            player_id: playerId,
        })
    },

    /**
     * Verrouiller / déverrouiller l'entrée du salon (hôte uniquement)
     * PATCH /api/rooms/{room}/lock
     * body: { player_id, is_locked }
     */
    setLocked(roomId, playerId, isLocked) {
        return api.patch(`/rooms/${roomId}/lock`, {
            player_id: playerId,
            is_locked: isLocked,
        })
    },

    /**
     * Exclure un joueur du salon (hôte uniquement)
     * POST /api/rooms/{room}/players/{player}/kick
     * body: { player_id }
     */
    kick(roomId, playerId, targetId) {
        return api.post(`/rooms/${roomId}/players/${targetId}/kick`, {
            player_id: playerId,
        })
    },

    /**
     * Partir de la room
     * @param roomId
     * @param playerId
     * @returns {Promise<axios.AxiosResponse<any>>}
     */
    leave(roomId, playerId) {
        return api.delete(`/rooms/${roomId}/leave`, {
            data: { player_id: playerId } // axios DELETE avec body
        })
    },
}
