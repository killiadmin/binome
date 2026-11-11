// Client temps réel du banc, indépendant du protocole.
//
//   const rt = await connect({ protocol: 'pusher', ... , playerId })
//   const members = await rt.subscribe('game.12')   // presence : membres présents
//   await rt.waitFor((e) => e.event === 'round.started' && e.channel === 'game.12')
//   rt.close()
//
// Chaque message reçu est journalisé dans rt.log sous la forme
//   { channel: 'game.12', event: 'round.started', data: {...}, at }
// et les mouvements de présence sous les pseudo-events « presence:joining » /
// « presence:leaving » (data = membre). Seul ce fichier connaît les protocoles :
// le scénario ne change pas quand on passe de Reverb (Pusher) au serveur Rust.

const PRESENCE = /^(room|game)\.\d+$/
const PRIVATE = /^player\.\d+$/

class RealtimeClient {
    constructor({ label, recorder }) {
        this.label = label
        this.recorder = recorder
        this.log = []
        this.waiters = new Set()
        this.closed = false
    }

    push(entry) {
        const e = { ...entry, at: Date.now() }
        this.log.push(e)
        if (!e.event.startsWith('presence:')) {
            const kind = e.channel.split('.')[0]
            this.recorder?.record(`event ${e.event} @${kind}`, e.data)
        }
        for (const w of [...this.waiters]) w.check()
    }

    /**
     * Attend un event (déjà reçu depuis `since`, ou à venir).
     * `since` : index dans le log à partir duquel chercher (défaut : 0).
     */
    waitFor(predicate, { timeout = 8000, since = 0, what = 'event' } = {}) {
        return new Promise((resolve, reject) => {
            const waiter = {
                check: () => {
                    const found = this.log.slice(since).find(predicate)
                    if (found) {
                        clearTimeout(timer)
                        this.waiters.delete(waiter)
                        resolve(found)
                    }
                },
            }
            const timer = setTimeout(() => {
                this.waiters.delete(waiter)
                reject(new Error(`[${this.label}] délai dépassé en attendant : ${what}`))
            }, timeout)
            this.waiters.add(waiter)
            waiter.check()
        })
    }

    /** Events reçus d'un type donné (depuis `since`). */
    received(event, { since = 0, channel = null } = {}) {
        return this.log.slice(since).filter((e) => e.event === event && (!channel || e.channel === channel))
    }

    mark() { return this.log.length }
}

// ─── Protocole Pusher (Laravel Reverb) ───────────────────────────────────────

class PusherClient extends RealtimeClient {
    constructor(opts) {
        super(opts)
        this.opts = opts
        this.pending = new Map() // canal pusher → { resolve, reject }
    }

    static wireName(channel) {
        if (PRESENCE.test(channel)) return `presence-${channel}`
        if (PRIVATE.test(channel)) return `private-${channel}`
        return channel
    }

    static logicalName(wire) {
        return wire.replace(/^(presence|private)-/, '')
    }

    connect() {
        const { wsUrl, appKey } = this.opts
        return new Promise((resolve, reject) => {
            this.ws = new WebSocket(`${wsUrl}/app/${appKey}?protocol=7&client=js&version=8.4.0&flash=false`)
            this.ws.onerror = (e) => reject(new Error(`[${this.label}] WebSocket : ${e.message ?? 'erreur'}`))
            this.ws.onclose = () => { this.closed = true }
            this.ws.onmessage = (msg) => {
                const m = JSON.parse(msg.data)
                const data = typeof m.data === 'string' ? safeJson(m.data) : m.data
                switch (m.event) {
                    case 'pusher:connection_established':
                        this.socketId = data.socket_id
                        resolve(this)
                        break
                    case 'pusher:ping':
                        this.send('pusher:pong', {})
                        break
                    case 'pusher_internal:subscription_succeeded': {
                        const p = this.pending.get(m.channel)
                        this.pending.delete(m.channel)
                        const members = data?.presence ? Object.values(data.presence.hash) : []
                        p?.resolve(members)
                        break
                    }
                    case 'pusher:subscription_error': {
                        const p = this.pending.get(m.channel)
                        this.pending.delete(m.channel)
                        p?.reject(Object.assign(new Error(`[${this.label}] abonnement refusé : ${m.channel}`), { status: data?.status }))
                        break
                    }
                    case 'pusher_internal:member_added':
                        this.push({ channel: PusherClient.logicalName(m.channel), event: 'presence:joining', data: { id: Number(data.user_id), ...data.user_info } })
                        break
                    case 'pusher_internal:member_removed':
                        this.push({ channel: PusherClient.logicalName(m.channel), event: 'presence:leaving', data: { id: Number(data.user_id) } })
                        break
                    case 'pusher:error':
                        this.push({ channel: '-', event: 'protocol:error', data })
                        break
                    default:
                        if (m.channel) this.push({ channel: PusherClient.logicalName(m.channel), event: m.event, data })
                }
            }
        })
    }

