#!/usr/bin/env node
// Compare deux contrats observés : node tests/e2e/compare.mjs laravel-ref rust
// Code de sortie ≠ 0 au moindre écart (champ manquant / en trop, type différent, route absente).

import { readFileSync } from 'node:fs'
import { compareCatalogs } from './lib/shapes.mjs'

const [refName = 'laravel-ref', candName = 'rust'] = process.argv.slice(2)
const load = (n) => JSON.parse(readFileSync(new URL(`./contracts/${n}.json`, import.meta.url), 'utf8'))

// Hors contrat : sérialisations accidentelles d'Eloquent, jamais lues par le front
// (vérifié dans frontend/src) et non reproduites par Rust.
//  - `/confirm` renvoie, via les relations chargées en cours de traitement, tout le
//    graphe de la partie (binômes, rounds, joueur courant) : inutile, et il expose la
//    composition des binômes à l'accusé ;
//  - `actions.character_id` est une colonne morte (supprimée du schéma).
const IGNORED = [
    /^\$\.action\.round\.(game|current_player)(\.|$)/,
    /^\$\.action\.character_id$/,
]
const strip = (catalog) => Object.fromEntries(Object.entries(catalog).map(([k, shape]) => [
    k, Object.fromEntries(Object.entries(shape).filter(([p]) => !IGNORED.some((re) => re.test(p)))),
]))
// Changements VOULUS depuis la migration (validés par l'utilisateur, 2026-11-12) :
//  - les parties survivent à la suppression de leur salon : room_id / room_code
//    peuvent valoir null dans l'historique et l'admin ;
//  - suppression de parties protégée par un mot de passe dédié : nouveaux refus 403.
const ACCEPTED = [
    /^TYPE +GET \/api\/(history|admin)\/games(\/:game)? 200 +\$(\.games\[\])?\.room_(id|code):/,
    /^TYPE +GET \/api\/history\/players 200 +\$\.games\[\]\.room_code:/,
    /^NOUVEAU +DELETE \/api\/admin\/(games\/:game|players\/:player) 403$/,
    // Profils joueurs (2026-11-12) : routes /profile*, connexion automatique en créant /
    // rejoignant un salon (profile + profile_token), pseudo protégé refusé sans jeton (403).
    // Les joueurs ne sont plus supprimés en quittant un salon : un joueur hors salon
    // peut apparaître dans l'admin (room_code null) et recevoir 403 sur /ready.
    /^NOUVEAU +(GET|POST|PATCH|PUT|DELETE) \/api\/profile(\/[\w-]+)? \d{3}$/,
    /^CHAMP EN TROP +POST \/api\/rooms(\/join)? 20[01] +\$\.profile(_token|\.\w+)? /,
    /^NOUVEAU +POST \/api\/rooms(\/join)? 403$/,
    /^NOUVEAU +PATCH \/api\/rooms\/:room\/ready 403$/,
    /^TYPE +GET \/api\/admin\/players 200 +\$\.players\[\]\.room_code:/,
    // Changements de personnage plafonnés à 2 par joueur et par partie (2026-11-12) :
    // compteur restant par joueur dans l'état public et dans characters.redrawn.
    /^CHAMP EN TROP +GET \/api\/games\/:game 200 +\$\.(max_character_changes|players\[\]\.character_changes_left) /,
    /^CHAMP EN TROP +event characters\.redrawn @game +\$\.(change_player_ids|character_changes_left)(\[\](\.(left|player_id))?)? /,
    // Identité signée (checklist point 5, 2026-11-14) : agir au nom d'un autre joueur
    // ou sans jeton valide → 401 / 403.
    /^NOUVEAU +GET \/api\/games\/:game\/me 40[13]$/,
    /^NOUVEAU +POST \/api\/games\/:game\/reactions 403$/,
    /^NOUVEAU +POST \/api\/players\/:player\/avatar 401$/,
    // Contrôles de l'administrateur (migration 6, 2026-11-14) : modes de jeu, cosmos et
    // binômes activables, validation manuelle des profils (historique, emojis image).
    /^NOUVEAU +(GET|PATCH) \/api\/admin\/(settings(\/game-modes\/:mode)?|cosmos\/:cosmos|binomes|players\/:player\/validation) \d{3}$/,
    /^CHAMP EN TROP +GET \/api\/admin\/players 200 +\$\.players\[\]\.(has_password|is_admin|is_validated|validated_at) /,
    /^CHAMP EN TROP +GET \/api\/cosmos 200 +\$\.(cosmos\[\]\.is_enabled|game_modes(\[\])?) /,
    /^CHAMP EN TROP +PATCH \/api\/rooms\/:room\/settings 422 +\$\.errors\.game_mode(\[\])? /,
    /^NOUVEAU +GET \/api\/history\/(games|leaderboard) 40[13]$/,
    /^NOUVEAU +POST \/api\/reactions 403$/,
    // Administrateurs désignés (migration 7, 2026-11-14) : rôle dans les profils et la
    // liste des joueurs ; un administrateur garde son mot de passe (409).
    /^NOUVEAU +PATCH \/api\/admin\/players\/:player\/admin \d{3}$/,
    /^NOUVEAU +GET \/api\/admin\/players 403$/,
    /^CHAMP EN TROP +GET \/api\/admin\/players 200 +\$\.players\[\]\.admin_role /,
    /^CHAMP EN TROP +(GET|PATCH|POST|PUT|DELETE) \/api\/profile[\w/-]* 200 +\$\.player\.admin_role /,
    /^CHAMP EN TROP +POST \/api\/rooms(\/join)? 20[01] +\$\.profile\.admin_role /,
    /^NOUVEAU +DELETE \/api\/profile\/password 409$/,
    // Boîte à idées (migration 8, 2026-11-14) : routes /ideas* et /admin/ideas/:idea.
    /^NOUVEAU +(GET|POST|PUT|PATCH|DELETE) \/api\/(admin\/)?ideas(\/:idea(\/vote)?)? \d{3}$/,
]
const issues = compareCatalogs(strip(load(refName)), strip(load(candName))).filter((i) => !ACCEPTED.some((re) => re.test(i)))

if (issues.length === 0) {
    console.log(`Contrats identiques : ${candName} reproduit ${refName}.`)
    process.exit(0)
}
console.log(`${issues.length} écart(s) entre ${refName} et ${candName} :\n`)
for (const i of issues) console.log(`  ${i}`)
process.exit(1)
