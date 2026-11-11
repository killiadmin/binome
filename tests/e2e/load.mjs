#!/usr/bin/env node
// Mesure de charge : mêmes conditions pour Laravel et Rust.
//
//   node tests/e2e/load.mjs [--containers binome_rust_test] --games 6 --duration 60 --ws 1000 [--rate N] [--idle 20]
//
// Déroulé :
//   1. repos       : échantillonne CPU / RAM des conteneurs pendant --idle s
//   2. websockets  : ouvre --ws connexions abonnées au canal public « lobby » → RAM tenue
//   3. charge      : --games parties de 8 joueurs en parallèle (sans sondes), enchaînées
//                    pendant --duration s ; latences HTTP
// Écrit tests/e2e/results/load-<cible>-<date>.json et affiche un résumé.

import { spawn } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { Context, TARGETS } from './lib/context.mjs'
import { fullGame } from './scenarios/full-game.mjs'
import { sleep } from './lib/bots.mjs'

const args = process.argv.slice(2)
const opt = (n, d) => { const i = args.indexOf(`--${n}`); return i === -1 ? d : args[i + 1] }
const targetName = opt('target', 'rust')
const games = Number(opt('games', 6))
const wsCount = Number(opt('ws', 500))
const idleSeconds = Number(opt('idle', 20))
const duration = Number(opt('duration', 60))
// --rate N : charge FIXE de N parties par minute (toutes tables confondues), pour
// comparer deux serveurs à charge égale. Sans --rate : chaque table enchaîne au plus
// vite (charge maximale, le débit dépend du serveur).
const rate = opt('rate', null) === null ? null : Number(opt('rate'))

const CONTAINERS = { rust: ['binome_rust_test'] }
const containers = (opt('containers', null)?.split(',')) ?? CONTAINERS[targetName]

// ── Échantillonnage docker stats (flux continu, ~1 mesure/s/conteneur) ─────────
function toMiB(s) {
    const m = s.match(/([\d.]+)\s*([KMG]i?B)/)
    if (!m) return 0
    const v = Number(m[1])
    return { KiB: v / 1024, KB: v / 1024, MiB: v, MB: v, GiB: v * 1024, GB: v * 1024 }[m[2]] ?? v
}