    send(event, data) {
        this.ws.send(JSON.stringify({ event, data }))
    }

    /** S'abonne ; renvoie les membres présents (presence) ou []. Rejette si refusé. */
    async subscribe(channel) {
        const wire = PusherClient.wireName(channel)
        const payload = { channel: wire }

        if (wire !== channel) {
            // Canal privé / presence : auth HTTP comme laravel-echo
            const res = await fetch(`${this.opts.authUrl}`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/x-www-form-urlencoded',
                    Accept: 'application/json',
                    'X-Player-Id': String(this.opts.playerId),
                },
                body: new URLSearchParams({ socket_id: this.socketId, channel_name: wire }),
            })
            if (res.status !== 200) {
                throw Object.assign(new Error(`[${this.label}] abonnement refusé : ${channel} (${res.status})`), { status: res.status })
            }
            Object.assign(payload, await res.json())
        }

        return new Promise((resolve, reject) => {
            this.pending.set(wire, { resolve, reject })
            this.send('pusher:subscribe', payload)
            setTimeout(() => {
                if (this.pending.delete(wire)) reject(new Error(`[${this.label}] pas de confirmation d'abonnement : ${channel}`))
            }, 8000)
        })
    }

    unsubscribe(channel) {
        this.send('pusher:unsubscribe', { channel: PusherClient.wireName(channel) })
    }

    close() {
        this.closed = true
        this.ws?.close()
    }
}

// ─── Protocole natif (serveur Rust) ──────────────────────────────────────────
//   client → { op: 'sub' | 'unsub', ch } | { op: 'ping' }
//   serveur → sub_ok { ch, members } | sub_err { ch, status } | joining / leaving { ch, member }
//             | event { ch, event, data } | pong

class NativeClient extends RealtimeClient {
    constructor(opts) {
        super(opts)
        this.opts = opts
        this.pending = new Map()
    }

    connect() {
        const { wsUrl, playerId } = this.opts
        const qs = playerId ? `?player_id=${playerId}` : ''
        return new Promise((resolve, reject) => {
            this.ws = new WebSocket(`${wsUrl}${qs}`)
            this.ws.onopen = () => resolve(this)
            this.ws.onerror = (e) => reject(new Error(`[${this.label}] WebSocket : ${e.message ?? 'erreur'}`))
            this.ws.onclose = () => { this.closed = true }
            this.ws.onmessage = (msg) => {
                const m = JSON.parse(msg.data)
                switch (m.op) {
                    case 'sub_ok':
                        this.pending.get(m.ch)?.resolve(m.members ?? [])
                        this.pending.delete(m.ch)
                        break
                    case 'sub_err':
                        this.pending.get(m.ch)?.reject(Object.assign(new Error(`[${this.label}] abonnement refusé : ${m.ch} (${m.status})`), { status: m.status }))
                        this.pending.delete(m.ch)
                        break
                    case 'joining':
                    case 'leaving':
                        this.push({ channel: m.ch, event: `presence:${m.op}`, data: m.member })
                        break
                    case 'event':
                        this.push({ channel: m.ch, event: m.event, data: m.data })
                        break
                    default:
                        break
                }
            }
        })
    }

    subscribe(channel) {
        return new Promise((resolve, reject) => {
            this.pending.set(channel, { resolve, reject })
            this.ws.send(JSON.stringify({ op: 'sub', ch: channel }))
            setTimeout(() => {
                if (this.pending.delete(channel)) reject(new Error(`[${this.label}] pas de confirmation d'abonnement : ${channel}`))
            }, 8000)
        })
    }

    unsubscribe(channel) {
        this.ws.send(JSON.stringify({ op: 'unsub', ch: channel }))
    }

    close() {
        this.closed = true
        this.ws?.close()
    }
}

function safeJson(text) {
    try { return JSON.parse(text) } catch { return text }
}

/**
 * Ouvre une connexion temps réel.
 * pusher : { wsUrl: 'ws://127.0.0.1:18080', appKey, authUrl: 'http://…/broadcasting/auth' }
 */
export async function connect({ protocol, label, recorder, playerId = null, ...opts }) {
    if (protocol === 'pusher') {
        return new PusherClient({ label, recorder, playerId, ...opts }).connect()
    }
    if (protocol === 'native') {
        return new NativeClient({ label, recorder, playerId, ...opts }).connect()
    }
    throw new Error(`Protocole temps réel inconnu : ${protocol}`)
}
