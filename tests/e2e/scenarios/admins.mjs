// Administrateurs désignés (migration 7) : le propriétaire (jeton X-Characters-Access)
// désigne un joueur au profil sécurisé, qui accède à l'administration avec son jeton
// de profil mais ne peut ni désigner d'administrateur ni en supprimer un.

const tok = (t) => ({ 'X-Profile-Token': t })

export async function admins(ctx) {
    const { api } = ctx
    const route = '/admin/players/:player/admin'
    const grant = (id, admin) => api.patch(route, { params: { player: id }, body: { admin }, admin: true, expect: 200 })

    // Deux profils : D (sécurisé, futur administrateur) et P (public)
    const first = await api.post('/profile/login', { body: { pseudo: ctx.pseudo('adm') }, expect: 200 })
    ctx.created.players.add(first.data.player.id)
    ctx.check(first.data.player.admin_role === null, 'nouveau profil : pas administrateur')
    const secured = await api.put('/profile/password', { body: { password: 'secret-e2e' }, headers: tok(first.data.token), expect: 200 })
    const D = { id: first.data.player.id, token: secured.data.token }
    const pub = await api.post('/profile/login', { body: { pseudo: ctx.pseudo('pub') }, expect: 200 })
    ctx.created.players.add(pub.data.player.id)
    const P = { id: pub.data.player.id, token: pub.data.token }

    // ── Désignation : propriétaire seulement, profil sécurisé ────────────────
    await ctx.refused(403, 'GET', '/admin/players', { headers: tok(D.token) }, 'administration avec le jeton d\'un joueur non désigné')
    await ctx.refused(403, 'PATCH', route, { params: { player: D.id }, body: { admin: true }, headers: tok(D.token) }, 'se désigner soi-même')
    await ctx.refused(409, 'PATCH', route, { params: { player: P.id }, body: { admin: true }, admin: true }, 'désigner un profil public')
    await ctx.refused(404, 'PATCH', route, { params: { player: 999999999 }, body: { admin: true }, admin: true }, 'désigner un joueur inexistant')
    await ctx.refused(422, 'PATCH', route, { params: { player: D.id }, body: {}, admin: true }, 'désignation sans le champ admin')

    const granted = await grant(D.id, true)
    ctx.check(granted.data.player.admin_role === 'admin' && granted.data.player.is_validated === true, 'désigné : administrateur et validé d\'office', granted.data.player)

    // ── Accès par le jeton de profil ─────────────────────────────────────────
    const listed = (await api.get('/admin/players', { headers: tok(D.token), expect: 200 })).data.players
    const me = listed.find((p) => p.id === D.id)
    ctx.check(me?.admin_role === 'admin' && me.is_validated === true, 'admin joueurs : rôle admin listé', me)
    ctx.check(listed.find((p) => p.id === P.id)?.admin_role === null, 'admin joueurs : un joueur ordinaire sans rôle')
    ctx.check((await api.get('/profile', { headers: tok(D.token), expect: 200 })).data.player.admin_role === 'admin', 'GET /profile : admin_role')
    await api.get('/admin/settings', { headers: tok(D.token), expect: 200 })
    await api.get('/history/leaderboard', { headers: tok(D.token), expect: 200 })
    await api.patch('/admin/players/:player/validation', { params: { player: P.id }, body: { validated: true }, headers: tok(D.token), expect: 200 })
    await ctx.refused(403, 'GET', '/admin/players', { headers: tok(first.data.token) }, 'jeton révoqué par le mot de passe : pas d\'administration')

    // ── Limites du désigné ───────────────────────────────────────────────────
    await ctx.refused(403, 'PATCH', route, { params: { player: P.id }, body: { admin: true }, headers: tok(D.token) }, 'un désigné ne désigne pas d\'administrateur')
    await ctx.refused(403, 'PATCH', route, { params: { player: D.id }, body: { admin: false }, headers: tok(D.token) }, 'un désigné ne retire pas de rôle')
    await ctx.refused(403, 'DELETE', '/admin/players/:player', { params: { player: D.id }, headers: tok(D.token) }, 'un désigné ne supprime pas un administrateur')
    await ctx.refused(409, 'PATCH', '/admin/players/:player/validation', { params: { player: D.id }, body: { validated: false }, admin: true }, 'un administrateur reste validé')
    await ctx.refused(409, 'DELETE', '/profile/password', { body: { current_password: 'secret-e2e' }, headers: tok(D.token) }, 'un administrateur garde son mot de passe')

    // ── Retrait : accès coupé aussitôt ───────────────────────────────────────
    const revoked = await grant(D.id, false)
    ctx.check(revoked.data.player.admin_role === null && revoked.data.player.is_validated === false, 'retrait : plus administrateur ni validé', revoked.data.player)
    await ctx.refused(403, 'GET', '/admin/players', { headers: tok(D.token) }, 'retrait : administration refermée')
    await api.delete('/profile/password', { body: { current_password: 'secret-e2e' }, headers: tok(D.token), expect: 200 })
}
