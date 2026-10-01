import { api } from './api'

// Historique public : aucune authentification, aucun token. Seules les parties
// terminées sont exposées par ces routes (cf. HistoryController côté backend).
export const historyService = {
    /**
     * Parties terminées, la plus récente d'abord
     * GET /api/history/games
     */
    listGames() {
        return api.get('/history/games')
    },

    /**
     * Détail complet d'une partie terminée (binomes, personnages, journal)
     * GET /api/history/games/{id}
     */
    getGame(id) {
        return api.get(`/history/games/${id}`)
    },

    /**
     * Classement cumulé par pseudo
     * GET /api/history/leaderboard
     */
    leaderboard() {
        return api.get('/history/leaderboard')
    },

    /**
     * Totaux d'un joueur + détail partie par partie
     * GET /api/history/players?pseudo=…
     */
    player(pseudo) {
        return api.get('/history/players', { params: { pseudo } })
    },
}
