// Partie complète : lobby → vote des personnages → rounds → fin → récap / historique.
//
// Options :
//   size  : nombre de joueurs (impair → orphelin)
//   rich  : sondes complètes (refus, déconnexions, passage de tour, exclusion,
//           tirage de carte partagé / gardé / expiré, votes des éliminés…).
//           Sans `rich`, la partie va droit au but (utile pour la charge).

import { Bot, everyone, gameState, marks, oracle, sleep } from '../lib/bots.mjs'
import { jpegDataUri } from '../lib/http.mjs'
import { connect } from '../lib/realtime.mjs'

const CARD_EXPIRY_TIMEOUT = 45_000 // ActionService::CARD_DRAW_SECONDS (30 s) + marge du worker

export async function fullGame(ctx, { size = 8, rich = true, redraw = rich } = {}) {
    const { api } = ctx
    const bots = Array.from({ length: size }, (_, i) => new Bot(ctx, `p${i + 1}`))
    const [host, ...guests] = bots
    const log = (m) => ctx.quiet || console.log(`   · ${m}`)
    const notes = {} // faits notés pendant la partie, vérifiés à la fin

    // ── LOBBY ────────────────────────────────────────────────────────────────
    const created = await api.post('/rooms', { body: { pseudo: host.pseudo, is_private: false, max_players: 12 }, expect: 201 })
    host.setId(created.data.player.id)
    // Historique réservé aux profils validés par l'admin (migration 6) : l'hôte le consulte
    await api.patch('/admin/players/:player/validation', { params: { player: host.id }, body: { validated: true }, admin: true, expect: 200 })
    const asHost = () => ({ 'X-Profile-Token': api.tokens.get(host.id) })
    const room = created.data.room
    ctx.check(/^[A-Z0-9]{6}$/.test(room.code), 'code de salon : 6 caractères A-Z0-9', room.code)
    await host.connect()
    const hereHost = await host.sub(`room.${room.id}`)
    ctx.check(hereHost.some((m) => m.id === host.id && m.pseudo === host.pseudo), 'presence room : l\'hôte se voit dans « here »')

    for (const g of guests) {
        const m = marks([host])
        const res = await api.post('/rooms/join', { body: { pseudo: g.pseudo, code: room.code }, expect: 200 })
        g.setId(res.data.player.id)
        ctx.check(res.data.room.id === room.id && res.data.room.current_game_id === null, `join ${g.name} : salon et pas de partie en cours`)
        await g.connect()
        await g.sub(`room.${room.id}`)
        await everyone([host], `room.${room.id}`, 'player.joined', { pred: (d) => d.player.id === g.id, since: m })
        await host.rt.waitFor((e) => e.event === 'presence:joining' && e.data.id === g.id, { since: m.get(host), what: `presence:joining ${g.name}` })
    }
    log(`${size} joueurs dans le salon ${room.code}`)
    const roomCh = `room.${room.id}`

    if (rich) {
        await ctx.refused(422, 'POST', '/rooms/join', { body: { pseudo: ctx.pseudo('zz'), code: 'AB' } }, 'join : code de mauvaise taille')
        await ctx.refused(409, 'POST', '/rooms', { body: { pseudo: guests[0].pseudo } }, 'créer un salon avec un pseudo connecté ailleurs')
        await ctx.refused(409, 'POST', '/rooms/join', { body: { pseudo: guests[0].pseudo, code: room.code } }, 'rejoindre avec un pseudo déjà connecté')
        await ctx.refused(403, 'POST', '/rooms/:room/start', { params: { room: room.id }, body: { player_id: guests[0].id } }, 'lancer la partie sans être l\'hôte')
        await ctx.refused(422, 'POST', '/rooms/:room/start', { params: { room: room.id }, body: { player_id: host.id } }, 'lancer alors que personne n\'est prêt')
        await ctx.refused(403, 'PATCH', '/rooms/:room/settings', { params: { room: room.id }, body: { player_id: guests[0].id, game_mode: 'random' } }, 'réglages par un non-hôte')
        await ctx.refused(422, 'PATCH', '/rooms/:room/settings', { params: { room: room.id }, body: { player_id: host.id, game_mode: 'cosmos' } }, 'mode cosmos sans cosmos_id')
        await ctx.refused(422, 'DELETE', '/rooms/:room/leave', { params: { room: room.id }, body: { player_id: 999999999 } }, 'quitter avec un joueur inexistant')

        // Réglages : cosmos puis retour en aléatoire, diffusés à tout le salon
        const cosmos = (await api.get('/cosmos', { expect: 200 })).data.cosmos
        const playable = cosmos.find((c) => c.is_enabled && c.playable_universe_count >= Math.ceil(size / 2))
        if (playable) {
            const m = marks(bots)
            await api.patch('/rooms/:room/settings', { params: { room: room.id }, body: { player_id: host.id, game_mode: 'cosmos', cosmos_id: playable.id }, expect: 200 })
            const [ev] = await everyone(bots, roomCh, 'room.settings.updated', { since: m })
            ctx.check(ev.data.game_mode === 'cosmos' && ev.data.cosmos_id === playable.id && ev.data.cosmos_name === playable.name, 'room.settings.updated : mode cosmos diffusé')
        }
        const m = marks(bots)
        const back = await api.patch('/rooms/:room/settings', { params: { room: room.id }, body: { player_id: host.id, game_mode: 'random', reveal_to_eliminated: true, best_question_vote: true }, expect: 200 })
        ctx.check(back.data.cosmos_id === null, 'retour au mode aléatoire : cosmos_id remis à null')
        await everyone(bots, roomCh, 'room.settings.updated', { pred: (d) => d.game_mode === 'random', since: m })

        // Départ puis retour d'un invité avant la partie
        const leaver = guests[guests.length - 1]
        let mk = marks([host])
        leaver.unsub(roomCh)
        await api.delete('/rooms/:room/leave', { params: { room: room.id }, body: { player_id: leaver.id }, expect: 200 })
        const [left] = await everyone([host], roomCh, 'player.left', { since: mk })
        ctx.check(!left.data.players.some((p) => p.id === leaver.id) && left.data.new_host_id === null, 'player.left : liste à jour, pas de changement d\'hôte')
        mk = marks([host])
        const rejoin = await api.post('/rooms/join', { body: { pseudo: leaver.pseudo, code: room.code }, expect: 200 })
        ctx.check(rejoin.data.player.id === leaver.id, 'retour au salon : le joueur garde son profil (même id)')
        leaver.setId(rejoin.data.player.id)
        await leaver.reconnect()
        await leaver.sub(roomCh)
        await everyone([host], roomCh, 'player.joined', { pred: (d) => d.player.id === leaver.id, since: mk })

        // Photo de profil
        mk = marks(bots)
        const av = await api.post('/players/:player/avatar', { params: { player: host.id }, body: { player_id: host.id, image: jpegDataUri('square-64.jpg') }, expect: 200 })
        const [avEv] = await everyone(bots, roomCh, 'player.avatar.updated', { pred: (d) => d.player_id === host.id, since: mk })
        ctx.check(avEv.data.avatar_url === av.data.avatar_url && av.data.avatar_url.startsWith('/binome/api/avatars/'), 'avatar : URL versionnée préfixée /binome diffusée')
        // URL signée (checklist 7 D) : sans sa signature, ni la photo ni ses voisines ne sortent
        const avatarId = Number(av.data.avatar_url.match(/avatars\/(\d+)/)[1])
        const signed = Object.fromEntries(new URL(av.data.avatar_url, 'http://x').searchParams)
        const img = await api.get('/avatars/:avatar', { params: { avatar: avatarId }, query: signed, raw: true, expect: 200 })
        ctx.check(img.data.contentType.startsWith('image/jpeg') && /immutable/.test(img.data.cacheControl ?? ''), 'avatar servi en image/jpeg, cache immutable')
        await ctx.refused(404, 'GET', '/avatars/:avatar', { params: { avatar: avatarId }, query: { v: signed.v }, raw: true }, 'avatar sans signature')
        await ctx.refused(404, 'GET', '/avatars/:avatar', { params: { avatar: avatarId }, query: { ...signed, s: '0'.repeat(32) }, raw: true }, 'avatar avec une fausse signature')
        await ctx.refused(404, 'GET', '/avatars/:avatar', { params: { avatar: avatarId - 1 }, query: signed, raw: true }, 'signature d\'un autre avatar (id voisin)')
        await ctx.refused(422, 'POST', '/players/:player/avatar', { params: { player: host.id }, body: { player_id: host.id, image: jpegDataUri('rect-80x40.jpg') } }, 'avatar non carré')
        await ctx.refused(403, 'POST', '/players/:player/avatar', { params: { player: host.id }, body: { player_id: guests[0].id, image: jpegDataUri('square-64.jpg') } }, 'avatar d\'un autre joueur')
        const roomView = (await api.get('/rooms/:room', { params: { room: room.id }, expect: 200 })).data.room
        ctx.check(roomView.players.find((p) => p.id === host.id)?.avatar_url === av.data.avatar_url, 'GET /rooms : avatar_url dans la liste des joueurs')
    }

    // Tous prêts (bascule : un aller-retour pour le premier invité)
    if (rich) {
        const on = await api.patch('/rooms/:room/ready', { params: { room: room.id }, body: { player_id: guests[0].id }, expect: 200 })
        const off = await api.patch('/rooms/:room/ready', { params: { room: room.id }, body: { player_id: guests[0].id }, expect: 200 })
        ctx.check(on.data.is_ready === true && off.data.is_ready === false, 'ready : bascule prêt / pas prêt')
    }
    let lastReady
    for (const b of bots) {
        const m = marks([host])
        lastReady = await api.patch('/rooms/:room/ready', { params: { room: room.id }, body: { player_id: b.id }, expect: 200 })
        await everyone([host], roomCh, 'player.ready', { pred: (d) => d.player_id === b.id, since: m })
    }
    ctx.check(lastReady.data.all_ready && lastReady.data.can_start, 'ready : tous prêts → can_start')

    // ── LANCEMENT ────────────────────────────────────────────────────────────
    let m = marks(bots)
    const start = await api.post('/rooms/:room/start', { params: { room: room.id }, body: { player_id: host.id }, expect: 201 })
    const gameId = start.data.game_id
    ctx.created.games.add(gameId)
    const [started] = await everyone(bots, roomCh, 'game.started', { since: m })
    ctx.check(started.data.game_id === gameId && started.data.players.length === size, 'game.started : tous les joueurs, à plat')
    ctx.check(started.data.has_orphan === (size % 2 === 1), `game.started : has_orphan = ${size % 2 === 1}`)
    ctx.check(!JSON.stringify(started.data).includes('character'), 'game.started : aucun personnage divulgué')
    if (rich) {
        await ctx.refused(409, 'POST', '/rooms/:room/start', { params: { room: room.id }, body: { player_id: host.id } }, 'relancer une partie déjà en cours')
    }

    const gameCh = `game.${gameId}`
    for (const b of bots) {
        await b.sub(gameCh)
        await b.sub(`player.${b.id}`)
    }

    // Spectateur : rejoint le salon après le lancement
    let spectator = null
    if (rich) {
        spectator = new Bot(ctx, 'spec')
        const res = await api.post('/rooms/join', { body: { pseudo: spectator.pseudo, code: room.code }, expect: 200 })
        spectator.setId(res.data.player.id)
        ctx.check(res.data.room.current_game_id === gameId, 'spectateur : current_game_id renvoyé au join')
        await spectator.connect()
        await spectator.sub(roomCh)
        const members = await spectator.sub(gameCh)
        ctx.check(members.length >= size, 'spectateur : presence game → les joueurs connectés')
        await spectator.sub(`player.${spectator.id}`)
        await ctx.refused(404, 'GET', '/games/:game/me', { params: { game: gameId }, query: { player_id: spectator.id } }, '/me d\'un spectateur')

        // Autorisations de canaux
        const rejected = await spectator.rt.subscribe(`player.${host.id}`).then(() => null, (e) => e)
        ctx.check(rejected && rejected.status === 403, 'canal privé player.{id} d\'un autre joueur refusé (403)')
        const outsider = new Bot(ctx, 'out')
        const o = await api.post('/rooms', { body: { pseudo: outsider.pseudo }, expect: 201 })
        outsider.setId(o.data.player.id)
        await outsider.connect()
        const refusedGame = await outsider.rt.subscribe(gameCh).then(() => null, (e) => e)
        ctx.check(refusedGame && refusedGame.status === 403, 'presence game.{id} refusée à un joueur hors du salon (403)')
        const refusedRoom = await outsider.rt.subscribe(roomCh).then(() => null, (e) => e)
        ctx.check(refusedRoom && refusedRoom.status === 403, 'presence room.{id} refusée à un joueur hors du salon (403)')
        await api.delete('/rooms/:room/leave', { params: { room: o.data.room.id }, body: { player_id: outsider.id }, expect: 200 })
        outsider.rt.close()

        // ── Identité signée (checklist point 5) : un player_id n'est cru qu'avec son jeton
        const victim = bots[1]
        const forged = `${victim.id}.${'0'.repeat(64)}`
        const asSpectator = { 'X-Profile-Token': api.tokens.get(spectator.id) }
        const me = (extra) => ({ params: { game: gameId }, query: { player_id: victim.id }, ...extra })
        await ctx.refused(401, 'GET', '/games/:game/me', me({ auth: false }), '/me d\'un joueur sans jeton')
        await ctx.refused(401, 'GET', '/games/:game/me', me({ headers: { 'X-Profile-Token': forged } }), '/me avec un jeton falsifié')
        await ctx.refused(403, 'GET', '/games/:game/me', me({ headers: asSpectator }), '/me d\'un autre joueur avec son propre jeton')
        await ctx.refused(403, 'POST', '/games/:game/reactions', { params: { game: gameId }, body: { player_id: host.id, reaction_id: 1 }, headers: asSpectator }, 'action au nom d\'un autre joueur')
        await ctx.refused(403, 'PATCH', '/rooms/:room/ready', { params: { room: room.id }, body: { player_id: victim.id }, headers: asSpectator }, 'salon : prêt au nom d\'un autre joueur')
        // WebSocket : le joueur vient du jeton, jamais d'un identifiant fourni
        const wsForged = await ctx.connect('ws-forge', null, forged)
        const r1 = await wsForged.subscribe(`player.${victim.id}`).then(() => null, (e) => e)
        ctx.check(r1?.status === 403, 'WebSocket : jeton falsifié → canal privé refusé (403)')
        wsForged.close()
        const legacy = await connect({ ...ctx.target.realtime, wsUrl: `${ctx.target.realtime.wsUrl}?player_id=${victim.id}`, label: 'ws-legacy', recorder: ctx.recorder })
        ctx.clients.add(legacy)
        const r2 = await legacy.subscribe(`player.${victim.id}`).then(() => null, (e) => e)
        ctx.check(r2?.status === 403, 'WebSocket : ancien ?player_id= ignoré → canal privé refusé (403)')
        const r3 = await legacy.subscribe(gameCh).then(() => null, (e) => e)
        ctx.check(r3?.status === 403, 'WebSocket : sans jeton, presence game.{id} refusée (403)')
        legacy.close()
    }

    const watchers = () => [...bots, ...(spectator ? [spectator] : [])].filter((b) => !b.offline)
    const byId = (id) => bots.find((b) => b.id === id)

    // ── VOTE DES PERSONNAGES ────────────────────────────────────────────────
    let state = await gameState(ctx, gameId)
    ctx.check(state.characters_confirmed === false && state.current_round === null && state.draw_number === 1, 'vote : pas de round tant que les personnages ne sont pas acceptés')
    ctx.check(state.has_orphan === (size % 2 === 1) && state.players.length === size, 'GET /games : joueurs à plat + has_orphan')
    ctx.check(state.binomes.length === 0 && !JSON.stringify(state).includes('forbidden_words'), 'GET /games : aucun binôme ni personnage exposé')

    const loadCharacters = async () => {
        for (const b of bots) {
            b.character = (await api.get('/games/:game/me', { params: { game: gameId }, query: { player_id: b.id }, expect: 200 })).data.character
        }
    }
    await loadCharacters()
    ctx.check(bots.every((b) => b.character.forbidden_words.length === 3 && (b.character.image === null || b.character.image.startsWith('/binome/api/character-images/'))), '/me : 3 mots interdits, image en URL versionnée')

    if (rich) {
        await ctx.refused(409, 'POST', '/games/:game/skip-turn', { params: { game: gameId }, body: { player_id: host.id } }, 'passer un tour pendant le vote')
        await ctx.refused(409, 'GET', '/games/:game/recap', { params: { game: gameId } }, 'récap d\'une partie en cours')
        await ctx.refused(404, 'GET', '/history/games/:game', { params: { game: gameId }, headers: asHost() }, 'historique d\'une partie en cours')
        await ctx.refused(403, 'GET', '/games/:game/reveal', { params: { game: gameId }, query: { player_id: host.id } }, 'vue omnisciente d\'un joueur en jeu')
        const wrong = bots[1]
        await ctx.refused(409, 'POST', '/games/:game/character-vote', { params: { game: gameId }, body: { player_id: wrong.id, character_id: wrong.character.id + 100000, accepted: true } }, 'vote sur un personnage qui n\'est pas le sien')
    }

    if (redraw) {
        // Une demande de changement → retirage pour tous une fois que tous ont voté
        const asker = bots[1]
        const oldIds = new Map(bots.map((b) => [b.id, b.character.id]))
        for (const [i, b] of bots.entries()) {
            const mk = marks(watchers())
            await api.post('/games/:game/character-vote', { params: { game: gameId }, body: { player_id: b.id, character_id: b.character.id, accepted: b !== asker }, expect: 200 })
            if (i < bots.length - 1) {
                const [ev] = await everyone(watchers(), gameCh, 'character.vote.cast', { since: mk })
                if (i === 1) ctx.check(ev.data.change_player_ids.includes(asker.id), 'character.vote.cast : demande de changement publique')
                if (b === asker) await ctx.refused(409, 'POST', '/games/:game/character-vote', { params: { game: gameId }, body: { player_id: asker.id, character_id: asker.character.id, accepted: true } }, 'voter deux fois pour le même tirage')
            } else {
                const [ev] = await everyone(watchers(), gameCh, 'characters.redrawn', { since: mk })
                ctx.check(ev.data.draw_number === 2 && ev.data.change_count === 1, 'characters.redrawn : tirage n°2, 1 demande')
                const left = new Map(ev.data.character_changes_left.map((c) => [c.player_id, c.left]))
                ctx.check(left.size === bots.length && left.get(asker.id) === 1 && bots.every((x) => x === asker || left.get(x.id) === 2),
                    'characters.redrawn : 1 changement restant au demandeur, 2 aux autres')
            }
        }
        await loadCharacters()
        ctx.check(!bots.some((b) => b.character.id === oldIds.get(asker.id)), 'retirage : le personnage refusé n\'est plus distribué')

        // Deuxième et dernier changement du même joueur → tirage n°3, puis refus
        let mk = marks(watchers())
        for (const b of bots) {
            await api.post('/games/:game/character-vote', { params: { game: gameId }, body: { player_id: b.id, character_id: b.character.id, accepted: b !== asker }, expect: 200 })
        }
        const [ev3] = await everyone(watchers(), gameCh, 'characters.redrawn', { since: mk })
        ctx.check(ev3.data.draw_number === 3 && ev3.data.character_changes_left.find((c) => c.player_id === asker.id)?.left === 0,
            'characters.redrawn : tirage n°3, plus aucun changement pour le demandeur')
        const st = await gameState(ctx, gameId)
        ctx.check(st.max_character_changes === 2 && st.players.find((p) => p.id === asker.id)?.character_changes_left === 0
            && st.players.every((p) => p.id === asker.id || p.character_changes_left === 2), 'GET /games : changements restants par joueur')
        await loadCharacters()
        await ctx.refused(409, 'POST', '/games/:game/character-vote', { params: { game: gameId }, body: { player_id: asker.id, character_id: asker.character.id, accepted: false } }, 'troisième demande de changement')

        // Un autre joueur peut encore demander : retirage pour tous, demandeur compris
        const other = bots[2]
        mk = marks(watchers())
        for (const b of bots) {
            await api.post('/games/:game/character-vote', { params: { game: gameId }, body: { player_id: b.id, character_id: b.character.id, accepted: b !== other }, expect: 200 })
        }
        const [ev4] = await everyone(watchers(), gameCh, 'characters.redrawn', { since: mk })
        const left4 = new Map(ev4.data.character_changes_left.map((c) => [c.player_id, c.left]))
        ctx.check(ev4.data.draw_number === 4 && left4.get(asker.id) === 0 && left4.get(other.id) === 1, 'characters.redrawn : retirage demandé par un autre joueur')
        await loadCharacters()
    }

    m = marks(watchers())
    for (const b of bots) {
        await api.post('/games/:game/character-vote', { params: { game: gameId }, body: { player_id: b.id, character_id: b.character.id, accepted: true }, expect: 200 })
    }
    const [r1] = await everyone(watchers(), gameCh, 'round.started', { since: m })
    ctx.check(r1.data.number === 1 && r1.data.is_new_round === true, 'round.started : round 1, is_new_round')
    const sortedIds = bots.map((b) => b.id).sort((a, b) => a - b)
    ctx.check(r1.data.current_player.id === sortedIds[0], 'ordre des tours : le plus petit id commence')
    log(`partie ${gameId} : personnages acceptés${redraw ? ' après un retirage' : ''}`)

    const truth = await oracle(ctx, gameId)
    for (const b of bots) {
        ctx.check(truth.get(b.id).characterId === b.character.id, `/me ${b.name} = personnage réel`)
    }

    // ── MOTEUR DE TOURS ─────────────────────────────────────────────────────
    const questionsByRound = new Map() // round → [{ actionId, authorId }]
    let qn = 0

    const turn = async () => {
        const s = await gameState(ctx, gameId)
        if (s.status === 'finished') return null
        const cur = byId(s.current_round.current_player_id)
        const alive = s.players.filter((p) => !p.is_eliminated).map((p) => byId(p.id))
        return { s, cur, alive, round: s.current_round, params: { game: gameId, round: s.current_round.id } }
    }
    const nextAlive = (t, from, exclude = []) => {
        const order = t.alive.filter((b) => b !== from && !exclude.includes(b)).sort((a, b) => a.id - b.id)
        return order.find((b) => b.id > from.id) ?? order[0]
    }
    /** Fin de tour : round.started (tour suivant) ou game.ended, reçu par tous. */
    const turnEnded = async (mk) => {
        const results = await Promise.all(watchers().map((b) => b.rt.waitFor(
            (e) => e.channel === gameCh && (e.event === 'round.started' || e.event === 'game.ended'),
            { since: mk.get(b) ?? 0, what: 'fin de tour' },
        )))
        return results[0]
    }

    const ask = async (t, target, extra = {}) => {
        const mk = marks(watchers())
        qn += 1
        const res = await api.post('/games/:game/rounds/:round/question', {
            params: t.params,
            body: { player_id: t.cur.id, target_player_id: target.id, question: `Question ${qn} : ton personnage sait-il voler ?` },
            expect: 200,
        })
        const actionId = res.data.action.id
        const [ev] = await everyone(watchers(), gameCh, 'action.played', { pred: (d) => d.action_id === actionId, since: mk })
        if (!extra.quiet) ctx.check(ev.data.type === 'question' && ev.data.target_player.id === target.id && ev.data.question?.startsWith('Question'), 'action.played : question publique')
        if (!questionsByRound.has(t.round.number)) questionsByRound.set(t.round.number, [])
        questionsByRound.get(t.round.number).push({ actionId, authorId: t.cur.id })
        return actionId
    }

    const answer = async (t, actionId, target, value) => {
        const mk = marks(watchers())
        await api.post('/games/:game/rounds/:round/actions/:action/answer', { params: { ...t.params, action: actionId }, body: { player_id: target.id, answer: value }, expect: 200 })
        const [ev] = await everyone(watchers(), gameCh, 'answer.given', { pred: (d) => d.action_id === actionId, since: mk })
        return { ev, mk }
    }

    const accuse = async (t, target, correct) => {
        const name = correct ? truth.get(target.id).character : 'Personnage Imaginaire E2E'
        let mk = marks(watchers())
        const res = await api.post('/games/:game/rounds/:round/accusation', { params: t.params, body: { player_id: t.cur.id, target_player_id: target.id, character_name: name }, expect: 200 })
        const actionId = res.data.action.id
        const [played] = await everyone(watchers(), gameCh, 'action.played', { pred: (d) => d.action_id === actionId, since: mk })
        ctx.check(played.data.type === 'accusation' && played.data.accusation_confirmed === null && played.data.character_name === name, 'action.played : accusation en attente de confirmation')
        return { actionId, name, mk }
    }

    const confirm = async (t, actionId, target, confirmed) => {
        const mk = marks(watchers())
        await api.post('/games/:game/rounds/:round/actions/:action/confirm', { params: { ...t.params, action: actionId }, body: { player_id: target.id, confirmed }, expect: 200 })
        const [ev] = await everyone(watchers(), gameCh, 'accusation.confirmed', { pred: (d) => d.action_id === actionId, since: mk })
        ctx.check(ev.data.accusation_correct === confirmed, `accusation.confirmed : correcte = ${confirmed}`)
        if (confirmed) await everyone(watchers(), gameCh, 'player.eliminated', { pred: (d) => d.eliminated_player.id === target.id, since: mk })
        return mk
    }

    const fillerQuestion = async (t) => {
        const target = nextAlive(t, t.cur)
        const id = await ask(t, target, { quiet: true })
        const { mk } = await answer(t, id, target, qn % 2 ? 'yes' : 'no')
        await turnEnded(mk)
    }

    const correctAccusation = async (t, target = nextAlive(t, t.cur)) => {
        const { actionId } = await accuse(t, target, true)
        const mk = await confirm(t, actionId, target, true)
        await turnEnded(mk)
    }

    const waitPresence = (who, event, id) => who.rt.waitFor(
        (e) => e.channel === gameCh && e.event === event && e.data.id === id,
        { since: 0, what: `${event} ${id}`, timeout: 10_000 },
    )
    const freshPresence = async (who, event, id) => {
        const since = who.rt.mark()
        return who.rt.waitFor((e) => e.channel === gameCh && e.event === event && e.data.id === id, { since, what: `${event} ${id}`, timeout: 10_000 })
    }

    // Étapes ciblées (mode `rich`) : chacune attend un tour où elle est jouable.
    const steps = !rich ? [] : [
        {
            name: 'question + sondes + corrections de l\'hôte',
            when: (t) => t.round.number === 1,
            run: async (t) => {
                const other = nextAlive(t, t.cur)
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/question', { params: t.params, body: { player_id: other.id, target_player_id: t.cur.id, question: 'Pas mon tour de jouer ?' } }, 'question hors de son tour')
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/question', { params: t.params, body: { player_id: t.cur.id, target_player_id: t.cur.id, question: 'Je me demande à moi ?' } }, 'question à soi-même')
                await ctx.refused(422, 'POST', '/games/:game/rounds/:round/question', { params: t.params, body: { player_id: t.cur.id, target_player_id: other.id, question: 'abc' } }, 'question trop courte')
                const id = await ask(t, other)
                const { ev, mk } = await answer(t, id, other, 'yes')
                ctx.check(ev.data.answer === 'yes' && ev.data.card_draw === null && ev.data.asker.id === t.cur.id, 'answer.given : oui, pas de tirage')
                await turnEnded(mk)

                const corrector = t.cur === host ? host : host
                let m2 = marks(watchers())
                const c1 = await api.post('/games/:game/actions/:action/correct-answer', { params: { game: gameId, action: id }, body: { player_id: corrector.id, answer: 'no' }, expect: 200 })
                ctx.check(c1.data.answer === 'no' && c1.data.answer_corrected_from === 'yes', 'correction de l\'hôte : oui → non, réponse d\'origine gardée')
                const [corr] = await everyone(watchers(), gameCh, 'answer.corrected', { pred: (d) => d.action_id === id, since: m2 })
                ctx.check(corr.data.previous_answer === 'yes' && corr.data.host.id === host.id, 'answer.corrected diffusé')
                m2 = marks(watchers())
                const c2 = await api.post('/games/:game/actions/:action/correct-answer', { params: { game: gameId, action: id }, body: { player_id: host.id, answer: 'yes' }, expect: 200 })
                ctx.check(c2.data.answer_corrected_from === null, 'retour à la réponse d\'origine : trace effacée')
                await everyone(watchers(), gameCh, 'answer.corrected', { pred: (d) => d.action_id === id && d.answer === 'yes', since: m2 })
                const notHost = bots.find((b) => b !== host)
                await ctx.refused(409, 'POST', '/games/:game/actions/:action/correct-answer', { params: { game: gameId, action: id }, body: { player_id: notHost.id, answer: 'no' } }, 'correction par un non-hôte')
                await ctx.refused(422, 'POST', '/games/:game/actions/:action/correct-answer', { params: { game: gameId, action: id }, body: { player_id: host.id, answer: 'dont_know' } }, 'correction vers « je ne sais pas »')
            },
        },
        {
            name: 'réponse : sondes',
            when: (t) => t.round.number === 1,
            run: async (t) => {
                const target = nextAlive(t, t.cur)
                const id = await ask(t, target)
                const intruder = nextAlive(t, target, [t.cur])
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/answer', { params: { ...t.params, action: id }, body: { player_id: intruder.id, answer: 'no' } }, 'réponse par un joueur non ciblé')
                await ctx.refused(422, 'POST', '/games/:game/rounds/:round/actions/:action/answer', { params: { ...t.params, action: id }, body: { player_id: target.id, answer: 'maybe' } }, 'réponse invalide')
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/confirm', { params: { ...t.params, action: id }, body: { player_id: target.id, confirmed: true } }, 'confirmer une question')
                const { mk } = await answer(t, id, target, 'no')
                await turnEnded(mk)
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/answer', { params: { ...t.params, action: id }, body: { player_id: target.id, answer: 'yes' } }, 'deuxième réponse à la même question')
            },
        },
        {
            name: 'je ne sais pas → carte montrée à tous sauf la cible',
            when: (t) => t.round.number === 1,
            run: async (t) => {
                const target = nextAlive(t, t.cur)
                const id = await ask(t, target)
                const { ev } = await answer(t, id, target, 'dont_know')
                ctx.check(ev.data.card_draw?.status === 'pending' && ev.data.card_draw.card_count === 3 && typeof ev.data.card_draw.deadline === 'string', 'answer.given : tirage ouvert (3 cartes, échéance)')
                const p = { ...t.params, action: id }
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/draw/decision', { params: p, body: { player_id: t.cur.id, share: true } }, 'décider avant d\'avoir tiré')
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/draw', { params: p, body: { player_id: target.id, card: 0 } }, 'tirage par un autre que le questionneur')
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/draw', { params: p, body: { player_id: t.cur.id, card: 7 } }, 'carte inexistante')
                let mk = marks(watchers())
                const drawn = await api.post('/games/:game/rounds/:round/actions/:action/draw', { params: p, body: { player_id: t.cur.id, card: 1 }, expect: 200 })
                ctx.check(truth.get(target.id).forbidden_words.includes(drawn.data.word) && drawn.data.reveal_seconds === 5, 'tirage : mot interdit de la cible, renvoyé au seul questionneur')
                const [cd] = await everyone(watchers(), gameCh, 'card.drawn', { pred: (d) => d.action_id === id, since: mk })
                ctx.check(cd.data.card === 1 && !('word' in cd.data), 'card.drawn : index de carte, jamais le mot')
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/draw', { params: p, body: { player_id: t.cur.id, card: 0 } }, 'tirer une deuxième carte')
                const st = await gameState(ctx, gameId)
                const drawnAction = st.actions.find((a) => a.id === id)
                ctx.check(!('drawn_word' in drawnAction) && !('word' in drawnAction) && !('word' in drawnAction.card_draw), 'GET /games : le mot tiré n\'est pas sérialisé')
                ctx.check(st.actions.find((a) => a.id === id)?.card_draw?.status === 'drawn', 'GET /games : card_draw.status = drawn')

                mk = marks(watchers())
                await api.post('/games/:game/rounds/:round/actions/:action/draw/decision', { params: p, body: { player_id: t.cur.id, share: true }, expect: 200 })
                await everyone(watchers(), gameCh, 'card.draw.finished', { pred: (d) => d.action_id === id, since: mk })
                const recipients = watchers().filter((b) => b !== t.cur && b !== target)
                const shared = await everyone(recipients, (b) => `player.${b.id}`, 'card.word.shared', { pred: (d) => d.action_id === id, since: mk })
                ctx.check(shared.every((e) => e.data.word === drawn.data.word && e.data.target.id === target.id), `card.word.shared : reçu par les ${recipients.length} autres (spectateur compris)`)
                await sleep(300)
                ctx.check(target.rt.received('card.word.shared', { since: mk.get(target) }).length === 0, 'card.word.shared : JAMAIS reçu par la cible')
                ctx.check(t.cur.rt.received('card.word.shared', { since: mk.get(t.cur) }).length === 0, 'card.word.shared : pas renvoyé au questionneur')
                await turnEnded(mk)
                const after = await gameState(ctx, gameId)
                ctx.check(after.actions.find((a) => a.id === id)?.card_draw?.status === 'done', 'après décision : card_draw.status = done (montré / gardé indiscernables)')
            },
        },
        {
            name: 'je ne sais pas → carte gardée',
            when: (t) => t.round.number === 1,
            run: async (t) => {
                const target = nextAlive(t, t.cur)
                const id = await ask(t, target)
                await answer(t, id, target, 'dont_know')
                const p = { ...t.params, action: id }
                await api.post('/games/:game/rounds/:round/actions/:action/draw', { params: p, body: { player_id: t.cur.id, card: 0 }, expect: 200 })
                const mk = marks(watchers())
                await api.post('/games/:game/rounds/:round/actions/:action/draw/decision', { params: p, body: { player_id: t.cur.id, share: false }, expect: 200 })
                await everyone(watchers(), gameCh, 'card.draw.finished', { pred: (d) => d.action_id === id, since: mk })
                await turnEnded(mk)
                await sleep(300)
                ctx.check(watchers().every((b) => b.rt.received('card.word.shared', { since: mk.get(b) }).length === 0), 'carte gardée : personne ne reçoit le mot')
            },
        },
        {
            name: 'je ne sais pas → tirage expiré (30 s, minuterie serveur)',
            when: (t) => t.round.number === 1,
            run: async (t) => {
                const target = nextAlive(t, t.cur)
                const id = await ask(t, target)
                const { mk } = await answer(t, id, target, 'dont_know')
                const t0 = Date.now()
                await everyone(watchers(), gameCh, 'card.draw.finished', { pred: (d) => d.action_id === id, since: mk, timeout: CARD_EXPIRY_TIMEOUT })
                const waited = (Date.now() - t0) / 1000
                ctx.check(waited >= 25 && waited <= 40, `expiration du tirage après ~30 s (${waited.toFixed(1)} s)`)
                await turnEnded(mk)
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/draw', { params: { ...t.params, action: id }, body: { player_id: t.cur.id, card: 0 } }, 'tirer après expiration')
            },
        },
        {
            name: 'passage de tour refusé : une question attend sa réponse (cible en ligne)',
            when: (t) => t.round.number === 1,
            run: async (t) => {
                const target = nextAlive(t, t.cur)
                const id = await ask(t, target)
                await ctx.refused(409, 'POST', '/games/:game/skip-turn', { params: { game: gameId }, body: { player_id: host.id } }, 'passer le tour pendant qu\'une question attend')
                const { mk } = await answer(t, id, target, 'yes')
                await turnEnded(mk)
            },
        },
        {
            name: 'accusation fausse + sondes de confirmation',
            when: (t) => t.round.number >= 2,
            run: async (t) => {
                const target = nextAlive(t, t.cur)
                await ctx.refused(422, 'POST', '/games/:game/rounds/:round/accusation', { params: t.params, body: { player_id: t.cur.id, target_player_id: t.cur.id, character_name: 'Moi' } }, 'accusation de soi-même')
                const { actionId } = await accuse(t, target, false)
                const intruder = nextAlive(t, target, [t.cur])
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/confirm', { params: { ...t.params, action: actionId }, body: { player_id: intruder.id, confirmed: false } }, 'confirmation par un autre que l\'accusé')
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/answer', { params: { ...t.params, action: actionId }, body: { player_id: target.id, answer: 'yes' } }, 'répondre à une accusation')
                const mk = await confirm(t, actionId, target, false)
                await turnEnded(mk)
                await ctx.refused(409, 'POST', '/games/:game/rounds/:round/actions/:action/confirm', { params: { ...t.params, action: actionId }, body: { player_id: target.id, confirmed: true } }, 'confirmer deux fois')
            },
        },
        {
            name: 'accusation juste → éliminé : vue omnisciente + vote meilleure question',
            when: (t) => t.round.number >= 2 && nextAlive(t, t.cur, [host]),
            run: async (t) => {
                const target = nextAlive(t, t.cur, [host])
                await correctAccusation(t, target)
                const reveal = await api.get('/games/:game/reveal', { params: { game: gameId }, query: { player_id: target.id }, expect: 200 })
                ctx.check(reveal.data.binomes.length === Math.ceil(size / 2) && reveal.data.binomes.every((b) => b.players.every((p) => p.forbidden_words.length === 3)), 'éliminé : vue omnisciente (tous les binômes, personnages, mots)')
                const s = await gameState(ctx, gameId)
                const aliveOne = s.players.find((p) => !p.is_eliminated)
                await ctx.refused(409, 'POST', '/games/:game/question-votes', { params: { game: gameId }, body: { player_id: aliveOne.id, action_id: questionsByRound.get(1)[0].actionId } }, 'vote d\'un joueur encore en jeu')

                const partnerId = truth.get(target.id).partnerId
                // Seuls le round courant et le précédent sont encore votables
                const now = (await gameState(ctx, gameId)).current_round?.number ?? t.round.number
                const r1q = [...questionsByRound.entries()].filter(([r]) => r >= now - 1).flatMap(([, q]) => q)
                const own = r1q.find((q) => q.authorId === target.id)
                const partners = r1q.find((q) => q.authorId === partnerId)
                const valid = r1q.filter((q) => q.authorId !== target.id && q.authorId !== partnerId)
                if (own) await ctx.refused(409, 'POST', '/games/:game/question-votes', { params: { game: gameId }, body: { player_id: target.id, action_id: own.actionId } }, 'voter pour sa propre question')
                if (partners) await ctx.refused(409, 'POST', '/games/:game/question-votes', { params: { game: gameId }, body: { player_id: target.id, action_id: partners.actionId } }, 'voter pour la question de son binôme')
                let mk = marks(watchers())
                await api.post('/games/:game/question-votes', { params: { game: gameId }, body: { player_id: target.id, action_id: valid[0].actionId }, expect: 200 })
                await everyone(watchers(), gameCh, 'question.voted', { pred: (d) => d.votes.some((v) => v.voter_id === target.id && v.action_id === valid[0].actionId), since: mk })
                mk = marks(watchers())
                await api.post('/games/:game/question-votes', { params: { game: gameId }, body: { player_id: target.id, action_id: valid[1].actionId }, expect: 200 })
                const [qv] = await everyone(watchers(), gameCh, 'question.voted', { since: mk })
                ctx.check(qv.data.votes.filter((v) => v.voter_id === target.id).length === 1 && qv.data.votes[0].action_id === valid[1].actionId, 'changer d\'avis remplace le vote')
                const st = await gameState(ctx, gameId)
                ctx.check(st.question_votes.some((v) => v.voter_id === target.id && v.action_id === valid[1].actionId), 'GET /games : question_votes publics')
                notes.bestQuestionAuthor = valid[1].authorId
            },
        },
        {
            name: 'réactions rapides (joueur + spectateur)',
            when: () => true,
            run: async (t) => {
                const list = (await api.get('/reactions', { expect: 200 })).data.reactions
                const unicode = list.find((r) => r.emoji)
                let mk = marks(watchers())
                await api.post('/games/:game/reactions', { params: { game: gameId }, body: { player_id: t.cur.id, reaction_id: unicode.id }, expect: 200 })
                const [rs] = await everyone(watchers(), gameCh, 'reaction.sent', { since: mk })
                ctx.check(rs.data.reaction.id === unicode.id && rs.data.reaction.emoji === unicode.emoji && rs.data.player.id === t.cur.id, 'reaction.sent : emoji Unicode diffusé')
                if (spectator) {
                    mk = marks(watchers())
                    await api.post('/games/:game/reactions', { params: { game: gameId }, body: { player_id: spectator.id, reaction_id: unicode.id }, expect: 200 })
                    await everyone(watchers(), gameCh, 'reaction.sent', { pred: (d) => d.player.id === spectator.id, since: mk })
                }
                await ctx.refused(404, 'POST', '/games/:game/reactions', { params: { game: gameId }, body: { player_id: t.cur.id, reaction_id: 999999999 } }, 'réaction inexistante')

                // Photo retirée en pleine partie : diffusée sur le salon ET la partie
                mk = marks(watchers())
                const del = await api.delete('/players/:player/avatar', { params: { player: host.id }, body: { player_id: host.id }, expect: 200 })
                ctx.check(del.data.avatar_url === null, 'photo retirée : avatar_url null')
                await everyone(watchers(), gameCh, 'player.avatar.updated', { pred: (d) => d.player_id === host.id && d.avatar_url === null, since: mk })
                await everyone(watchers(), roomCh, 'player.avatar.updated', { pred: (d) => d.player_id === host.id && d.avatar_url === null, since: mk })
                await ctx.refused(403, 'DELETE', '/players/:player/avatar', { params: { player: host.id }, body: { player_id: t.cur === host ? bots[1].id : t.cur.id } }, 'retirer la photo d\'un autre')
                await fillerQuestion(t)
            },
        },
        {
            name: 'joueur déconnecté à son tour → l\'hôte passe son tour',
            when: (t) => t.cur !== host,
            run: async (t) => {
                const away = t.cur
                await ctx.refused(409, 'POST', '/games/:game/skip-turn', { params: { game: gameId }, body: { player_id: host.id } }, 'passer le tour d\'un joueur encore connecté')
                const notHost = nextAlive(t, away, [host])
                if (notHost) await ctx.refused(409, 'POST', '/games/:game/skip-turn', { params: { game: gameId }, body: { player_id: notHost.id } }, 'passer un tour sans être l\'hôte')
                const gone = freshPresence(host, 'presence:leaving', away.id)
                away.disconnect()
                await gone
                const mk = marks(watchers())
                const res = await api.post('/games/:game/skip-turn', { params: { game: gameId }, body: { player_id: host.id }, expect: 200 })
                ctx.check(res.data.reason === 'turn', 'skip-turn : reason = turn')
                const [ts] = await everyone(watchers(), gameCh, 'turn.skipped', { since: mk })
                ctx.check(ts.data.player.id === away.id && ts.data.action_id === null, 'turn.skipped diffusé, aucune action enregistrée')
                await turnEnded(mk)
                const back = freshPresence(host, 'presence:joining', away.id)
                await away.reconnect()
                await back
                ctx.check(true, 'reconnexion : réabonnement et presence:joining')
            },
        },
        {
            name: 'cible déconnectée d\'une question → passage refusé, elle revient répondre',
            when: (t) => t.cur !== host && nextAlive(t, t.cur, [host]),
            run: async (t) => {
                const target = nextAlive(t, t.cur, [host])
                const id = await ask(t, target)
                const gone = freshPresence(host, 'presence:leaving', target.id)
                target.disconnect()
                await gone
                await ctx.refused(409, 'POST', '/games/:game/skip-turn', { params: { game: gameId }, body: { player_id: host.id } }, 'passer le tour d\'une cible hors ligne qui doit répondre')
                await target.reconnect()
                const { mk } = await answer(t, id, target, 'yes')
                await turnEnded(mk)
            },
        },
        {
            name: 'accusé déconnecté → l\'hôte passe, le serveur tranche',
            when: (t) => t.cur !== host && nextAlive(t, t.cur, [host]),
            run: async (t) => {
                const target = nextAlive(t, t.cur, [host])
                const { actionId } = await accuse(t, target, true)
                const gone = freshPresence(host, 'presence:leaving', target.id)
                target.disconnect()
                await gone
                const mk = marks(watchers())
                const res = await api.post('/games/:game/skip-turn', { params: { game: gameId }, body: { player_id: host.id }, expect: 200 })
                ctx.check(res.data.reason === 'accusation', 'skip-turn : reason = accusation')
                const [ac] = await everyone(watchers(), gameCh, 'accusation.confirmed', { pred: (d) => d.action_id === actionId, since: mk })
                ctx.check(ac.data.accusation_correct === true, 'le serveur tranche : nom exact → accusation juste')
                await everyone(watchers(), gameCh, 'player.eliminated', { pred: (d) => d.eliminated_player.id === target.id, since: mk })
                await turnEnded(mk)
                await target.reconnect()
            },
        },
        {
            name: 'exclusion d\'un joueur déconnecté (et de son binôme)',
            when: (t) => t.alive.some((b) => b !== host && b !== t.cur),
            run: async (t) => {
                const victim = t.alive.filter((b) => b !== host && b !== t.cur).sort((a, b) => b.id - a.id)[0]
                await ctx.refused(409, 'POST', '/games/:game/players/:player/exclude', { params: { game: gameId, player: victim.id }, body: { player_id: host.id } }, 'exclure un joueur connecté')
                const gone = freshPresence(host, 'presence:leaving', victim.id)
                victim.disconnect()
                await gone
                const someone = t.alive.find((b) => b !== host && b !== victim)
                await ctx.refused(409, 'POST', '/games/:game/players/:player/exclude', { params: { game: gameId, player: victim.id }, body: { player_id: someone.id } }, 'exclure sans être l\'hôte')
                await ctx.refused(409, 'POST', '/games/:game/players/:player/exclude', { params: { game: gameId, player: host.id }, body: { player_id: host.id } }, 'l\'hôte s\'exclut lui-même')
                const mk = marks(watchers())
                await api.post('/games/:game/players/:player/exclude', { params: { game: gameId, player: victim.id }, body: { player_id: host.id }, expect: 200 })
                const [ex] = await everyone(watchers(), gameCh, 'player.excluded', { since: mk })
                const partnerId = truth.get(victim.id).partnerId
                const partnerAlive = partnerId && t.alive.some((b) => b.id === partnerId)
                ctx.check(ex.data.player.id === victim.id && (ex.data.partner?.id ?? null) === (partnerAlive ? partnerId : null), 'player.excluded : exclu + binôme encore en jeu')
                const s = await gameState(ctx, gameId)
                const v = s.players.find((p) => p.id === victim.id)
                ctx.check(v.is_eliminated && v.is_excluded, 'GET /games : exclu marqué is_eliminated + is_excluded')
                notes.excludedId = victim.id
            },
        },
    ]

    // ── BOUCLE DE JEU ───────────────────────────────────────────────────────
    let turns = 0
    for (;;) {
        const t = await turn()
        if (!t) break
        if (++turns > 200) throw new Error('partie interminable')
        const step = steps[0]
        if (step && step.when(t)) {
            if (ctx.verbose) log(`étape : ${step.name}`)
            await step.run(t)
            steps.shift()
        } else if (steps.length) {
            await fillerQuestion(t)
        } else {
            await correctAccusation(t)
        }
    }
    ctx.check(steps.length === 0, 'toutes les étapes ciblées ont été jouées', steps.map((s) => s.name))
    log(`partie terminée en ${turns} tours`)

    // ── FIN DE PARTIE ───────────────────────────────────────────────────────
    const final = await gameState(ctx, gameId)
    const ended = watchers().map((b) => b.rt.received('game.ended', { channel: gameCh })[0])
    ctx.check(ended.every(Boolean), 'game.ended reçu par tous les connectés')
    const ge = ended[0].data
    const alive = final.players.filter((p) => !p.is_eliminated).map((p) => p.id).sort()
    ctx.check(JSON.stringify(ge.winners.map((w) => w.id).sort()) === JSON.stringify(alive), 'gagnants = joueurs encore en jeu')
    ctx.check(ge.all_binomes.length === Math.ceil(size / 2) && ge.all_binomes.filter((b) => b.is_orphan).length === size % 2, 'game.ended : révélation complète, orphelin signalé')
    ctx.check(final.current_round === null, 'partie terminée : plus de round en cours')
    ctx.check(ge.stats.every((s, i, a) => i === 0 || a[i - 1].score >= s.score), 'game.ended : stats triées par score décroissant')

    const recap = (await api.get('/games/:game/recap', { params: { game: gameId }, expect: 200 })).data
    for (const st of recap.stats) {
        const p = bots.find((b) => b.pseudo === st.player_pseudo)
        const info = truth.get(p.id)
        const partnerAliveAtEnd = info.partnerId && final.players.some((x) => x.id === info.partnerId && !x.is_eliminated)
        const bonus = st.is_winner && !st.is_eliminated ? (info.isOrphan || partnerAliveAtEnd ? 5 : 3) : 0
        const expected = st.is_excluded ? 0 : st.eliminations + st.rounds_survived + st.best_questions + bonus
        ctx.check(st.score === expected, `score ${p.name} = éliminations + rounds survécus + meilleures questions + bonus (${bonus})`, { st, expected })
    }
    if (notes.bestQuestionAuthor) {
        const author = bots.find((b) => b.id === notes.bestQuestionAuthor)
        ctx.check(recap.stats.find((s) => s.player_pseudo === author.pseudo)?.best_questions >= 1 && recap.best_questions.some((q) => q.round === 1), 'meilleure question du round 1 : +1 pour son auteur')
    }
    if (notes.excludedId) {
        const ex = bots.find((b) => b.id === notes.excludedId)
        ctx.check(recap.exclusions.some((e) => e.pseudo === ex.pseudo) && recap.stats.find((s) => s.player_pseudo === ex.pseudo).score === 0, 'récap : exclusion listée, exclu à 0 point')
    }

    const anyRound = final.actions.at(-1).round_id
    await ctx.refused(409, 'POST', '/games/:game/rounds/:round/question', { params: { game: gameId, round: anyRound }, body: { player_id: host.id, target_player_id: guests[0].id, question: 'Encore une question ?' } }, 'jouer après la fin')
    await ctx.refused(404, 'POST', '/games/:game/rounds/:round/question', { params: { game: gameId + 100000, round: anyRound }, body: { player_id: host.id, target_player_id: guests[0].id, question: 'Mauvaise partie ?' } }, 'round d\'une autre partie (scopeBindings)')

    // ── HISTORIQUE, CLASSEMENT, ADMIN ───────────────────────────────────────
    await ctx.refused(401, 'GET', '/history/games', {}, 'historique sans profil connecté')
    const hist = (await api.get('/history/games', { headers: asHost(), expect: 200 })).data.games
    ctx.check(hist.some((g) => g.id === gameId && g.status === 'finished') && hist.every((g) => g.status === 'finished'), 'historique public : parties terminées uniquement')
    const detail = (await api.get('/history/games/:game', { headers: asHost(), params: { game: gameId }, expect: 200 })).data
    ctx.check(detail.binomes.length === Math.ceil(size / 2) && detail.game_stats.length === size, 'historique : détail complet de la partie')
    ctx.check(Array.isArray(recap.awards) && recap.rounds_count === detail.rounds.length, 'récap : trophées + nombre de rounds')
    const board = (await api.get('/history/leaderboard', { headers: asHost(), expect: 200 })).data.leaderboard
    ctx.check(board.some((r) => r.pseudo === host.pseudo), 'classement : le pseudo apparaît')
    ctx.check(board.every((r, i) => i === 0 || r.rank === (r.total_score === board[i - 1].total_score ? board[i - 1].rank : i + 1)), 'classement : rangs ex æquo (1, 2, 2, 4)')
    const mine = (await api.get('/history/players', { headers: asHost(), query: { pseudo: host.pseudo.toUpperCase() }, expect: 200 })).data
    ctx.check(mine.games.some((g) => g.game_id === gameId), 'fiche joueur : recherche insensible à la casse')
    await ctx.refused(404, 'GET', '/history/players', { headers: asHost(), query: { pseudo: ctx.pseudo('inconnu') } }, 'fiche d\'un pseudo sans partie')
    await ctx.refused(422, 'GET', '/history/players', { headers: asHost() }, 'fiche sans pseudo')
    const adminGames = (await api.get('/admin/games', { admin: true, expect: 200 })).data.games
    ctx.check(adminGames.some((g) => g.id === gameId), 'admin : la partie est listée')

    // Tout le monde quitte le salon
    const everyoneInRoom = [...bots, ...(spectator ? [spectator] : [])]
    for (const [i, b] of everyoneInRoom.entries()) {
        const res = await api.delete('/rooms/:room/leave', { params: { room: room.id }, body: { player_id: b.id }, expect: 200 })
        if (i === everyoneInRoom.length - 1) ctx.check(/supprimé/.test(res.data.message), 'dernier départ : salon supprimé')
    }
    await ctx.refused(404, 'GET', '/rooms/:room', { params: { room: room.id } }, 'salon supprimé')
    // La partie survit au salon : historique et classement intacts
    const kept = await api.get('/history/games/:game', { headers: asHost(), params: { game: gameId } })
    ctx.check(kept.status === 200 && kept.data.room_code === room.code && kept.data.game_stats.length === size, 'salon supprimé : la partie reste dans l\'historique, avec le code de son salon')
    const listed = (await api.get('/history/games', { headers: asHost(), expect: 200 })).data.games.find((g) => g.id === gameId)
    ctx.check(listed && listed.room_id === null && listed.room_code === room.code, 'historique : partie listée sans salon, code conservé')
    const stillRanked = (await api.get('/history/leaderboard', { headers: asHost(), expect: 200 })).data.leaderboard.some((r) => r.pseudo === host.pseudo)
    ctx.check(stillRanked, 'classement : les points de la partie sont conservés')
    for (const b of everyoneInRoom) b.rt?.close()
    return { gameId, roomId: room.id, players: bots.map((b) => ({ id: b.id, pseudo: b.pseudo })) }
}
