// Profils : connexion par pseudo, jeton de profil, mot de passe facultatif,
// connexion automatique en créant / rejoignant un salon, renommage, profil admin.

import { Bot } from '../lib/bots.mjs'
import { jpegDataUri } from '../lib/http.mjs'

const tok = (t) => ({ 'X-Profile-Token': t })

export async function profile(ctx) {
    const { api } = ctx
    const a = new Bot(ctx, 'pa')
    const b = new Bot(ctx, 'pb')

    // ── Connexion : pseudo inconnu → profil créé, public ─────────────────────
    const login = await api.post('/profile/login', { body: { pseudo: a.pseudo }, expect: 200 })
    a.setId(login.data.player.id)
    let token = login.data.token
    ctx.check(/^\d+\.[0-9a-f]{64}$/.test(token), 'connexion : jeton « id.hmac »')
    ctx.check(login.data.player.has_password === false && login.data.player.password_prompted === false && login.data.player.is_admin === false, 'nouveau profil : public, jamais sollicité, pas admin', login.data.player)
    ctx.check(login.data.current === null, 'nouveau profil : aucun salon en cours')
    await ctx.refused(422, 'POST', '/profile/login', { body: { pseudo: 'x' } }, 'connexion : pseudo trop court')

    // ── Page profil : jeton exigé ────────────────────────────────────────────
    await ctx.refused(401, 'GET', '/profile', {}, 'profil sans jeton')
    await ctx.refused(401, 'GET', '/profile', { headers: tok(`${a.id}.${'0'.repeat(64)}`) }, 'profil avec un jeton falsifié')
    const empty = await api.get('/profile', { headers: tok(token), expect: 200 })
    ctx.check(empty.data.games.length === 0 && empty.data.totals.games_played === 0, 'profil neuf : aucune partie')

    // ── Créer un salon connecté : profil et jeton renvoyés ───────────────────
    const created = await api.post('/rooms', { body: { pseudo: a.pseudo }, headers: tok(token), expect: 201 })
    ctx.check(created.data.player.id === a.id && created.data.profile?.id === a.id && typeof created.data.profile_token === 'string', 'création de salon : connexion automatique (profile + profile_token)')
    const room = created.data.room
    const withRoom = await api.get('/profile', { headers: tok(token), expect: 200 })
    ctx.check(withRoom.data.current?.room_id === room.id && withRoom.data.current?.room_code === room.code && withRoom.data.current?.host_id === a.id && withRoom.data.current?.game_id === null, 'profil : salon en cours', withRoom.data.current)

    // Proposition de mot de passe : une seule fois (drapeau en base)
    const prompted = await api.post('/profile/password-prompt', { headers: tok(token), expect: 200 })
    ctx.check(prompted.data.player.password_prompted === true, 'proposition de mot de passe marquée vue')

    await ctx.refused(409, 'PATCH', '/profile', { body: { pseudo: ctx.pseudo('pa2') }, headers: tok(token) }, 'renommage refusé pendant un salon')

    // ── Mot de passe posé : anciens jetons révoqués ──────────────────────────
    await ctx.refused(422, 'PUT', '/profile/password', { body: { password: '123' }, headers: tok(token) }, 'mot de passe trop court')
    const set = await api.put('/profile/password', { body: { password: 'secret-e2e' }, headers: tok(token), expect: 200 })
    ctx.check(set.data.player.has_password === true && set.data.token !== token, 'mot de passe posé : nouveau jeton')
    await ctx.refused(401, 'GET', '/profile', { headers: tok(token) }, 'ancien jeton révoqué par le mot de passe')
    token = set.data.token

    // ── Pseudo protégé : rejoindre exige le jeton ────────────────────────────
    const other = await api.post('/rooms', { body: { pseudo: b.pseudo }, expect: 201 })
    b.setId(other.data.player.id)
    ctx.check(other.data.profile?.has_password === false && other.data.profile?.password_prompted === false, 'arrivée sans connexion : profil public à protéger')
    await ctx.refused(403, 'POST', '/rooms/join', { body: { pseudo: a.pseudo, code: other.data.room.code } }, 'rejoindre sous un pseudo protégé sans jeton')
    await ctx.refused(403, 'POST', '/rooms/join', { body: { pseudo: a.pseudo.toUpperCase(), code: other.data.room.code } }, 'pseudo protégé : insensible à la casse')
    await ctx.refused(403, 'POST', '/rooms/join', { body: { pseudo: a.pseudo, code: other.data.room.code }, headers: tok(other.data.profile_token) }, 'rejoindre avec le jeton d\'un autre profil')
    await api.post('/rooms/join', { body: { pseudo: a.pseudo, code: other.data.room.code }, headers: tok(token), expect: 200 })

    // ── Photo d'un profil protégé : jeton exigé ──────────────────────────────
    const photo = { player_id: a.id, image: jpegDataUri('square-64.jpg') }
    await ctx.refused(403, 'POST', '/players/:player/avatar', { params: { player: a.id }, body: photo }, 'photo d\'un profil protégé sans jeton')
    const avatar = await api.post('/players/:player/avatar', { params: { player: a.id }, body: photo, headers: tok(token), expect: 200 })
    ctx.check(typeof avatar.data.avatar_url === 'string', 'photo d\'un profil protégé avec son jeton')
    // Profil public : inchangé (pas de jeton exigé)
    await api.post('/players/:player/avatar', { params: { player: b.id }, body: { player_id: b.id, image: jpegDataUri('square-64.jpg') }, expect: 200 })

    // ── Connexion à un profil protégé (autre navigateur) ─────────────────────
    const noPwd = await ctx.refused(422, 'POST', '/profile/login', { body: { pseudo: a.pseudo } }, 'profil protégé : mot de passe requis')
    ctx.check(Array.isArray(noPwd.data.errors?.password), 'profil protégé : erreur sur le champ password')
    await ctx.refused(422, 'POST', '/profile/login', { body: { pseudo: a.pseudo, password: 'mauvais' } }, 'profil protégé : mauvais mot de passe')
    const relog = await api.post('/profile/login', { body: { pseudo: a.pseudo, password: 'secret-e2e' }, expect: 200 })
    ctx.check(relog.data.player.id === a.id && relog.data.current?.room_id === other.data.room.id, 'reconnexion : salon le plus récent renvoyé', relog.data.current)
    ctx.check(relog.data.admin_token === undefined, 'profil joueur : pas de jeton admin')
    token = relog.data.token

    // Profil public connecté ailleurs : pas de prise de contrôle
    await b.connect()
    await b.sub(`room.${other.data.room.id}`)
    await ctx.refused(409, 'POST', '/profile/login', { body: { pseudo: b.pseudo } }, 'profil public déjà connecté sur un autre appareil')
    b.rt.close()

    // ── Changer / retirer le mot de passe : l'actuel est exigé ───────────────
    await ctx.refused(422, 'PUT', '/profile/password', { body: { password: 'nouveau-e2e' }, headers: tok(token) }, 'changement sans le mot de passe actuel')
    const removed = await api.delete('/profile/password', { body: { current_password: 'secret-e2e' }, headers: tok(token), expect: 200 })
    ctx.check(removed.data.player.has_password === false, 'mot de passe retiré : profil public')
    token = removed.data.token
    await ctx.refused(409, 'DELETE', '/profile/password', { body: { current_password: 'x' }, headers: tok(token) }, 'retirer un mot de passe absent')

    // ── Renommage (hors salon) ───────────────────────────────────────────────
    for (const r of [room, other.data.room]) {
        await api.delete('/rooms/:room/leave', { params: { room: r.id }, body: { player_id: a.id }, expect: 200 })
    }
    await ctx.refused(422, 'PATCH', '/profile', { body: { pseudo: b.pseudo }, headers: tok(token) }, 'renommer vers un pseudo pris')
    const newPseudo = ctx.pseudo('pa2')
    const renamed = await api.patch('/profile', { body: { pseudo: newPseudo }, headers: tok(token), expect: 200 })
    ctx.check(renamed.data.player.pseudo === newPseudo && typeof renamed.data.player.avatar_url === 'string', 'renommé : la photo suit le profil', renamed.data.player)
    const after = await api.get('/profile', { headers: tok(renamed.data.token), expect: 200 })
    ctx.check(after.data.player.pseudo === newPseudo && after.data.current === null, 'après départ et renommage : aucun salon, nouveau pseudo')
    // Le profil reste après le départ (plus de suppression des joueurs n'ayant pas joué)
    ctx.check((await api.post('/profile/login', { body: { pseudo: newPseudo }, expect: 200 })).data.player.id === a.id, 'profil conservé après le départ du salon')
    await api.delete('/rooms/:room/leave', { params: { room: other.data.room.id }, body: { player_id: b.id }, expect: 200 })

    // ── Profil administrateur (BINOME_ADMIN_PSEUDO du serveur de test) ───────
    const adminPseudo = ctx.target.adminPseudo
    const adminNoPwd = await ctx.refused(422, 'POST', '/profile/login', { body: { pseudo: adminPseudo } }, 'profil admin : mot de passe requis')
    ctx.check(Array.isArray(adminNoPwd.data.errors?.password), 'profil admin : erreur sur le champ password')
    await ctx.refused(403, 'POST', '/rooms', { body: { pseudo: adminPseudo.toLowerCase() } }, 'pseudo admin réservé sans connexion')
    const adm = await api.post('/profile/login', { body: { pseudo: adminPseudo, password: ctx.target.adminPassword }, expect: 200 })
    ctx.created.players.add(adm.data.player.id)
    ctx.check(adm.data.player.is_admin === true && adm.data.admin_token === api.adminToken, 'profil admin : is_admin et jeton d\'administration', { is_admin: adm.data.player.is_admin })
    const adminTok = tok(adm.data.token)
    await ctx.refused(403, 'PUT', '/profile/password', { body: { password: 'nimporte-quoi' }, headers: adminTok }, 'profil admin : mot de passe non modifiable')
    await ctx.refused(403, 'PATCH', '/profile', { body: { pseudo: ctx.pseudo('adm2') }, headers: adminTok }, 'profil admin : pseudo non modifiable')
    await ctx.refused(422, 'PATCH', '/profile', { body: { pseudo: adminPseudo }, headers: tok(renamed.data.token) }, 'renommer vers le pseudo admin')
    const admRoom = await api.post('/rooms', { body: { pseudo: adminPseudo }, headers: adminTok, expect: 201 })
    ctx.check(admRoom.data.profile.is_admin === true, 'profil admin : création de salon connectée')
    await api.delete('/rooms/:room/leave', { params: { room: admRoom.data.room.id }, body: { player_id: adm.data.player.id }, expect: 200 })
}
