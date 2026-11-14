// Temps réel : client du protocole WebSocket natif du serveur Binome
// (docs/rust-migration/04-protocole-ws.md). Remplace laravel-echo + pusher-js.
//
// L'interface publique est restée celle de l'ancienne version (useReverb,
// joinRoom, joinGame, …, resetEcho) : les pages n'ont pas changé. Comportements
// repris de Pusher, dont elles dépendent :
//   - `onHere(members)` est rappelé après chaque reconnexion (liste à jour) ;
//   - `onConnectionChange` reçoit `{ previous, current }` avec les états
//     'connecting' | 'connected' | 'unavailable' | 'disconnected' ;
//   - reconnexion et réabonnement automatiques ; les events émis pendant la
//     coupure sont perdus : la page resynchronise son état sur 'connected'.

const PING_EVERY_MS = 25_000
const PONG_TIMEOUT_MS = 10_000
const UNAVAILABLE_AFTER_MS = 10_000

let client = null

// Abonnés prévenus quand le singleton est détruit : les canaux qui ne sont
// pas réabonnés par la page appelante (le chat du hall) se rebranchent dessus.
const resetListeners = new Set()

// Jeton du profil connecté (même clé localStorage que l'intercepteur d'api.js) :
// le serveur en déduit le joueur ; sans jeton valide, seul le canal `lobby` est ouvert.
function profileToken() {
    try {
        return JSON.parse(localStorage.getItem('profile') ?? 'null')?.token ?? null
    } catch {
        return null
    }
}

function socketUrl(playerId) {
    const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws'
    const token = playerId ? profileToken() : null
    const qs = token ? `?token=${encodeURIComponent(token)}` : ''
    return `${scheme}://${window.location.host}${import.meta.env.BASE_URL}ws${qs}`
}

class Channel {
    constructor(name) {
        this.name = name
        this.listeners = new Map() // event → Set(fn)
        this.hooks = { here: new Set(), joining: new Set(), leaving: new Set(), error: new Set() }
    }

    listen(event, fn) {
        const name = event.replace(/^\./, '') // `.round.started` (syntaxe Echo) → `round.started`
        if (!this.listeners.has(name)) this.listeners.set(name, new Set())
        this.listeners.get(name).add(fn)
        return this
    }

    here(fn) { this.hooks.here.add(fn); return this }
    joining(fn) { this.hooks.joining.add(fn); return this }
    leaving(fn) { this.hooks.leaving.add(fn); return this }
    error(fn) { this.hooks.error.add(fn); return this }

    emit(hook, payload) {
        for (const fn of this.hooks[hook]) fn(payload)
    }

    dispatch(event, data) {
        for (const fn of this.listeners.get(event) ?? []) fn(data)
    }
}

class RealtimeClient {
    constructor(playerId) {
        this.playerId = playerId
        this.channels = new Map()
        this.stateListeners = new Set()
        this.state = 'initialized'
        this.closedByUs = false
        this.attempt = 0
        this.onVisible = () => {
            // Téléphone rallumé : la socket peut se croire ouverte alors qu'elle est morte
            if (document.visibilityState === 'visible') this.ping()
        }
        this.onOnline = () => this.reconnectNow()
        document.addEventListener('visibilitychange', this.onVisible)
        window.addEventListener('online', this.onOnline)
        this.open()
    }

    setState(current) {
        if (current === this.state) return
        const previous = this.state
        this.state = current
        for (const fn of this.stateListeners) fn({ previous, current })
    }

    open() {
        this.setState('connecting')
        clearTimeout(this.unavailableTimer)
        this.unavailableTimer = setTimeout(() => {
            if (this.state === 'connecting') this.setState('unavailable')
        }, UNAVAILABLE_AFTER_MS)

        const ws = new WebSocket(socketUrl(this.playerId))
        this.ws = ws
        ws.onopen = () => {
            this.attempt = 0
            clearTimeout(this.unavailableTimer)
            for (const name of this.channels.keys()) this.send({ op: 'sub', ch: name })
            this.setState('connected')
            this.startHeartbeat()
        }
        ws.onmessage = (msg) => this.receive(msg.data)
        ws.onclose = () => {
            if (this.ws !== ws) return
            this.stopHeartbeat()
            if (this.closedByUs) {
                this.setState('disconnected')
                return
            }
            this.scheduleReconnect()
        }
        // onerror est toujours suivi de onclose : la reconnexion se fait là
        ws.onerror = () => {}
    }

