import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'
import { addShape, serializeShape } from './shapes.mjs'

/**
 * Enregistre la forme de chaque réponse HTTP et de chaque event temps réel,
 * plus les latences HTTP (pour les mesures).
 * Clés : « GET /api/games/:game 200 » et « event round.started @game ».
 */
export class Recorder {
    constructor() {
        this.shapes = new Map()   // clé → Map(chemin → Set)
        this.samples = new Map()  // clé → nombre d'occurrences
        this.latencies = new Map() // « GET /api/games/:game » → [ms]
    }

    record(key, value) {
        if (!this.shapes.has(key)) this.shapes.set(key, new Map())
        addShape(this.shapes.get(key), value)
        this.samples.set(key, (this.samples.get(key) ?? 0) + 1)
    }

    latency(route, ms) {
        if (!this.slowest || ms > this.slowest.ms) this.slowest = { route, ms: Math.round(ms) }
        if (!this.latencies.has(route)) this.latencies.set(route, [])
        this.latencies.get(route).push(ms)
    }

    catalog() {
        return Object.fromEntries(
            [...this.shapes.keys()].sort().map((k) => [k, serializeShape(this.shapes.get(k))]),
        )
    }

    latencyTable() {
        const pct = (sorted, p) => sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))]
        return [...this.latencies.entries()]
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([route, values]) => {
                const s = [...values].sort((a, b) => a - b)
                return { route, n: s.length, p50: pct(s, 50), p95: pct(s, 95), max: s[s.length - 1] }
            })
    }

    /**
     * Enregistre le contrat en le CUMULANT avec celui déjà sur disque : les parties
     * étant tirées au sort, chaque exécution n'observe qu'une partie des variantes
     * (partenaire déjà éliminé ou non, orphelin…). `fresh` repart de zéro.
     */
    save(path, { fresh = false } = {}) {
        mkdirSync(dirname(path), { recursive: true })
        const merged = fresh || !existsSync(path) ? {} : JSON.parse(readFileSync(path, 'utf8'))
        for (const [key, shape] of Object.entries(this.catalog())) {
            merged[key] ??= {}
            for (const [p, types] of Object.entries(shape)) {
                const all = new Set([...(merged[key][p]?.split(' | ') ?? []), ...types.split(' | ')])
                merged[key][p] = [...all].sort().join(' | ')
            }
        }
        const sorted = Object.fromEntries(Object.keys(merged).sort().map((k) => [k, Object.fromEntries(Object.keys(merged[k]).sort().map((p) => [p, merged[k][p]]))]))
        writeFileSync(path, `${JSON.stringify(sorted, null, 2)}\n`)
    }
}
