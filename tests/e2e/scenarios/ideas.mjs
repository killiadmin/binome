// Boîte à idées (migration 8) : lecture libre ; poster, voter (+1 par joueur,
// retirable, jamais sur sa propre idée) et supprimer la sienne exigent le jeton
// de profil ; 5 idées par 24 h, insultes refusées ; un administrateur met en
// favori (en tête de liste) et supprime.

const tok = (t) => ({ 'X-Profile-Token': t })

export async function ideas(ctx) {
    const { api } = ctx
    const login = async (name) => {
        const r = await api.post('/profile/login', { body: { pseudo: ctx.pseudo(name) }, expect: 200 })
        ctx.created.players.add(r.data.player.id)
        return { id: r.data.player.id, token: r.data.token }
    }
    const A = await login('ida')
    const B = await login('idb')
    const post = (who, body, expect = 201) => api.post('/ideas', { body: { player_id: who.id, body }, headers: tok(who.token), expect })
    const list = async (who) => (await api.get('/ideas', { headers: who ? tok(who.token) : {}, expect: 200 })).data
    const find = (data, id) => data.ideas.find((i) => i.id === id)
    const vote = (who, id, on, expect = 200) =>
        api[on ? 'put' : 'delete']('/ideas/:idea/vote', { params: { idea: id }, body: { player_id: who.id }, headers: tok(who.token), expect })

    // ── Poster ───────────────────────────────────────────────────────────────
    await ctx.refused(401, 'POST', '/ideas', { body: { player_id: A.id, body: 'Une idée sans jeton de profil' }, auth: false }, 'poster sans jeton')
    await ctx.refused(403, 'POST', '/ideas', { body: { player_id: A.id, body: 'Une idée au nom d\'un autre joueur' }, headers: tok(B.token) }, 'poster au nom d\'un autre')
    await ctx.refused(422, 'POST', '/ideas', { body: { player_id: A.id, body: 'court' }, headers: tok(A.token) }, 'idée trop courte')
    await ctx.refused(422, 'POST', '/ideas', { body: { player_id: A.id, body: 'x'.repeat(501) }, headers: tok(A.token) }, 'idée trop longue')
    await ctx.refused(422, 'POST', '/ideas', { body: { player_id: A.id, body: 'Le créateur du salon est un c0nnnard' }, headers: tok(A.token) }, 'insulte refusée')

    const first = (await post(A, 'Ajouter un mode contre-la-montre e2e')).data.idea
    ctx.check(first.votes === 0 && first.is_mine === true && first.is_pinned === false && first.author_name, 'idée publiée', first)
    const second = (await post(A, 'Un concours mensuel avec classement e2e')).data.idea

    const anon = await list(null)
    ctx.check(anon.remaining_today === null && find(anon, first.id)?.has_voted === false && find(anon, first.id)?.is_mine === false, 'lecture anonyme', anon)
    ctx.check((await list(A)).remaining_today === 3, 'quota : 3 idées restantes après 2')

    // ── Votes ────────────────────────────────────────────────────────────────
    await ctx.refused(409, 'PUT', '/ideas/:idea/vote', { params: { idea: first.id }, body: { player_id: A.id }, headers: tok(A.token) }, 'voter pour sa propre idée')
    await ctx.refused(401, 'PUT', '/ideas/:idea/vote', { params: { idea: first.id }, body: { player_id: B.id }, auth: false }, 'voter sans jeton')
    await ctx.refused(404, 'PUT', '/ideas/:idea/vote', { params: { idea: 999999999 }, body: { player_id: B.id }, headers: tok(B.token) }, 'voter pour une idée inexistante')
    const voted = (await vote(B, second.id, true)).data.idea
    ctx.check(voted.votes === 1 && voted.has_voted === true, 'vote enregistré', voted)
    ctx.check((await vote(B, second.id, true)).data.idea.votes === 1, 'second vote : toujours +1')
    let byB = await list(B)
    const order = byB.ideas.map((i) => i.id)
    ctx.check(order.indexOf(second.id) < order.indexOf(first.id), 'la plus votée avant l\'autre')
    ctx.check((await vote(B, second.id, false)).data.idea.votes === 0, 'vote retiré')
    ctx.check((await vote(B, second.id, false)).data.idea.votes === 0, 'retrait répété sans effet')

    // ── Administration ───────────────────────────────────────────────────────
    await ctx.refused(403, 'PATCH', '/admin/ideas/:idea', { params: { idea: first.id }, body: { is_pinned: true }, headers: tok(B.token) }, 'favori sans être admin')
    await ctx.refused(422, 'PATCH', '/admin/ideas/:idea', { params: { idea: first.id }, body: {}, admin: true }, 'favori sans is_pinned')
    const pinned = (await api.patch('/admin/ideas/:idea', { params: { idea: first.id }, body: { is_pinned: true }, admin: true, expect: 200 })).data.idea
    ctx.check(pinned.is_pinned === true && pinned.pinned_at, 'idée en favori', pinned)
    await vote(B, second.id, true)
    byB = await list(B)
    ctx.check(byB.ideas[0]?.id === first.id, 'le favori passe en tête malgré les votes', byB.ideas.slice(0, 3))
    const unpinned = (await api.patch('/admin/ideas/:idea', { params: { idea: first.id }, body: { is_pinned: false }, admin: true, expect: 200 })).data.idea
    ctx.check(unpinned.is_pinned === false, 'favori retiré')

    // ── Suppressions ─────────────────────────────────────────────────────────
    await ctx.refused(403, 'DELETE', '/ideas/:idea', { params: { idea: first.id }, body: { player_id: B.id }, headers: tok(B.token) }, 'supprimer l\'idée d\'un autre')
    await api.delete('/ideas/:idea', { params: { idea: first.id }, body: { player_id: A.id }, headers: tok(A.token), expect: 200 })
    await api.delete('/admin/ideas/:idea', { params: { idea: second.id }, admin: true, expect: 200 })
    await ctx.refused(404, 'DELETE', '/admin/ideas/:idea', { params: { idea: second.id }, admin: true }, 'idée déjà supprimée')
    const after = await list(A)
    ctx.check(!find(after, first.id) && !find(after, second.id), 'idées supprimées absentes de la liste')

    // ── Quota de 5 par 24 h (les idées supprimées ne comptent plus) ──────────
    const made = []
    for (let n = 1; n <= 5; n++) made.push((await post(B, `Idée de quota numéro ${n} e2e`)).data.idea.id)
    await ctx.refused(422, 'POST', '/ideas', { body: { player_id: B.id, body: 'Une sixième idée de trop e2e' }, headers: tok(B.token) }, '6e idée en 24 h')
    for (const id of made) await api.delete('/admin/ideas/:idea', { params: { idea: id }, admin: true, expect: 200 })
}
