// Administration : accès par mot de passe, cosmos / univers / personnages,
// validation des propositions, emojis, joueurs.
//
// ⚠ Crée des cosmos et univers qu'aucune route ne permet de supprimer : à ne
// lancer que contre une base JETABLE (la pile de référence restaurée d'un dump).

import { fixtureBlob } from '../lib/http.mjs'

export async function catalog(ctx) {
    const { api } = ctx
    const name = (n) => `E2E ${ctx.tag} ${n}`

    // ── Accès ───────────────────────────────────────────────────────────────
    await ctx.refused(422, 'POST', '/access/characters', { body: { password: 'mauvais-mot-de-passe' } }, 'mot de passe admin incorrect')
    await ctx.refused(403, 'GET', '/characters', {}, 'liste des personnages sans jeton')
    await ctx.refused(403, 'GET', '/admin/games', { headers: { 'X-Characters-Access': 'faux' } }, 'admin avec un jeton faux')
    await ctx.loginAdmin()

    // ── Cosmos / univers ─────────────────────────────────────────────────────
    const cosmos = (await api.post('/cosmos', { body: { name: name('Cosmos') }, admin: true, expect: 201 })).data.cosmos
    await ctx.refused(422, 'POST', '/cosmos', { body: { name: name('Cosmos') }, admin: true }, 'cosmos en double')
    await ctx.refused(403, 'POST', '/cosmos', { body: { name: name('Cosmos 2') } }, 'créer un cosmos sans jeton')
    const universe = (await api.post('/universes', { body: { name: name('Univers'), cosmos_id: cosmos.id }, admin: true, expect: 201 })).data.universe
    ctx.check(universe.cosmos_name === cosmos.name, 'univers rattaché au cosmos')
    const patched = (await api.patch('/universes/:universe', { params: { universe: universe.id }, body: { cosmos_id: null }, admin: true, expect: 200 })).data.universe
    ctx.check(patched.cosmos_id === null, 'univers détaché de son cosmos')
    const universes = (await api.get('/universes', { admin: true, expect: 200 })).data.universes
    ctx.check(universes.some((u) => u.id === universe.id && u.characters_count === 0), 'GET /universes : compteur de personnages')

    // ── Personnages (multipart, comme le front) ─────────────────────────────
    const createCharacter = async (n, withImage = true) => {
        const form = new FormData()
        form.append('name', n)
        if (withImage) form.append('image', fixtureBlob('character.png', 'image/png'), 'character.png')
        for (const w of ['alpha', 'bravo', 'charlie']) form.append('forbidden_words[]', w)
        form.append('level_affectation', '1')
        const res = await api.post('/universes/:universe/characters', { params: { universe: universe.id }, form, admin: true, expect: 201 })
        ctx.created.characters.add(res.data.character.id)
        return res.data.character
    }
    const a = await createCharacter('Élodie Test')
    const b = await createCharacter('Élodie Test', false) // même nom → slug unique « -2 »
    ctx.check(a.image?.startsWith('/binome/api/character-images/') && b.image === null, 'création : URL d\'image si fichier, null sinon')

    const img = await api.get('/character-images/:character', { params: { character: a.id }, raw: true, expect: 200 })
    ctx.check(img.data.contentType.startsWith('image/png') && /immutable/.test(img.data.cacheControl ?? ''), 'image de personnage servie avec son type, cache immutable')
    await ctx.refused(404, 'GET', '/character-images/:character', { params: { character: b.id } }, 'image d\'un personnage sans image')

    const badForm = new FormData()
    badForm.append('name', 'X')
    await ctx.refused(422, 'POST', '/universes/:universe/characters', { params: { universe: universe.id }, form: badForm, admin: true }, 'personnage invalide (nom court, mots manquants)')

    const words = await api.patch('/characters/:character/forbidden-words', { params: { character: a.id }, body: { forbidden_words: ['delta', 'echo', 'foxtrot'] }, admin: true, expect: 200 })
    ctx.check(JSON.stringify(words.data.character.forbidden_words) === '["delta","echo","foxtrot"]', 'mots interdits modifiés')
    await ctx.refused(422, 'PATCH', '/characters/:character/forbidden-words', { params: { character: a.id }, body: { forbidden_words: ['un', 'deux'] }, admin: true }, 'il faut exactement 3 mots interdits')

    // Modification : POST + _method=PUT (multipart), comme characterService.updateCharacter
    const upd = new FormData()
    upd.append('_method', 'PUT')
    upd.append('name', 'Élodie Renommée')
    for (const w of ['golf', 'hotel', 'india']) upd.append('forbidden_words[]', w)
    upd.append('level_affectation', '1')
    upd.append('status', 'pending')
    const updated = (await api.post('/characters/:character', { params: { character: a.id }, form: upd, admin: true, expect: 200 })).data.character
    ctx.check(updated.name === 'Élodie Renommée' && updated.verif_manual === false && updated.hidden === false, 'modification (POST + _method=PUT) : statut « à valider »')

    const upload = new FormData()
    upload.append('image', fixtureBlob('character.png', 'image/png'), 'c.png')
    const up = await api.post('/characters/:character/image', { params: { character: b.id }, form: upload, admin: true, expect: 200 })
    ctx.check(up.data.character.image?.startsWith('/binome/api/character-images/'), 'import d\'image sur un personnage existant')
    // b a été créé validé (fiche) : on le repasse « à valider » pour former un binôme en attente
    const toPending = new FormData()
    toPending.append('_method', 'PUT')
    toPending.append('name', 'Élodie Test')
    for (const w of ['juliet', 'kilo', 'lima']) toPending.append('forbidden_words[]', w)
    toPending.append('level_affectation', '1')
    toPending.append('status', 'pending')
    await api.post('/characters/:character', { params: { character: b.id }, form: toPending, admin: true, expect: 200 })
    // Vraie route PUT en JSON (sans fichier), pour le contrat
    const put = await api.put('/characters/:character', { params: { character: b.id }, body: { name: 'Élodie Test', forbidden_words: ['juliet', 'kilo', 'lima'], level_affectation: 1 }, admin: true, expect: 200 })
    ctx.check(put.data.character.verif_manual === false, 'PUT JSON sans statut : l\'état de relecture est conservé')

    const pending = (await api.get('/characters/pending', { admin: true, expect: 200 })).data.binomes
    const group = pending.find((p) => p.key === `${universe.id}-1`)
    ctx.check(group && group.characters.length === 2, 'binôme en attente groupé par univers + niveau')

    const ids = [a.id, b.id]
    const rej = await api.post('/characters/validation/reject', { body: { character_ids: ids }, admin: true, expect: 200 })
    ctx.check(rej.data.rejected_ids.length === 2, 'binôme refusé (hidden)')
    await ctx.refused(422, 'POST', '/characters/validation/accept', { body: { character_ids: ids }, admin: true }, 'accepter un binôme refusé (plus en attente)')
    const res = await api.post('/characters/validation/restore', { body: { character_ids: ids }, admin: true, expect: 200 })
    ctx.check(res.data.restored_ids.length === 2, 'binôme remis à valider')
    const acc = await api.post('/characters/validation/accept', { body: { character_ids: ids }, admin: true, expect: 200 })
    ctx.check(acc.data.validated_ids.length === 2, 'binôme validé')
    const all = (await api.get('/characters', { admin: true, expect: 200 })).data.characters
    ctx.check(all.filter((x) => ids.includes(x.id)).every((x) => x.verif_manual && !x.hidden && x.universe_name === universe.name), 'GET /characters : état validé')
    const cosmosList = (await api.get('/cosmos', { expect: 200 })).data.cosmos
    ctx.check(cosmosList.some((x) => x.id === cosmos.id), 'GET /cosmos ouvert sans jeton')

    for (const id of ids) {
        await api.delete('/characters/:character', { params: { character: id }, admin: true, expect: 200 })
        ctx.created.characters.delete(id)
    }

    // ── Emojis (admin) ──────────────────────────────────────────────────────
    const existing = new Set((await api.get('/admin/reactions', { admin: true, expect: 200 })).data.reactions.map((r) => r.emoji))
    const emoji = ['🦩', '🪼', '🦦', '🪿', '🦥', '🫎', '🦭', '🪲'].find((e) => !existing.has(e))
    const created = (await api.post('/admin/reactions', { body: { name: 'E2E', emoji }, admin: true, expect: 201 })).data.reaction
    ctx.created.reactions.add(created.id)
    ctx.check(created.emoji === emoji && created.url === null && created.is_enabled === true, 'admin : emoji Unicode ajouté')
    await ctx.refused(422, 'POST', '/admin/reactions', { body: { name: 'Doublon', emoji }, admin: true }, 'emoji Unicode en double')
    await ctx.refused(422, 'POST', '/admin/reactions', { body: { name: 'Lettre', emoji: 'A' }, admin: true }, 'pas un emoji')
    // (l'ancien backend acceptait deux emojis : corrigé lors de la migration)
    const twin = await ctx.refused(422, 'POST', '/admin/reactions', { body: { name: 'Deux', emoji: `${emoji}${emoji}` }, admin: true }, 'deux emojis')
    if (twin.status === 201) ctx.created.reactions.add(twin.data.reaction.id)
    const off = (await api.patch('/admin/reactions/:reaction', { params: { reaction: created.id }, body: { is_enabled: false, name: 'E2E off' }, admin: true, expect: 200 })).data.reaction
    ctx.check(off.is_enabled === false && off.name === 'E2E off', 'admin : emoji masqué et renommé')
    const visible = (await api.get('/reactions', { expect: 200 })).data.reactions
    ctx.check(!visible.some((r) => r.id === created.id), 'emoji masqué absent de la barre')
    await api.delete('/admin/reactions/:reaction', { params: { reaction: created.id }, admin: true, expect: 200 })
    ctx.created.reactions.delete(created.id)

    // ── Joueurs (admin) ─────────────────────────────────────────────────────
    const lonely = ctx.pseudo('adm')
    const room = (await api.post('/rooms', { body: { pseudo: lonely }, expect: 201 })).data
    ctx.created.players.add(room.player.id)
    const players = (await api.get('/admin/players', { admin: true, expect: 200 })).data.players
    ctx.check(players.some((p) => p.id === room.player.id && p.room_code === room.room.code && p.in_game === false), 'admin : joueur listé avec son salon')
    await api.delete('/admin/players/:player/room', { params: { player: room.player.id }, admin: true, expect: 200 })
    await ctx.refused(409, 'DELETE', '/admin/players/:player/room', { params: { player: room.player.id }, admin: true }, 'retirer d\'un salon un joueur qui n\'en a pas')
    await ctx.refused(403, 'DELETE', '/admin/players/:player', { params: { player: room.player.id }, query: { with_games: 1 }, admin: true }, 'supprimer un joueur avec ses parties sans le mot de passe de suppression')
    await api.delete('/admin/players/:player', { params: { player: room.player.id }, admin: true, expect: 200 })
    await ctx.refused(404, 'DELETE', '/admin/players/:player', { params: { player: room.player.id }, admin: true }, 'joueur déjà supprimé')
}
