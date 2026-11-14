import { api } from './api'

// Boîte à idées. Lecture libre ; poster, voter et supprimer la sienne envoient
// player_id + le jeton de profil (intercepteur d'api.js). Favori et suppression
// admin passent par /admin (X-Characters-Access ou jeton d'un admin désigné).
export const ideaService = {
    /** GET /api/ideas → { ideas, daily_limit, remaining_today } */
    async list() {
        return (await api.get('/ideas')).data
    },

    /** POST /api/ideas → idée créée */
    async create(playerId, body) {
        return (await api.post('/ideas', { player_id: playerId, body })).data.idea
    },

    /** PUT / DELETE /api/ideas/{id}/vote → idée à jour */
    async vote(playerId, ideaId, on) {
        const res = on
            ? await api.put(`/ideas/${ideaId}/vote`, { player_id: playerId })
            : await api.delete(`/ideas/${ideaId}/vote`, { data: { player_id: playerId } })
        return res.data.idea
    },

    /** DELETE /api/ideas/{id} — par son auteur */
    async remove(playerId, ideaId) {
        await api.delete(`/ideas/${ideaId}`, { data: { player_id: playerId } })
    },

    /** PATCH /api/admin/ideas/{id} → idée à jour */
    async setPinned(ideaId, isPinned) {
        return (await api.patch(`/admin/ideas/${ideaId}`, { is_pinned: isPinned })).data.idea
    },

    /** DELETE /api/admin/ideas/{id} */
    async adminRemove(ideaId) {
        await api.delete(`/admin/ideas/${ideaId}`)
    },
}

/** Message lisible d'une erreur d'API (validation 422, refus, limite de débit). */
export function ideaError(e, fallback) {
    const status = e?.response?.status
    if (status === 429) return 'Trop de tentatives : patiente une minute.'
    if (status === 404) return 'Cette idée a été supprimée entre-temps.'
    const data = e?.response?.data
    return data?.errors?.body?.[0] ?? data?.message ?? fallback
}