    scheduleReconnect() {
        if (this.state !== 'unavailable') this.setState('connecting')
        const delay = Math.min(10_000, 1000 * 2 ** this.attempt)
        this.attempt += 1
        clearTimeout(this.retryTimer)
        this.retryTimer = setTimeout(() => this.open(), delay)
    }

    reconnectNow() {
        if (this.closedByUs || this.state === 'connected') return
        clearTimeout(this.retryTimer)
        this.attempt = 0
        this.open()
    }

    startHeartbeat() {
        this.stopHeartbeat()
        this.pingTimer = setInterval(() => this.ping(), PING_EVERY_MS)
    }

    stopHeartbeat() {
        clearInterval(this.pingTimer)
        clearTimeout(this.pongTimer)
    }

    /** Ping applicatif : sans pong à temps, la liaison est considérée morte. */
    ping() {
        if (this.ws?.readyState !== WebSocket.OPEN) return
        this.send({ op: 'ping' })
        clearTimeout(this.pongTimer)
        this.pongTimer = setTimeout(() => this.ws?.close(), PONG_TIMEOUT_MS)
    }

    send(message) {
        if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(message))
    }

    receive(raw) {
        let m
        try { m = JSON.parse(raw) } catch { return }
        if (m.op === 'pong') {
            clearTimeout(this.pongTimer)
            return
        }
        const channel = this.channels.get(m.ch)
        if (!channel) return
        switch (m.op) {
            case 'sub_ok': channel.emit('here', m.members ?? []); break
            case 'sub_err': channel.emit('error', { type: 'AuthError', status: m.status }); break
            case 'joining': channel.emit('joining', m.member); break
            case 'leaving': channel.emit('leaving', m.member); break
            case 'event': channel.dispatch(m.event, m.data); break
            default: break
        }
    }

    subscribe(name) {
        let channel = this.channels.get(name)
        if (!channel) {
            channel = new Channel(name)
            this.channels.set(name, channel)
            this.send({ op: 'sub', ch: name }) // sinon envoyé à l'ouverture
        }
        return channel
    }

    leave(name) {
        if (this.channels.delete(name)) this.send({ op: 'unsub', ch: name })
    }

    onStateChange(fn) {
        this.stateListeners.add(fn)
        return () => this.stateListeners.delete(fn)
    }

    disconnect() {
        this.closedByUs = true
        clearTimeout(this.retryTimer)
        clearTimeout(this.unavailableTimer)
        this.stopHeartbeat()
        document.removeEventListener('visibilitychange', this.onVisible)
        window.removeEventListener('online', this.onOnline)
        this.ws?.close()
        this.channels.clear()
    }
}

function getClient(playerId = null) {
    if (!client) client = new RealtimeClient(playerId)
    return client
}