class Sampler {
    constructor(names) {
        this.samples = []
        this.phase = 'init'
        this.proc = spawn('docker', ['stats', '--format', '{{json .}}', ...names])
        let buf = ''
        this.proc.stdout.on('data', (chunk) => {
            buf += chunk
            const lines = buf.split('\n')
            buf = lines.pop()
            for (const raw of lines) {
                const line = raw.replace(/\x1b\[[0-9;]*[A-Za-z]/g, '').trim()
                if (!line.startsWith('{')) continue
                const s = JSON.parse(line)
                this.samples.push({
                    phase: this.phase,
                    name: s.Name,
                    cpu: Number(s.CPUPerc.replace('%', '')),
                    mem: toMiB(s.MemUsage.split('/')[0]),
                    pids: Number(s.PIDs),
                })
            }
        })
    }

    summary(phase) {
        const rows = {}
        for (const s of this.samples.filter((x) => x.phase === phase)) {
            rows[s.name] ??= { cpu: [], mem: [], pids: [] }
            rows[s.name].cpu.push(s.cpu)
            rows[s.name].mem.push(s.mem)
            rows[s.name].pids.push(s.pids)
        }
        const avg = (a) => a.reduce((x, y) => x + y, 0) / a.length
        const out = {}
        for (const [name, r] of Object.entries(rows)) {
            out[name] = {
                cpu_avg: +avg(r.cpu).toFixed(2), cpu_max: +Math.max(...r.cpu).toFixed(2),
                mem_avg_mib: +avg(r.mem).toFixed(1), mem_max_mib: +Math.max(...r.mem).toFixed(1),
                pids_max: Math.max(...r.pids), n: r.cpu.length,
            }
        }
        const tot = Object.values(out)
        out.TOTAL = {
            cpu_avg: +tot.reduce((x, r) => x + r.cpu_avg, 0).toFixed(2),
            cpu_max: +tot.reduce((x, r) => x + r.cpu_max, 0).toFixed(2),
            mem_avg_mib: +tot.reduce((x, r) => x + r.mem_avg_mib, 0).toFixed(1),
            mem_max_mib: +tot.reduce((x, r) => x + r.mem_max_mib, 0).toFixed(1),
            pids_max: tot.reduce((x, r) => x + r.pids_max, 0),
        }
        return out
    }

    stop() { this.proc.kill() }
}

const printPhase = (title, s) => {
    console.log(`\n${title}`)
    console.table(Object.fromEntries(Object.entries(s).map(([k, v]) => [k.replace(/^binome_ref-|-1$/g, ''), v])))
}

// ── Déroulé ─────────────────────────────────────────────────────────────────
const target = TARGETS[targetName]()
const sampler = new Sampler(containers)
await sleep(3000)

sampler.phase = 'idle'
console.log(`Repos : ${idleSeconds} s…`)
await sleep(idleSeconds * 1000)

// WebSockets inactifs
sampler.phase = 'ws'
const wsCtx = new Context(target)
const sockets = []
const tWs = Date.now()
let wsFailed = 0
for (let i = 0; i < wsCount; i += 50) {
    const batch = await Promise.allSettled(Array.from({ length: Math.min(50, wsCount - i) }, async (_, k) => {
        const c = await wsCtx.connect(`ws${i + k}`)
        await c.subscribe('lobby')
        return c
    }))
    for (const r of batch) r.status === 'fulfilled' ? sockets.push(r.value) : wsFailed++
}
const wsOpenSeconds = (Date.now() - tWs) / 1000
console.log(`${sockets.length} connexions ouvertes (${wsFailed} échecs) en ${wsOpenSeconds.toFixed(1)} s ; maintien 15 s…`)
await sleep(15_000)
const wsAlive = sockets.filter((s) => !s.closed).length
sockets.forEach((s) => s.close())
await sleep(3000)

// Charge : parties concurrentes
sampler.phase = 'load'
const contexts = []
const tLoad = Date.now()
const results = await Promise.allSettled(Array.from({ length: games }, async () => {
    // Chaque « table » enchaîne les parties jusqu'à la fin de la fenêtre
    const slot = rate ? (games * 60_000) / rate : 0 // ms entre deux débuts de partie d'une table
    let next = Date.now()
    while (Date.now() - tLoad < duration * 1000) {
        const ctx = new Context(target, { quiet: true })
        contexts.push(ctx)
        await ctx.loginAdmin()
        await fullGame(ctx, { size: 8, rich: false, redraw: false })
        if (rate) {
            next += slot
            await sleep(Math.max(0, next - Date.now()))
        }
    }
}))
const loadSeconds = (Date.now() - tLoad) / 1000
sampler.phase = 'after'
await sleep(2000)
sampler.stop()

// Nettoyage par ids exacts
for (const ctx of contexts) {
    for (const id of ctx.created.games) await ctx.api.delete('/admin/games/:game', { params: { game: id }, admin: true })
    for (const id of ctx.created.players) await ctx.api.delete('/admin/players/:player', { params: { player: id }, query: { with_games: 1 }, admin: true })
}

// Latences agrégées
const all = new Map()
for (const ctx of contexts) {
    for (const [route, values] of ctx.recorder.latencies) {
        if (!all.has(route)) all.set(route, [])
        all.get(route).push(...values)
    }
}
const pct = (s, p) => s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))]
const flat = [...all.values()].flat().sort((a, b) => a - b)
const slowest = contexts.map((c) => c.recorder.slowest).filter(Boolean).sort((a, b) => b.ms - a.ms)[0] ?? null
const latency = {
    requests: flat.length,
    p50_ms: +pct(flat, 50).toFixed(1), p95_ms: +pct(flat, 95).toFixed(1), p99_ms: +pct(flat, 99).toFixed(1), max_ms: +flat.at(-1).toFixed(1),
    req_per_s: +(flat.length / loadSeconds).toFixed(1),
    slowest_request: slowest,
}
const perRoute = [...all.entries()].map(([route, v]) => {
    const s = [...v].sort((a, b) => a - b)
    return { route, n: s.length, p50: +pct(s, 50).toFixed(1), p95: +pct(s, 95).toFixed(1) }
}).sort((a, b) => b.p95 - a.p95)

const failures = results.filter((r) => r.status === 'rejected').map((r) => r.reason.message)
const failedChecks = contexts.flatMap((c) => c.checks.filter((x) => !x.ok).map((x) => x.message))

const report = {
    target: targetName,
    date: new Date().toISOString(),
    machine: `${process.platform} ${process.arch}`,
    params: { parallel_games: games, duration_s: duration, rate_per_min: rate, games_played: contexts.length, players_per_game: 8, ws: wsCount, idle_seconds: idleSeconds },
    idle: sampler.summary('idle'),
    websockets: { requested: wsCount, opened: sockets.length, failed: wsFailed, still_open_after_15s: wsAlive, open_seconds: +wsOpenSeconds.toFixed(1), resources: sampler.summary('ws') },
    load: { seconds: +loadSeconds.toFixed(1), games_played: contexts.length, failures, failed_checks: failedChecks, latency, resources: sampler.summary('load'), slowest_routes: perRoute.slice(0, 10) },
}

printPhase('REPOS', report.idle)
printPhase(`WEBSOCKETS : ${sockets.length} connexions inactives`, report.websockets.resources)
printPhase(`CHARGE : ${games} tables × 8 joueurs${rate ? `, ${rate} parties/min imposées` : ' en continu'}, ${contexts.length} parties en ${loadSeconds.toFixed(1)} s`, report.load.resources)
console.log('Latence HTTP globale :', latency)
console.log('Routes les plus lentes (p95) :')
console.table(perRoute.slice(0, 8))
if (failures.length || failedChecks.length) console.log('ÉCHECS :', failures, failedChecks)

const dir = new URL('./results/', import.meta.url)
mkdirSync(dir, { recursive: true })
const file = new URL(`load-${targetName}-${report.date.slice(0, 19).replace(/:/g, '')}.json`, dir)
writeFileSync(file, `${JSON.stringify(report, null, 2)}\n`)
console.log(`\nRapport : ${file.pathname}`)
process.exit(failures.length ? 1 : 0)
