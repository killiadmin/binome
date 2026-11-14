// Contrôles de l'administrateur (migration 6) : modes de jeu et cosmos proposés,
// binômes écartés du tirage, validation manuelle des profils (historique, emojis).
// Le banc ouvre les deux modes au départ (Context.openGameModes) et les laisse ouverts.

import { jpegDataUri } from '../lib/http.mjs'

const tok = (t) => ({ 'X-Profile-Token': t })

export async function controls(ctx) {
    const { api } = ctx
    const setMode = (mode, enabled) => api.patch('/admin/settings/game-modes/:mode', { params: { mode }, body: { enabled }, admin: true, expect: 200 })
    const setCosmos = (id, enabled) => api.patch('/admin/cosmos/:cosmos', { params: { cosmos: id }, body: { is_enabled: enabled }, admin: true, expect: 200 })
    const publicCosmos = async () => (await api.get('/cosmos', { expect: 200 })).data

    // ── Réglages : accès et forme ────────────────────────────────────────────
    await ctx.refused(403, 'GET', '/admin/settings', {}, 'réglages du jeu sans jeton admin')
    const settings = (await api.get('/admin/settings', { admin: true, expect: 200 })).data
    ctx.check(settings.game_modes.map((m) => m.key).join() === 'random,cosmos' && settings.game_modes.every((m) => m.enabled), 'réglages : deux modes, ouverts par le banc', settings.game_modes)
    ctx.check(settings.cosmos.every((c) => typeof c.is_enabled === 'boolean') && settings.binomes.every((b) => b.characters.length >= 2), 'réglages : cosmos avec état, binômes complets')

    // ── Modes de jeu ────────────────────────────────────────────────────────
    await setMode('random', false)
    await ctx.refused(409, 'PATCH', '/admin/settings/game-modes/:mode', { params: { mode: 'cosmos' }, body: { enabled: false }, admin: true }, 'désactiver le dernier mode de jeu')
    await ctx.refused(404, 'PATCH', '/admin/settings/game-modes/:mode', { params: { mode: 'inconnu' }, body: { enabled: true }, admin: true }, 'mode de jeu inconnu')
    ctx.check((await publicCosmos()).game_modes.join() === 'cosmos', 'GET /cosmos : seuls les modes proposés')

    const pseudo = ctx.pseudo('ctl')
    const login = await api.post('/profile/login', { body: { pseudo }, expect: 200 })
    const pid = login.data.player.id
    const token = login.data.token
    ctx.created.players.add(pid)
    ctx.check(login.data.player.is_validated === false, 'nouveau profil : non validé')
    const room = (await api.post('/rooms', { body: { pseudo }, headers: tok(token), expect: 201 })).data.room
    const shown = (await api.get('/rooms/:room', { params: { room: room.id }, expect: 200 })).data.room
    ctx.check(shown.game_mode === 'cosmos', 'salon créé sans le mode Aléatoire : mode Cosmos imposé', shown.game_mode)
    await ctx.refused(422, 'PATCH', '/rooms/:room/settings', { params: { room: room.id }, body: { player_id: pid, game_mode: 'random' } }, 'choisir un mode désactivé')
    await setMode('random', true)

    // ── Cosmos ──────────────────────────────────────────────────────────────
    const target = settings.cosmos.find((c) => c.is_enabled && c.playable_universe_count > 0)
    if (ctx.check(Boolean(target), 'base de test : un cosmos activé et jouable')) {
        await setCosmos(target.id, false)
        ctx.check((await publicCosmos()).cosmos.find((c) => c.id === target.id).is_enabled === false, 'GET /cosmos : cosmos désactivé signalé')
        await ctx.refused(422, 'PATCH', '/rooms/:room/settings', { params: { room: room.id }, body: { player_id: pid, game_mode: 'cosmos', cosmos_id: target.id } }, 'choisir un cosmos désactivé')
        await setCosmos(target.id, true)
        await api.patch('/rooms/:room/settings', { params: { room: room.id }, body: { player_id: pid, game_mode: 'cosmos', cosmos_id: target.id }, expect: 200 })
        // Désactivé après coup : le garder pour régler les options reste permis (le lancement le refusera)
        await setCosmos(target.id, false)
        await api.patch('/rooms/:room/settings', { params: { room: room.id }, body: { player_id: pid, game_mode: 'cosmos', cosmos_id: target.id, reveal_to_eliminated: false }, expect: 200 })
        await setCosmos(target.id, true)

        // ── Binômes ─────────────────────────────────────────────────────────
        const binome = settings.binomes.find((b) => b.cosmos_id === target.id && b.is_enabled)
        if (ctx.check(Boolean(binome), 'base de test : un binôme actif dans ce cosmos')) {
            const before = (await publicCosmos()).cosmos.find((c) => c.id === target.id).playable_universe_count
            const sameUniverse = settings.binomes.some((b) => b.universe_id === binome.universe_id && b.level_affectation !== binome.level_affectation && b.is_enabled)
            const body = (on) => ({ universe_id: binome.universe_id, level_affectation: binome.level_affectation, is_enabled: on })
            await api.patch('/admin/binomes', { body: body(false), admin: true, expect: 200 })
            const off = (await api.get('/admin/settings', { admin: true, expect: 200 })).data.binomes
                .find((b) => b.universe_id === binome.universe_id && b.level_affectation === binome.level_affectation)
            ctx.check(off.is_enabled === false, 'binôme désactivé : listé comme tel')
            const after = (await publicCosmos()).cosmos.find((c) => c.id === target.id).playable_universe_count
            ctx.check(after === (sameUniverse ? before : before - 1), 'binôme désactivé : retiré des univers jouables', { before, after, sameUniverse })
            await api.patch('/admin/binomes', { body: body(true), admin: true, expect: 200 })
            ctx.check((await publicCosmos()).cosmos.find((c) => c.id === target.id).playable_universe_count === before, 'binôme réactivé : de nouveau jouable')
        }
    }
    await ctx.refused(422, 'PATCH', '/admin/binomes', { body: { universe_id: 999999999, level_affectation: 1, is_enabled: false }, admin: true }, 'binôme d\'un univers inexistant')

    // ── Validation des profils ──────────────────────────────────────────────
    await ctx.refused(401, 'GET', '/history/leaderboard', {}, 'historique sans profil')
    await ctx.refused(403, 'GET', '/history/leaderboard', { headers: tok(token) }, 'historique d\'un profil non validé')
    await ctx.refused(403, 'POST', '/reactions', { body: { player_id: pid, name: 'Non', image: jpegDataUri('square-64.jpg') } }, 'emoji d\'un profil non validé')
    ctx.check((await api.get('/reactions', { headers: tok(token), expect: 200 })).data.reactions.every((r) => r.emoji !== null), 'profil non validé : pas d\'emojis personnalisés')

    const validated = await api.patch('/admin/players/:player/validation', { params: { player: pid }, body: { validated: true }, admin: true, expect: 200 })
    ctx.check(validated.data.player.is_validated === true && typeof validated.data.player.validated_at === 'string', 'validation : profil validé et daté')
    ctx.check((await api.get('/profile', { headers: tok(token), expect: 200 })).data.player.is_validated === true, 'GET /profile : is_validated')
    const listed = (await api.get('/admin/players', { admin: true, expect: 200 })).data.players.find((p) => p.id === pid)
    ctx.check(listed.is_validated === true && listed.has_password === false, 'admin joueurs : validé, profil public')
    await api.get('/history/leaderboard', { headers: tok(token), expect: 200 })

    await api.patch('/admin/players/:player/validation', { params: { player: pid }, body: { validated: false }, admin: true, expect: 200 })
    await ctx.refused(403, 'GET', '/history/games', { headers: tok(token) }, 'validation retirée : historique refermé')
    await ctx.refused(404, 'PATCH', '/admin/players/:player/validation', { params: { player: 999999999 }, body: { validated: true }, admin: true }, 'valider un joueur inexistant')
    await ctx.refused(403, 'PATCH', '/admin/players/:player/validation', { params: { player: pid }, body: { validated: true } }, 'valider sans jeton admin')

    await api.delete('/rooms/:room/leave', { params: { room: room.id }, body: { player_id: pid }, expect: 200 })
}
