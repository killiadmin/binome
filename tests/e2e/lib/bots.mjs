// Joueurs simulés et aides de partie partagées par les scénarios.

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

export class Bot {
    constructor(ctx, name) {
        this.ctx = ctx
        this.name = name
        this.pseudo = ctx.pseudo(name)
        this.id = null
        this.rt = null
        this.channels = new Set()
        this.offline = false
        this.character = null
    }

    setId(id) {
        this.id = id
        this.ctx.created.players.add(id)
        return this
    }

    async connect() {
        this.rt = await this.ctx.connect(this.pseudo, this.id)
        this.offline = false
        return this
    }

    async sub(channel) {
        const members = await this.rt.subscribe(channel)
        this.channels.add(channel)
        return members
    }

    unsub(channel) {
        this.rt.unsubscribe(channel)
        this.channels.delete(channel)
    }

    /** Coupe la connexion (téléphone verrouillé, Wi-Fi perdu…). */
    disconnect() {
        this.rt.close()
        this.offline = true
    }

    /** Nouvelle connexion + réabonnement à tous les canaux, comme le front. */
    async reconnect() {
        const channels = [...this.channels]
        if (this.rt && !this.rt.closed) this.rt.close()
        await this.connect()
        for (const ch of channels) await this.rt.subscribe(ch)
    }
}

/** Positions courantes des journaux, à prendre AVANT l'action déclenchante. */
export function marks(bots) {
    return new Map(bots.map((b) => [b, b.rt.mark()]))
}

/**
 * Tous les `bots` reçoivent l'event sur leur canal (`channelOf(bot)` ou nom fixe).
 * Renvoie la liste des events reçus (même ordre que `bots`).
 */
export function everyone(bots, channelOf, event, { pred = null, since = null, timeout = 8000 } = {}) {
    const chOf = typeof channelOf === 'function' ? channelOf : () => channelOf
    return Promise.all(bots.map((b) => b.rt.waitFor(
        (e) => e.event === event && e.channel === chOf(b) && (!pred || pred(e.data)),
        { since: since?.get(b) ?? 0, timeout, what: `${event} sur ${chOf(b)}` },
    )))
}

/**
 * Oracle de la partie (réservé au banc) : la vue admin révèle binômes et
 * personnages. playerId → { character, forbidden_words, binomeId, partnerId, isOrphan }.
 */
export async function oracle(ctx, gameId) {
    const { data } = await ctx.api.get('/admin/games/:game', { params: { game: gameId }, admin: true, expect: 200 })
    const byPlayer = new Map()
    for (const b of data.binomes) {
        for (const p of b.players) {
            const partner = b.players.find((o) => o.id !== p.id)
            byPlayer.set(p.id, {
                character: p.character?.name,
                characterId: p.character?.id,
                forbidden_words: p.character?.forbidden_words ?? [],
                binomeId: b.id,
                partnerId: partner?.id ?? null,
                isOrphan: b.is_orphan,
            })
        }
    }
    return byPlayer
}

export async function gameState(ctx, gameId) {
    return (await ctx.api.get('/games/:game', { params: { game: gameId }, expect: 200 })).data
}
