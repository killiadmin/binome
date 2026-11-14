// Droit à l'effacement (migration 9) : DELETE /profile par le joueur (mot de passe
// exigé s'il en a un), anonymisation par défaut côté administrateur. Un joueur qui
// a joué reste dans les parties sous « Joueur supprimé #000123 » ; sans partie, il
// disparaît. Jeton révoqué, pseudo libéré, plus listé en administration.

import { fullGame } from './full-game.mjs'

const tok = (t) => ({ 'X-Profile-Token': t })
const GHOST = /^Joueur supprimé #\d{6}$/

export async function erasure(ctx) {
    const { api } = ctx

    // ── Profil protégé, sans partie : supprimé ───────────────────────────────
    const first = await api.post('/profile/login', { body: { pseudo: ctx.pseudo('eff') }, expect: 200 })
    ctx.created.players.add(first.data.player.id)
    const pseudo = first.data.player.pseudo
    const secured = await api.put('/profile/password', { body: { password: 'secret-e2e' }, headers: tok(first.data.token), expect: 200 })
    const A = { id: first.data.player.id, token: secured.data.token }
    const idea = (await api.post('/ideas', { body: { player_id: A.id, body: 'Une idée postée avant effacement e2e' }, headers: tok(A.token), expect: 201 })).data.idea

    await ctx.refused(401, 'DELETE', '/profile', { auth: false }, 'effacement sans jeton')
    await ctx.refused(422, 'DELETE', '/profile', { headers: tok(A.token) }, 'effacement sans le mot de passe')
    await ctx.refused(422, 'DELETE', '/profile', { body: { current_password: 'mauvais-mdp' }, headers: tok(A.token) }, 'effacement avec un mauvais mot de passe')
    const erased = await api.delete('/profile', { body: { current_password: 'secret-e2e' }, headers: tok(A.token), expect: 200 })
    ctx.check(/supprimé/.test(erased.data.message), 'effacement : profil supprimé', erased.data)
    await ctx.refused(401, 'GET', '/profile', { headers: tok(A.token) }, 'effacement : jeton révoqué')
    const kept = (await api.get('/ideas', { expect: 200 })).data.ideas.find((i) => i.id === idea.id)
    ctx.check(kept && GHOST.test(kept.author_name), 'effacement : son idée reste, sous « Joueur supprimé »', kept)
    const again = await api.post('/profile/login', { body: { pseudo }, expect: 200 })
    ctx.created.players.add(again.data.player.id)
    ctx.check(again.data.player.id !== A.id && again.data.player.has_password === false, 'effacement : pseudo libéré (nouveau profil public)', again.data.player)

    // ── Joueurs ayant joué : anonymisés, parties intactes ────────────────────
    const { gameId, players } = await fullGame(ctx, { size: 4, rich: false })
    const [, self, byAdmin] = players
    const before = (await api.get('/admin/games/:game', { params: { game: gameId }, admin: true, expect: 200 })).data
    await api.delete('/profile', { headers: tok(api.tokens.get(self.id)), expect: 200 })
    const adminRes = await api.delete('/admin/players/:player', { params: { player: byAdmin.id }, admin: true, expect: 200 })
    ctx.check(/anonymisé/.test(adminRes.data.message), 'admin : suppression par défaut = anonymisation', adminRes.data)
    await ctx.refused(409, 'DELETE', '/admin/players/:player', { params: { player: byAdmin.id }, admin: true }, 'admin : joueur déjà anonymisé')

    const after = (await api.get('/admin/games/:game', { params: { game: gameId }, admin: true, expect: 200 })).data
    const names = after.game_stats.map((s) => s.player_pseudo)
    ctx.check(after.game_stats.length === before.game_stats.length, 'anonymisation : la partie garde toutes ses lignes de score')
    ctx.check(!names.includes(self.pseudo) && !names.includes(byAdmin.pseudo) && names.filter((n) => GHOST.test(n)).length === 2,
        'anonymisation : les deux pseudos remplacés dans les scores', names)
    const seated = after.binomes.flatMap((b) => b.players)
    ctx.check(seated.filter((p) => GHOST.test(p.pseudo)).length === 2 && seated.every((p) => p.pseudo !== self.pseudo),
        'anonymisation : pseudos remplacés dans les binômes', seated.map((p) => p.pseudo))
    const listed = (await api.get('/admin/players', { admin: true, expect: 200 })).data.players
    ctx.check(!listed.some((p) => p.id === self.id || p.id === byAdmin.id), 'anonymisation : plus listés en administration')
    await ctx.refused(422, 'POST', '/profile/login', { body: { pseudo: after.game_stats.find((s) => GHOST.test(s.player_pseudo)).player_pseudo } },
        'pseudo anonyme impossible à saisir (plus de 20 caractères)')
}
