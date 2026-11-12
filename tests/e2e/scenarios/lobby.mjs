// Hors partie : transfert d'hôte, lancement à moins de 4, chat du hall, emojis joueurs.

import { Bot, everyone, marks } from '../lib/bots.mjs'
import { jpegDataUri } from '../lib/http.mjs'

export async function lobby(ctx) {
    const { api } = ctx

    // ── Transfert d'hôte, salon supprimé quand il se vide ───────────────────
    const h = new Bot(ctx, 'lh')
    const g = new Bot(ctx, 'lg')
    const c = await api.post('/rooms', { body: { pseudo: h.pseudo }, expect: 201 })
    h.setId(c.data.player.id)
    const room = c.data.room
    const j = await api.post('/rooms/join', { body: { pseudo: g.pseudo, code: room.code.toLowerCase() === room.code ? room.code : room.code }, expect: 200 })
    g.setId(j.data.player.id)
    await g.connect()
    await g.sub(`room.${room.id}`)
    const mk = marks([g])
    await api.delete('/rooms/:room/leave', { params: { room: room.id }, body: { player_id: h.id }, expect: 200 })
    const [left] = await everyone([g], `room.${room.id}`, 'player.left', { since: mk })
    ctx.check(left.data.new_host_id === g.id, 'départ de l\'hôte : l\'hôte passe au joueur suivant')
    const view = (await api.get('/rooms/:room', { params: { room: room.id }, expect: 200 })).data.room
    ctx.check(view.created_by === g.id && view.players.length === 1, 'GET /rooms : created_by mis à jour')
    // L'ancien hôte garde son profil (plus supprimé au départ) mais n'est plus dans le salon
    await ctx.refused(403, 'PATCH', '/rooms/:room/ready', { params: { room: room.id }, body: { player_id: h.id } }, 'ready d\'un joueur parti du salon')
    const last = await api.delete('/rooms/:room/leave', { params: { room: room.id }, body: { player_id: g.id }, expect: 200 })
    ctx.check(/supprimé/.test(last.data.message), 'dernier départ : salon supprimé')
    g.rt.close()

    // ── Lancement refusé à moins de 4 joueurs ────────────────────────────────
    const trio = ['t1', 't2', 't3'].map((n) => new Bot(ctx, n))
    const tc = await api.post('/rooms', { body: { pseudo: trio[0].pseudo }, expect: 201 })
    trio[0].setId(tc.data.player.id)
    for (const b of trio.slice(1)) b.setId((await api.post('/rooms/join', { body: { pseudo: b.pseudo, code: tc.data.room.code }, expect: 200 })).data.player.id)
    for (const b of trio) await api.patch('/rooms/:room/ready', { params: { room: tc.data.room.id }, body: { player_id: b.id }, expect: 200 })
    await ctx.refused(422, 'POST', '/rooms/:room/start', { params: { room: tc.data.room.id }, body: { player_id: trio[0].id } }, 'lancer à 3 joueurs')

    // Un 4e joueur : la partie démarre, puis l'admin la supprime en cours de route
    const fourth = new Bot(ctx, 't4')
    fourth.setId((await api.post('/rooms/join', { body: { pseudo: fourth.pseudo, code: tc.data.room.code }, expect: 200 })).data.player.id)
    await api.patch('/rooms/:room/ready', { params: { room: tc.data.room.id }, body: { player_id: fourth.id }, expect: 200 })
    const started = await api.post('/rooms/:room/start', { params: { room: tc.data.room.id }, body: { player_id: trio[0].id }, expect: 201 })
    await ctx.refused(403, 'DELETE', '/admin/games/:game', { params: { game: started.data.game_id }, admin: true }, 'supprimer une partie sans le mot de passe de suppression')
    await ctx.refused(403, 'DELETE', '/admin/games/:game', { params: { game: started.data.game_id }, admin: true, headers: { 'X-Delete-Password': 'mauvais mot de passe' } }, 'supprimer une partie avec un mauvais mot de passe')
    const del = await api.delete('/admin/games/:game', { params: { game: started.data.game_id }, admin: true, deletion: true, expect: 200 })
    ctx.check(/supprimée/.test(del.data.message), 'admin : partie en cours supprimée')
    await ctx.refused(404, 'GET', '/games/:game', { params: { game: started.data.game_id } }, 'partie supprimée')
    trio.push(fourth)
    await ctx.refused(422, 'POST', '/rooms', { body: { pseudo: 'x' } }, 'pseudo trop court')
    await ctx.refused(422, 'POST', '/rooms/join', { body: { pseudo: ctx.pseudo('zz'), code: 'ZZZZZZ' } }, 'code de salon inexistant')
    await ctx.refused(404, 'GET', '/rooms/:room', { params: { room: 999999999 } }, 'salon inexistant')

    // ── Chat du hall : visiteur anonyme sur le canal public « lobby » ────────
    const visitor = await ctx.connect('visiteur')
    await visitor.subscribe('lobby')
    const anonId = ctx.tag.toUpperCase().replace(/[^A-Z0-9]/g, '') || 'E2E'
    let since = visitor.mark()
    const anon = await api.post('/chat/messages', { body: { body: 'Bonjour le hall', anon_id: anonId }, expect: 201 })
    ctx.check(anon.data.message.author_name === `Anonyme#${anonId}` && anon.data.message.is_anonymous === true && anon.data.message.author_key === `a:${anonId}`, 'chat : auteur anonyme dérivé côté serveur')
    await visitor.waitFor((e) => e.channel === 'lobby' && e.event === 'chat.message' && e.data.message.id === anon.data.message.id, { since, what: 'chat.message anonyme' })

    since = visitor.mark()
    const msg = await api.post('/chat/messages', { body: { body: `Venez dans ${tc.data.room.code} ! COUCOU`, player_id: trio[0].id, anon_id: 'ignored' }, expect: 201 })
    ctx.check(msg.data.message.author_name === trio[0].pseudo && msg.data.message.author_key === `p:${trio[0].id}`, 'chat : pseudo du joueur, non usurpable')
    ctx.check(JSON.stringify(msg.data.message.codes) === JSON.stringify([tc.data.room.code]), 'chat : seuls les codes de salons réels sont reconnus')
    await visitor.waitFor((e) => e.event === 'chat.message' && e.data.message.id === msg.data.message.id, { since, what: 'chat.message joueur' })
    await ctx.refused(422, 'POST', '/chat/messages', { body: { body: 'salut', anon_id: 'pas valide !' } }, 'chat : anon_id invalide')
    await ctx.refused(422, 'POST', '/chat/messages', { body: { body: '', anon_id: anonId } }, 'chat : message vide')
    await ctx.refused(422, 'POST', '/chat/messages', { body: { body: 'x'.repeat(501), anon_id: anonId } }, 'chat : message trop long')
    // Anti-spam : 20 messages / minute par auteur (ici un anonyme dédié)
    const spammer = `S${ctx.tag}`.replace(/[^A-Za-z0-9]/g, '').slice(0, 12)
    const statuses = []
    for (let i = 0; i < 21; i++) {
        statuses.push((await api.post('/chat/messages', { body: { body: `spam ${i}`, anon_id: spammer } })).status)
    }
    ctx.check(statuses.slice(0, 20).every((s) => s === 201) && statuses[20] === 429, 'chat : 21e message de la minute → 429', statuses)
    const other = await api.post('/chat/messages', { body: { body: 'je ne suis pas le spammeur', anon_id: `${spammer.slice(0, 10)}X` } })
    ctx.check(other.status === 201, 'chat : le quota est par auteur, pas global')

    const history = (await api.get('/chat/messages', { expect: 200 })).data.messages
    ctx.check(history.some((m) => m.id === msg.data.message.id) && history.length <= 50, 'chat : historique (50 max) contient les messages')
    visitor.close()

    // ── Emojis créés par un joueur ──────────────────────────────────────────
    const author = trio[0]
    const r = await api.post('/reactions', { body: { player_id: author.id, name: 'Carré', image: jpegDataUri('square-64.jpg') }, expect: 201 })
    ctx.created.reactions.add(r.data.reaction.id)
    ctx.check(r.data.reaction.created_by === author.pseudo && r.data.reaction.emoji === null && r.data.reaction.url.startsWith('/binome/api/reactions/'), 'emoji joueur : URL d\'image versionnée')
    await ctx.refused(422, 'POST', '/reactions', { body: { player_id: author.id, name: 'Trop grand', image: jpegDataUri('square-512.jpg') } }, 'emoji > 256 px')
    await ctx.refused(422, 'POST', '/reactions', { body: { player_id: author.id, name: 'Rectangle', image: jpegDataUri('rect-80x40.jpg') } }, 'emoji non carré')
    const list = (await api.get('/reactions', { expect: 200 })).data.reactions
    ctx.check(list.some((x) => x.id === r.data.reaction.id), 'GET /reactions : l\'emoji est disponible pour tous')
    const firstImage = list.findIndex((x) => x.emoji === null)
    ctx.check(firstImage === -1 || list.slice(firstImage).every((x) => x.emoji === null), 'GET /reactions : Unicode d\'abord, puis images')
    const image = await api.get('/reactions/:reaction/image', { params: { reaction: r.data.reaction.id }, raw: true, expect: 200 })
    ctx.check(image.data.contentType.startsWith('image/jpeg'), 'image d\'emoji servie en JPEG')

    for (const b of trio) await api.delete('/rooms/:room/leave', { params: { room: tc.data.room.id }, body: { player_id: b.id }, expect: 200 })
}
