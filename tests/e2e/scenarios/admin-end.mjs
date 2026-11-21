// Fin de partie décidée par l'administrateur (migration 11, onglet Parties) :
// POST /admin/games/{game}/end clôt une partie en cours comme une fin normale
// (stats enregistrées, sans gagnant, game.ended) puis vide et ferme le salon.

import { Bot, everyone, marks } from '../lib/bots.mjs'

export async function adminEnd(ctx) {
    const { api } = ctx
    const bots = Array.from({ length: 4 }, (_, i) => new Bot(ctx, `e${i + 1}`))
    const [host, ...guests] = bots
    const route = '/admin/games/:game/end'

    // ── Salon de 4 joueurs, partie lancée, personnages acceptés ─────────────
    const created = await api.post('/rooms', { body: { pseudo: host.pseudo }, expect: 201 })
    host.setId(created.data.player.id)
    const room = created.data.room
    const roomCh = `room.${room.id}`
    await host.connect()
    await host.sub(roomCh)
    for (const g of guests) {
        g.setId((await api.post('/rooms/join', { body: { pseudo: g.pseudo, code: room.code }, expect: 200 })).data.player.id)
        await g.connect()
        await g.sub(roomCh)
    }
    for (const b of bots) await api.patch('/rooms/:room/ready', { params: { room: room.id }, body: { player_id: b.id }, expect: 200 })
    const gameId = (await api.post('/rooms/:room/start', { params: { room: room.id }, body: { player_id: host.id }, expect: 201 })).data.game_id
    ctx.created.games.add(gameId)
    const gameCh = `game.${gameId}`
    for (const b of bots) await b.sub(gameCh)

    let m = marks(bots)
    for (const b of bots) {
        const character = (await api.get('/games/:game/me', { params: { game: gameId }, query: { player_id: b.id }, expect: 200 })).data.character
        await api.post('/games/:game/character-vote', { params: { game: gameId }, body: { player_id: b.id, character_id: character.id, accepted: true }, expect: 200 })
    }
    await everyone(bots, gameCh, 'round.started', { since: m })

    // ── Refus ────────────────────────────────────────────────────────────────
    await ctx.refused(403, 'POST', route, { params: { game: gameId } }, 'fin de partie sans accès admin')
    await ctx.refused(403, 'POST', route, { params: { game: gameId }, headers: { 'X-Profile-Token': api.tokens.get(host.id) } }, 'fin de partie avec le jeton d\'un joueur non admin')
    await ctx.refused(404, 'POST', route, { params: { game: 999999999 }, admin: true }, 'fin d\'une partie inexistante')

    // ── Fin de partie ────────────────────────────────────────────────────────
    const everybody = bots
    m = marks(everybody)
    const res = await api.post(route, { params: { game: gameId }, admin: true, expect: 200 })
    ctx.check(res.data.game_id === gameId && res.data.removed_players === 4, 'fin admin : les 4 membres retirés du salon', res.data)

    const ended = await everyone(bots, gameCh, 'game.ended', { since: m })
    const ev = ended[0].data
    ctx.check(ev.ended_by_admin === true && ev.winners.length === 0, 'game.ended : ended_by_admin, aucun gagnant', ev)
    ctx.check(ev.stats.length === 4 && ev.stats.every((s) => !s.is_winner && !s.survived_full_game), 'game.ended : stats des 4 joueurs, sans victoire')
    ctx.check(ev.all_binomes.flatMap((b) => b.players).every((p) => typeof p.character === 'string'), 'game.ended : personnages révélés')

    const left = await everyone(everybody, roomCh, 'player.left', { since: m })
    ctx.check(left.every((e) => e.data.players.length === 0 && e.data.new_host_id === null), 'player.left : salon vidé (tous les membres)')

    // ── État après coup ──────────────────────────────────────────────────────
    await ctx.refused(404, 'GET', '/rooms/:room', { params: { room: room.id } }, 'salon fermé après la fin admin')
    const state = (await api.get('/games/:game', { params: { game: gameId }, expect: 200 })).data
    ctx.check(state.status === 'finished' && state.current_round === null, 'GET /games : partie terminée, plus de round en cours')
    const recap = (await api.get('/games/:game/recap', { params: { game: gameId }, expect: 200 })).data
    ctx.check(recap.ended_by_admin === true && recap.winners.length === 0 && recap.stats.length === 4, 'récap : ended_by_admin, stats enregistrées, aucun gagnant')
    const listed = (await api.get('/admin/games', { admin: true, expect: 200 })).data.games.find((g) => g.id === gameId)
    ctx.check(listed?.status === 'finished' && listed.ended_by_admin === true && listed.room_id === null && listed.room_code === room.code,
        'admin parties : terminée, arrêtée par l\'admin, salon détaché (code gardé)', listed)

    // Les joueurs sont libres : ils peuvent recréer un salon
    const again = await api.post('/rooms', { body: { pseudo: host.pseudo }, expect: 201 })
    ctx.check(again.data.player.id === host.id, 'après la fin admin : l\'hôte crée un nouveau salon avec son profil')
    await api.delete('/rooms/:room/leave', { params: { room: again.data.room.id }, body: { player_id: host.id }, expect: 200 })

    await ctx.refused(409, 'POST', route, { params: { game: gameId }, admin: true }, 'terminer une partie déjà terminée')
    for (const b of everybody) b.rt.close()
}