export function useReverb(playerId = null) {
    const rt = getClient(playerId)

    function joinRoom(roomId, callbacks = {}) {
        const channel = rt.subscribe(`room.${roomId}`)

        channel
            .here((members) => callbacks.onHere?.(members))
            .joining((member) => callbacks.onJoining?.(member))
            .leaving((member) => callbacks.onLeaving?.(member))
            .error((error) => {
                console.error('[temps réel] ERREUR canal :', error)
                callbacks.onError?.(error)
            })

        if (callbacks.onPlayerJoined) channel.listen('.player.joined', callbacks.onPlayerJoined)
        if (callbacks.onPlayerReady) channel.listen('.player.ready', callbacks.onPlayerReady)
        if (callbacks.onRoomSettingsUpdated) channel.listen('.room.settings.updated', callbacks.onRoomSettingsUpdated)
        if (callbacks.onGameStarted) channel.listen('.game.started', callbacks.onGameStarted)
        if (callbacks.onPlayerLeft) channel.listen('.player.left', callbacks.onPlayerLeft)
        if (callbacks.onPlayerAvatarUpdated) channel.listen('.player.avatar.updated', callbacks.onPlayerAvatarUpdated)

        return channel
    }

    function leaveRoom(roomId) {
        rt.leave(`room.${roomId}`)
    }

    /**
     * Chat commun du hall — canal PUBLIC : lisible par un visiteur qui n'a ni
     * créé ni rejoint de salon.
     */
    function joinLobbyChat(callbacks = {}) {
        const channel = rt.subscribe('lobby')

        channel.listen('.chat.message', (data) => callbacks.onChatMessage?.(data))
        channel.error((error) => {
            console.error('[temps réel] Erreur canal lobby :', error)
            callbacks.onError?.(error)
        })

        return channel
    }

    function leaveLobbyChat() {
        rt.leave('lobby')
    }

    /**
     * Rejoindre le canal d'une partie. Tous les callbacks sont optionnels.
     */
    function joinGame(gameId, callbacks = {}) {
        const channel = rt.subscribe(`game.${gameId}`)

        // Présence : qui est connecté à la partie
        channel
            .here((members) => callbacks.onHere?.(members))
            .joining((member) => callbacks.onJoining?.(member))
            .leaving((member) => callbacks.onLeaving?.(member))
            .error((error) => {
                console.error(`[temps réel] Erreur canal game.${gameId} :`, error)
                callbacks.onError?.(error)
            })

        const events = {
            onGameStarted: '.game.started',
            onCharacterVoteCast: '.character.vote.cast',
            onCharactersRedrawn: '.characters.redrawn',
            onRoundStarted: '.round.started',
            onActionPlayed: '.action.played',
            onBinomeDiscovered: '.binome.discovered',
            onGameEnded: '.game.ended',
            onAnswerGiven: '.answer.given',
            onAnswerCorrected: '.answer.corrected',
            onCardDrawn: '.card.drawn',
            onCardDrawFinished: '.card.draw.finished',
            onAccusationConfirmed: '.accusation.confirmed',
            onPlayerEliminated: '.player.eliminated',
            onReactionSent: '.reaction.sent',
            onTurnSkipped: '.turn.skipped',
            onPlayerExcluded: '.player.excluded',
            onQuestionVoted: '.question.voted',
            onPlayerAvatarUpdated: '.player.avatar.updated',
        }
        for (const [callback, event] of Object.entries(events)) {
            if (callbacks[callback]) channel.listen(event, callbacks[callback])
        }

        return channel
    }

    /**
     * Quitter proprement un canal (important pour éviter les fuites mémoire)
     */
    function leaveGame(gameId) {
        rt.leave(`game.${gameId}`)
    }

    /**
     * Canal privé du joueur (player.{id}) : messages qui ne doivent pas
     * transiter par game.{id}, où d'autres les liraient — ex. un mot interdit
     * montré à tous sauf à son propriétaire.
     */
    function joinPlayerChannel(id, callbacks = {}) {
        const channel = rt.subscribe(`player.${id}`)

        channel.error((error) => {
            console.error(`[temps réel] Erreur canal player.${id} :`, error)
            callbacks.onError?.(error)
        })

        if (callbacks.onCardWordShared) channel.listen('.card.word.shared', callbacks.onCardWordShared)

        return channel
    }

    function leavePlayerChannel(id) {
        rt.leave(`player.${id}`)
    }

    /**
     * Suivre l'état de la connexion (téléphone verrouillé, Wi-Fi qui saute…).
     * Reconnexion et réabonnement sont automatiques, mais les events émis
     * pendant la coupure sont perdus : l'appelant resynchronise son état quand
     * `current` repasse à 'connected'. Renvoie la fonction de désabonnement.
     */
    function onConnectionChange(callback) {
        return rt.onStateChange(callback)
    }

    return {
        joinRoom,
        leaveRoom,
        joinLobbyChat,
        leaveLobbyChat,
        joinGame,
        leaveGame,
        joinPlayerChannel,
        leavePlayerChannel,
        onConnectionChange,
    }
}

/** Le singleton existe-t-il ? Évite de rouvrir une connexion juste pour la fermer. */
export function hasEcho() {
    return client !== null
}

/**
 * S'abonner à la destruction du singleton.
 * Renvoie la fonction de désabonnement.
 */
export function onEchoReset(callback) {
    resetListeners.add(callback)
    return () => resetListeners.delete(callback)
}

/** Ferme la connexion ; la suivante sera ouverte avec le playerId courant. */
export function resetEcho() {
    if (client) {
        client.disconnect()
        client = null
    }
    resetListeners.forEach((cb) => cb())
}
