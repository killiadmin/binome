import Echo from 'laravel-echo'
import Pusher from 'pusher-js'

window.Pusher = Pusher

let echo = null

console.log('[ENV]', {
    key:      import.meta.env.VITE_REVERB_APP_KEY,
    host:     import.meta.env.VITE_REVERB_HOST,
    port:     import.meta.env.VITE_REVERB_PORT,
    backend:  import.meta.env.VITE_BACKEND_URL,
    api:      import.meta.env.VITE_API_URL,
})

function getEcho(playerId = null) {
    if (!echo) {
        echo = new Echo({
            broadcaster: 'reverb',
            key: import.meta.env.VITE_REVERB_APP_KEY,
            wsHost: import.meta.env.VITE_REVERB_HOST,
            wsPort: Number(import.meta.env.VITE_REVERB_PORT),
            forceTLS: import.meta.env.VITE_REVERB_SCHEME === 'https',
            enabledTransports: ['ws', 'wss'],
            disableStats: true,
            authEndpoint: `${import.meta.env.VITE_API_URL}/broadcasting/auth`,
            auth: {
                headers: {
                    'X-Player-Id': String(playerId),
                    'Accept': 'application/json',
                },
            },
        })
    }
    return echo
}

export function useReverb(playerId = null) {
    const echoInstance = getEcho(playerId)

    function joinRoom(roomId, callbacks = {}) {
        const channel = echoInstance.join(`room.${roomId}`)

        channel
            .here((members) => {
                console.log('[Reverb] HERE members:', members)
                callbacks.onHere?.(members)
            })
            .joining((member) => {
                console.log('[Reverb] JOINING:', member)
                callbacks.onJoining?.(member)
            })
            .leaving((member) => {
                console.log('[Reverb] LEAVING:', member)
                callbacks.onLeaving?.(member)
            })
            .error((error) => {
                console.error('[Reverb] ERREUR channel:', error)
                callbacks.onError?.(error)
            })

        if (callbacks.onPlayerJoined) {
            channel.listen('.player.joined', callbacks.onPlayerJoined)
        }
        if (callbacks.onPlayerReady) {
            channel.listen('.player.ready', callbacks.onPlayerReady)
        }

        if (callbacks.onGameStarted) {
            channel.listen('.game.started', callbacks.onGameStarted)
        }

        if (callbacks.onPlayerLeft) {
            channel.listen('.player.left', callbacks.onPlayerLeft)
        }

        return channel
    }

    function leaveRoom(roomId) {
        echoInstance.leave(`room.${roomId}`)
    }

    /**
     * Rejoindre le channel d'une partie
     * Tous les callbacks sont optionnels
     */
    function joinGame(gameId, callbacks = {}) {
        const channel = echoInstance.join(`game.${gameId}`)

        // Presence — qui est connecté dans le salon
        channel
            .here((members) => callbacks.onHere?.(members))
            .joining((member) => callbacks.onJoining?.(member))
            .leaving((member) => callbacks.onLeaving?.(member))
            .error((error) => {
                console.error(`[Reverb] Erreur channel game.${gameId} :`, error)
                callbacks.onError?.(error)
            })

        // Events métier — on vérifie que le callback existe avant d'écouter
        if (callbacks.onGameStarted) {
            channel.listen('.game.started', callbacks.onGameStarted)
        }
        if (callbacks.onRoundStarted) {
            channel.listen('.round.started', callbacks.onRoundStarted)
        }
        if (callbacks.onActionPlayed) {
            channel.listen('.action.played', callbacks.onActionPlayed)
        }
        if (callbacks.onBinomeDiscovered) {
            channel.listen('.binome.discovered', callbacks.onBinomeDiscovered)
        }
        if (callbacks.onGameEnded) {
            channel.listen('.game.ended', callbacks.onGameEnded)
        }

        return channel
    }

    /**
     * Quitter proprement un channel (important pour éviter les fuites mémoire)
     */
    function leaveGame(gameId) {
        echoInstance.leave(`game.${gameId}`)
    }

    /**
     * Vérifier l'état de la connexion WebSocket
     */
    function isConnected() {
        return echoInstance.connector.pusher.connection.state === 'connected'
    }

    return {
        joinRoom,
        leaveRoom,
        joinGame,
        leaveGame,
    }
}

export function resetEcho() {
    if (echo) {
        echo.disconnect()
        echo = null
    }
}
