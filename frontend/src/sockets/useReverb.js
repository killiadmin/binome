import Echo from 'laravel-echo'
import Pusher from 'pusher-js'

window.Pusher = Pusher

let echo = null

// Abonnés prévenus quand le singleton est détruit : les channels qui ne sont
// pas re-souscrits par la page appelante (le chat du hall) doivent se rebrancher
// sur la nouvelle instance.
const resetListeners = new Set()

// The WebSocket connects back to whatever origin served the page; the Vite dev
// server proxies /app to the Reverb container. Nothing here depends on the
// host's LAN IP, so switching networks needs no config change.
const wsPort = Number(window.location.port) || (window.location.protocol === 'https:' ? 443 : 80)

console.log('[ENV]', {
    key:  import.meta.env.VITE_REVERB_APP_KEY,
    host: window.location.hostname,
    port: wsPort,
})

function getEcho(playerId = null) {
    if (!echo) {
        echo = new Echo({
            broadcaster: 'reverb',
            key: import.meta.env.VITE_REVERB_APP_KEY,
            wsHost: window.location.hostname,
            wsPort,
            wssPort: wsPort,
            forceTLS: window.location.protocol === 'https:',
            enabledTransports: ['ws', 'wss'],
            disableStats: true,
            authEndpoint: '/broadcasting/auth',
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

        if (callbacks.onRoomSettingsUpdated) {
            channel.listen('.room.settings.updated', callbacks.onRoomSettingsUpdated)
        }

        if (callbacks.onGameStarted) {
            channel.listen('.game.started', callbacks.onGameStarted)
        }

        if (callbacks.onPlayerLeft) {
            channel.listen('.player.left', callbacks.onPlayerLeft)
        }

        if (callbacks.onPlayerAvatarUpdated) {
            channel.listen('.player.avatar.updated', callbacks.onPlayerAvatarUpdated)
        }

        return channel
    }

    function leaveRoom(roomId) {
        echoInstance.leave(`room.${roomId}`)
    }

    /**
     * Chat commun du hall — channel PUBLIC (aucune auth) : il doit rester
     * accessible aux visiteurs qui n'ont ni créé ni rejoint de salon, et qui
     * n'ont donc pas de Player à présenter au BroadcastAuthController.
     */
    function joinLobbyChat(callbacks = {}) {
        const channel = echoInstance.channel('lobby')

        channel.listen('.chat.message', (data) => callbacks.onChatMessage?.(data))
        channel.error?.((error) => {
            console.error('[Reverb] Erreur channel lobby :', error)
            callbacks.onError?.(error)
        })

        return channel
    }

    function leaveLobbyChat() {
        echoInstance.leave('lobby')
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

        if (callbacks.onGameStarted) {
            channel.listen('.game.started', (data) => {
                console.log('[WS] game.started reçu', data)
                callbacks.onGameStarted(data)
            })
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

        if (callbacks.onAnswerGiven) {
            channel.listen('.answer.given', callbacks.onAnswerGiven)
        }

        if (callbacks.onAccusationConfirmed) {
            channel.listen('.accusation.confirmed', callbacks.onAccusationConfirmed)
        }

        if (callbacks.onPlayerEliminated) {
            channel.listen('.player.eliminated', callbacks.onPlayerEliminated)
        }

        if (callbacks.onReactionSent) {
            channel.listen('.reaction.sent', callbacks.onReactionSent)
        }

        if (callbacks.onTurnSkipped) {
            channel.listen('.turn.skipped', callbacks.onTurnSkipped)
        }

        if (callbacks.onPlayerExcluded) {
            channel.listen('.player.excluded', callbacks.onPlayerExcluded)
        }

        if (callbacks.onPlayerAvatarUpdated) {
            channel.listen('.player.avatar.updated', callbacks.onPlayerAvatarUpdated)
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
     * Suivre l'état de la connexion WebSocket (téléphone verrouillé, Wi-Fi qui
     * saute…). Pusher se reconnecte et se réabonne tout seul, mais les events
     * émis pendant la coupure sont perdus : l'appelant doit resynchroniser son
     * état quand `current` repasse à 'connected'.
     * Renvoie la fonction de désabonnement.
     */
    function onConnectionChange(callback) {
        const connection = echoInstance.connector.pusher.connection
        const handler = ({previous, current}) => callback({previous, current})
        connection.bind('state_change', handler)
        return () => connection.unbind('state_change', handler)
    }

    return {
        joinRoom,
        leaveRoom,
        joinLobbyChat,
        leaveLobbyChat,
        joinGame,
        leaveGame,
        onConnectionChange,
    }
}

/** Le singleton existe-t-il ? Évite de rouvrir une connexion juste pour la fermer. */
export function hasEcho() {
    return echo !== null
}

/**
 * S'abonner à la destruction du singleton Echo.
 * Renvoie la fonction de désabonnement.
 */
export function onEchoReset(callback) {
    resetListeners.add(callback)
    return () => resetListeners.delete(callback)
}

export function resetEcho() {
    if (echo) {
        echo.disconnect()
        echo = null
    }
    resetListeners.forEach((cb) => cb())
}
