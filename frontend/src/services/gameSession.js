// Session de jeu de ce navigateur (clé localStorage `session`, lue par
// RoomPage.vue et RoundPage.vue) : { roomId, gameCode, playerId, hostId, isHost, gameId }.

export function readGameSession() {
    try {
        return JSON.parse(localStorage.getItem('session') ?? 'null')
    } catch {
        return null
    }
}

/**
 * Reprend, sur ce navigateur, le salon / la partie d'un profil (champ `current`
 * renvoyé par la connexion et la page profil). Renvoie la route où aller, ou null.
 */
export function resumeFromProfile(playerId, current) {
    if (!current?.room_id) return null
    localStorage.setItem('session', JSON.stringify({
        roomId: current.room_id,
        gameCode: current.room_code,
        playerId,
        hostId: current.host_id,
        isHost: current.host_id === playerId,
        gameId: current.game_id ?? null,
    }))
    return current.game_id
        ? { name: 'RoundPage', params: { gameId: current.game_id } }
        : { name: 'Room' }
}

/** Session d'un autre joueur sur ce navigateur : on l'oublie avant d'en ouvrir une. */
export function dropForeignSession(playerId) {
    const s = readGameSession()
    if (s && s.playerId !== playerId) localStorage.removeItem('session')
}
