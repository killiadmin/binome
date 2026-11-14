#!/usr/bin/env node
// Banc E2E de la migration Rust.
//
//   node tests/e2e/run.mjs [--only lobby,catalog,game8,game7,profile,controls,admins] [--save] [--verbose]
//
// --save : écrit le contrat observé (formes JSON des réponses et des events)
//          dans tests/e2e/contracts/<cible>.json et les latences dans results/.
// Code de sortie ≠ 0 si une vérification échoue.

import { mkdirSync, writeFileSync } from 'node:fs'
import { Context, TARGETS } from './lib/context.mjs'
import { fullGame } from './scenarios/full-game.mjs'
import { lobby } from './scenarios/lobby.mjs'
import { catalog } from './scenarios/catalog.mjs'
import { profile } from './scenarios/profile.mjs'
import { controls } from './scenarios/controls.mjs'
import { admins } from './scenarios/admins.mjs'

const args = process.argv.slice(2)
const opt = (name, fallback = null) => {
    const i = args.indexOf(`--${name}`)
    return i === -1 ? fallback : args[i + 1]
}
const flag = (name) => args.includes(`--${name}`)

const targetName = opt('target', 'rust')
if (!TARGETS[targetName]) {
    console.error(`Cible inconnue « ${targetName} ». Connues : ${Object.keys(TARGETS).join(', ')}`)
    process.exit(2)
}

const SCENARIOS = {
    lobby: (ctx) => lobby(ctx),
    catalog: (ctx) => catalog(ctx),
    game8: (ctx) => fullGame(ctx, { size: 8, rich: true }),
    game7: (ctx) => fullGame(ctx, { size: 7, rich: false, redraw: true }),
    profile: (ctx) => profile(ctx),
    controls: (ctx) => controls(ctx),
    admins: (ctx) => admins(ctx),
}
const selected = (opt('only') ?? Object.keys(SCENARIOS).join(',')).split(',')

const ctx = new Context(TARGETS[targetName](), { verbose: flag('verbose') })
console.log(`Banc E2E → ${targetName} (${ctx.target.baseUrl}), étiquette « ${ctx.tag} »`)
await ctx.loginAdmin()
await ctx.openGameModes()

let crashed = null
const t0 = Date.now()
for (const name of selected) {
    console.log(`▶ ${name}`)
    const started = Date.now()
    try {
        await SCENARIOS[name](ctx)
    } catch (e) {
        crashed = e
        ctx.check(false, `scénario ${name} interrompu : ${e.message}`)
        if (ctx.verbose) console.error(e.stack)
        break
    } finally {
        console.log(`  ${name} : ${((Date.now() - started) / 1000).toFixed(1)} s`)
    }
}
ctx.closeAll()

// ── Nettoyage par ids exacts (jamais par motif) ─────────────────────────────
const cleanup = { games: 0, players: 0, other: 0 }
for (const id of ctx.created.games) {
    const r = await ctx.api.delete('/admin/games/:game', { params: { game: id }, admin: true, deletion: true })
    if (r.status === 200) cleanup.games++
}
for (const id of ctx.created.players) {
    const r = await ctx.api.delete('/admin/players/:player', { params: { player: id }, query: { with_games: 1 }, admin: true, deletion: true })
    if (r.status === 200) cleanup.players++
}
for (const id of ctx.created.reactions) {
    if ((await ctx.api.delete('/admin/reactions/:reaction', { params: { reaction: id }, admin: true })).status === 200) cleanup.other++
}
for (const id of ctx.created.characters) {
    if ((await ctx.api.delete('/characters/:character', { params: { character: id }, admin: true })).status === 200) cleanup.other++
}
console.log(`Nettoyage : ${cleanup.games} parties, ${cleanup.players} joueurs, ${cleanup.other} autres`)

// ── Bilan ───────────────────────────────────────────────────────────────────
const failed = ctx.checks.filter((c) => !c.ok)
console.log(`\n${ctx.checks.length - failed.length}/${ctx.checks.length} vérifications OK en ${((Date.now() - t0) / 1000).toFixed(1)} s`)
for (const f of failed) console.log(`  ✗ ${f.message}`)
if (crashed && !ctx.verbose) console.log(`\n${crashed.stack}`)

if (flag('save')) {
    const contractPath = new URL(`./contracts/${targetName}.json`, import.meta.url)
    ctx.recorder.save(contractPath.pathname)
    const resultsDir = new URL('./results/', import.meta.url)
    mkdirSync(resultsDir, { recursive: true })
    writeFileSync(new URL(`latencies-${targetName}.json`, resultsDir), `${JSON.stringify(ctx.recorder.latencyTable(), null, 2)}\n`)
    console.log(`Contrat : ${contractPath.pathname}`)
}

process.exit(failed.length ? 1 : 0)
