// Modération du salon par l'hôte (2026-11-17) :
//   PATCH /rooms/{room}/lock                      verrouille / déverrouille l'entrée
//   POST  /rooms/{room}/players/{player}/kick     exclut un joueur du salon
// L'exclu reçoit `player.kicked` (salon) et `room.kicked` (canal privé), puis
// le serveur le désabonne du salon (et de la partie en cours s'il la regardait).

import { Bot, everyone, marks, sleep } from '../lib/bots.mjs'

export async function roomModeration(ctx) {
    const { api } = ctx
    const lockRoute = '/rooms/:room/lock'
    const kickRoute = '/rooms/:room/players/:player/kick'

    // ── Salon : hôte + 3 invités, tous abonnés au salon ─────────────────────
    const host = new Bot(ctx, 'm0')
    const created = await api.post('/rooms', { body: { pseudo: host.pseudo }, expect: 201 })
    host.setId(created.data.player.id)
    const room = created.data.room
    const roomCh = `room.${room.id}`
    await host.connect()
    await host.sub(roomCh)

    const join = async (bot, expect = 200) => {
        const res = await api.post('/rooms/join', { body: { pseudo: bot.pseudo, code: room.code }, expect })
        if (expect === 200 && !bot.id) bot.setId(res.data.player.id)
        return res
    }
    const guests = [new Bot(ctx, 'm1'), new Bot(ctx, 'm2'), new Bot(ctx, 'm3')]
    for (const g of guests) {
        await join(g)
        await g.connect()
        await g.sub(roomCh)
    }
    const [a, b, c] = guests
    await b.sub(`player.${b.id}`)
    const p = { room: room.id }

    // ── Refus ────────────────────────────────────────────────────────────────
    await ctx.refused(403, 'PATCH', lockRoute, { params: p, body: { player_id: a.id, is_locked: true } }, 'verrou par un invité')
    await ctx.refused(422, 'PATCH', lockRoute, { params: p, body: { player_id: host.id } }, 'verrou sans is_locked')
    await ctx.refused(401, 'PATCH', lockRoute, { params: p, body: { player_id: host.id, is_locked: true }, auth: false }, 'verrou sans jeton')
    await ctx.refused(403, 'PATCH', lockRoute, { params: p, body: { player_id: host.id, is_locked: true }, headers: { 'X-Profile-Token': api.tokens.get(a.id) } }, 'verrou au nom de l\'hôte avec le jeton d\'un invité')
    await ctx.refused(404, 'PATCH', lockRoute, { params: { room: 999999999 }, body: { player_id: host.id, is_locked: true } }, 'verrou d\'un salon inexistant')
    await ctx.refused(403, 'POST', kickRoute, { params: { ...p, player: b.id }, body: { player_id: a.id } }, 'exclusion par un invité')
    await ctx.refused(409, 'POST', kickRoute, { params: { ...p, player: host.id }, body: { player_id: host.id } }, 'l\'hôte s\'exclut lui-même')
    await ctx.refused(404, 'POST', kickRoute, { params: { ...p, player: 999999999 }, body: { player_id: host.id } }, 'exclusion d\'un joueur inexistant')

    // ── Verrou ───────────────────────────────────────────────────────────────
    const everybody = [host, ...guests]
    let m = marks(everybody)
    const locked = await api.patch(lockRoute, { params: p, body: { player_id: host.id, is_locked: true }, expect: 200 })
    ctx.check(locked.data.is_locked === true, 'verrou : is_locked true dans la réponse', locked.data)
    const lockEvents = await everyone(everybody, roomCh, 'room.lock.updated', { since: m })
    ctx.check(lockEvents.every((e) => e.data.is_locked === true), 'room.lock.updated reçu par tout le salon')
    ctx.check((await api.get('/rooms/:room', { params: p, expect: 200 })).data.room.is_locked === true, 'GET /rooms : is_locked true')

    const late = new Bot(ctx, 'm4')
    const refusedJoin = await ctx.refused(403, 'POST', '/rooms/join', { body: { pseudo: late.pseudo, code: room.code } }, 'arrivée dans un salon verrouillé')
    ctx.check(/verrouillé/.test(refusedJoin.data?.message ?? ''), 'refus : le message parle du verrou', refusedJoin.data)
    // Re-verrouiller ne change rien (idempotent)
    await api.patch(lockRoute, { params: p, body: { player_id: host.id, is_locked: true }, expect: 200 })

    await api.patch(lockRoute, { params: p, body: { player_id: host.id, is_locked: false }, expect: 200 })
    ctx.check((await api.get('/rooms/:room', { params: p, expect: 200 })).data.room.is_locked === false, 'déverrouillé : is_locked false')

    // ── Exclusion d'un invité ────────────────────────────────────────────────
    m = marks(everybody)
    const kicked = await api.post(kickRoute, { params: { ...p, player: b.id }, body: { player_id: host.id }, expect: 200 })
    ctx.check(kicked.data.players.length === 3 && !kicked.data.players.some((x) => x.id === b.id), 'exclusion : liste sans l\'exclu', kicked.data)
    const kickEvents = await everyone(everybody, roomCh, 'player.kicked', { since: m })
    ctx.check(kickEvents.every((e) => e.data.player.id === b.id && e.data.kicked_by.id === host.id && e.data.players.length === 3),
        'player.kicked reçu par tout le salon, exclu compris', kickEvents[0].data)
    const priv = await b.rt.waitFor((e) => e.event === 'room.kicked' && e.channel === `player.${b.id}`, { since: m.get(b), what: 'room.kicked' })
    ctx.check(priv.data.room_id === room.id && priv.data.kicked_by.id === host.id, 'room.kicked sur le canal privé de l\'exclu', priv.data)
    await host.rt.waitFor((e) => e.event === 'presence:leaving' && e.channel === roomCh && e.data.id === b.id, { since: m.get(host), what: 'leaving de l\'exclu' })
    ctx.check(true, 'présence : l\'exclu quitte le salon (désabonné par le serveur)')

    // Plus rien du salon pour l'exclu, et il ne peut pas s'y réabonner
    m = marks(everybody)
    await api.patch('/rooms/:room/ready', { params: p, body: { player_id: a.id }, expect: 200 })
    await everyone([host, a, c], roomCh, 'player.ready', { since: m })
    await sleep(200)
    ctx.check(b.rt.received('player.ready', { since: m.get(b) }).length === 0, 'l\'exclu ne reçoit plus les events du salon')
    let resub = null
    try { await b.rt.subscribe(roomCh) } catch (e) { resub = e.status }
    ctx.check(resub === 403, 'l\'exclu ne peut plus s\'abonner au salon → 403', resub)
    b.channels.delete(roomCh)
    await ctx.refused(409, 'POST', kickRoute, { params: { ...p, player: b.id }, body: { player_id: host.id } }, 'exclure un joueur qui n\'est plus dans le salon')
    await ctx.refused(403, 'PATCH', '/rooms/:room/ready', { params: p, body: { player_id: b.id } }, 'l\'exclu ne peut plus agir dans le salon')

    // Salon ouvert : il peut revenir ; verrouillé : non
    await join(b)
    await b.sub(roomCh)
    await api.post(kickRoute, { params: { ...p, player: b.id }, body: { player_id: host.id }, expect: 200 })
    await api.patch(lockRoute, { params: p, body: { player_id: host.id, is_locked: true }, expect: 200 })
    await ctx.refused(403, 'POST', '/rooms/join', { body: { pseudo: b.pseudo, code: room.code } }, 'l\'exclu ne revient pas dans un salon verrouillé')

    // ── Partie en cours : joueur assis protégé, spectateur exclu ─────────────
    await api.patch(lockRoute, { params: p, body: { player_id: host.id, is_locked: false }, expect: 200 })
    await join(late)
    await late.connect()
    await late.sub(roomCh)
    const players4 = [host, a, c, late]
    for (const x of players4) {
        const r = (await api.get('/rooms/:room', { params: p, expect: 200 })).data.room.players.find((y) => y.id === x.id)
        if (!r.is_ready) await api.patch('/rooms/:room/ready', { params: p, body: { player_id: x.id }, expect: 200 })
    }
    const gameId = (await api.post('/rooms/:room/start', { params: p, body: { player_id: host.id }, expect: 201 })).data.game_id
    ctx.created.games.add(gameId)
    const gameCh = `game.${gameId}`

    const spectator = new Bot(ctx, 'mspec')
    await join(spectator)
    await spectator.connect()
    await spectator.sub(roomCh)
    await spectator.sub(gameCh)
    await spectator.sub(`player.${spectator.id}`)
    await host.sub(gameCh)

    await ctx.refused(409, 'POST', kickRoute, { params: { ...p, player: a.id }, body: { player_id: host.id } }, 'exclure du salon un joueur de la partie en cours')
    m = marks([host, spectator])
    await api.post(kickRoute, { params: { ...p, player: spectator.id }, body: { player_id: host.id }, expect: 200 })
    await spectator.rt.waitFor((e) => e.event === 'room.kicked', { since: m.get(spectator), what: 'room.kicked (spectateur)' })
    await host.rt.waitFor((e) => e.event === 'presence:leaving' && e.channel === gameCh && e.data.id === spectator.id, { since: m.get(host), what: 'leaving du spectateur (partie)' })
    ctx.check(true, 'spectateur exclu : désabonné de la partie aussi')
    let resubGame = null
    try { await spectator.rt.subscribe(gameCh) } catch (e) { resubGame = e.status }
    ctx.check(resubGame === 403, 'le spectateur exclu ne peut plus regarder la partie → 403', resubGame)

    // ── Nettoyage : fin admin (vide et ferme le salon) ───────────────────────
    await api.post('/admin/games/:game/end', { params: { game: gameId }, admin: true, expect: 200 })
    for (const x of [...everybody, late, spectator]) x.rt?.close()
}
