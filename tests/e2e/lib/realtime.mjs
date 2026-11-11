// Client temps réel du banc, indépendant du protocole.
//
//   const rt = await connect({ protocol: 'native', wsUrl, playerId })
//   const members = await rt.subscribe('game.12')   // presence : membres présents
//   await rt.waitFor((e) => e.event === 'round.started' && e.channel === 'game.12')
//   rt.close()
//
// Chaque message reçu est journalisé dans rt.log sous la forme
//   { channel: 'game.12', event: 'round.started', data: {...}, at }
// et les mouvements de présence sous les pseudo-events « presence:joining » /
// « presence:leaving » (data = membre). Seul ce fichier connaît le protocole
// (docs/rust-migration/04-protocole-ws.md).

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

/** Ouvre une connexion temps réel : { protocol: 'native', wsUrl }. */
export async function connect({ protocol, label, recorder, playerId = null, ...opts }) {
    if (protocol === 'native') {
        return new NativeClient({ label, recorder, playerId, ...opts }).connect()
    }
    throw new Error(`Protocole temps réel inconnu : ${protocol}`)
}
