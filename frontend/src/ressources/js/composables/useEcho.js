import { inject, onUnmounted } from 'vue'

export function useEcho() {
    const echo = inject('echo')

    if (!echo) {
        throw new Error('Echo non disponible. Vérifie que app.provide("echo") est bien appelé dans main.js')
    }

    const channels = []

    /**
     * Rejoindre le PresenceChannel d'une partie
     * et écouter tous les events du jeu
     */
    function joinGameChannel(gameId, handlers = {}) {
        const channel = echo.join(`game.${gameId}`)

        channel
            .here((members) => {
                handlers.onHere?.(members)
            })
            .joining((member) => {
                handlers.onJoining?.(member)
            })
            .leaving((member) => {
                handlers.onLeaving?.(member)
            })
            .error((error) => {
                console.error(`[Echo] Erreur channel game.${gameId}:`, error)
                handlers.onError?.(error)
            })

        if (handlers.onGameStarted) {
            channel.listen('.game.started', handlers.onGameStarted)
        }
        if (handlers.onRoundStarted) {
            channel.listen('.round.started', handlers.onRoundStarted)
        }
        if (handlers.onActionPlayed) {
            channel.listen('.action.played', handlers.onActionPlayed)
        }
        if (handlers.onBinomeDiscovered) {
            channel.listen('.binome.discovered', handlers.onBinomeDiscovered)
        }
        if (handlers.onGameEnded) {
            channel.listen('.game.ended', handlers.onGameEnded)
        }

        channels.push(`game.${gameId}`)

        return channel
    }

    /**
     * Quitter proprement un channel
     */
    function leaveGameChannel(gameId) {
        echo.leave(`game.${gameId}`)
        const index = channels.indexOf(`game.${gameId}`)
        if (index > -1) channels.splice(index, 1)
    }

    // Cleanup automatique quand le composant est détruit
    onUnmounted(() => {
        channels.forEach((channelName) => echo.leave(channelName))
    })

    return {
        echo,
        joinGameChannel,
        leaveGameChannel,
    }
}
