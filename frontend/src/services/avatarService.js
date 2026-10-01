import { api } from './api'

// Photo de profil d'un joueur (remplace le badge à initiales).
// L'image part en data URI JPEG carrée ; le serveur ne renvoie qu'une URL.
export const avatarService = {
    async upload(playerId, dataUrl) {
        const res = await api.post(`/players/${playerId}/avatar`, {
            player_id: playerId,
            image: dataUrl,
        })
        return res.data.avatar_url
    },

    async remove(playerId) {
        await api.delete(`/players/${playerId}/avatar`, {
            data: { player_id: playerId },
        })
    },
}
