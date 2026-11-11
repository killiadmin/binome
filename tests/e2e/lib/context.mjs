import { readFileSync } from 'node:fs'
import { createHmac } from 'node:crypto'
import { Api } from './http.mjs'
import { Recorder } from './recorder.mjs'
import { connect } from './realtime.mjs'

const ROOT = new URL('../../../', import.meta.url)

/** Lit une clé d'un fichier .env sans jamais l'afficher. */
function envValue(file, key) {
    const line = readFileSync(new URL(file, ROOT), 'utf8')
        .split('\n')
        .find((l) => l.startsWith(`${key}=`))
    return line ? line.slice(key.length + 1).trim().replace(/^"|"$/g, '') : null
}

/**
 * Cibles connues. On n'ouvre jamais la pile en service (ports 8001 / 5173) :
 * le banc crée, modifie et supprime des données.
 */
export const TARGETS = {
    'laravel-ref': () => ({
        name: 'laravel-ref',
        baseUrl: 'http://127.0.0.1:18001',
        realtime: {
            protocol: 'pusher',
            wsUrl: 'ws://127.0.0.1:18080',
            appKey: envValue('backend/.env', 'REVERB_APP_KEY'),
            authUrl: 'http://127.0.0.1:18001/broadcasting/auth',
        },
        adminPassword: envValue('backend/.env', 'CHARACTERS_ACCESS_PASSWORD'),
    }),
    // Serveur Rust de test (copie de la base), lancé par tests/e2e/rust-stack.sh
    rust: () => ({
        name: 'rust',
        baseUrl: 'http://127.0.0.1:18101/binome',
        realtime: { protocol: 'native', wsUrl: 'ws://127.0.0.1:18101/binome/ws' },
        adminPassword: envValue('backend/.env', 'CHARACTERS_ACCESS_PASSWORD'),
    }),
}

export class Context {
    constructor(target, { verbose = false, quiet = false } = {}) {
        this.quiet = quiet
        this.target = target
        this.verbose = verbose
        this.recorder = new Recorder()
        this.api = new Api({ baseUrl: target.baseUrl, recorder: this.recorder })
        this.tag = Math.random().toString(36).slice(2, 7)
        this.checks = []
        this.clients = new Set()
        // Tout ce que le banc crée, pour un nettoyage par ids exacts.
        this.created = { players: new Set(), games: new Set(), reactions: new Set(), characters: new Set() }
    }

    /** Pseudo unique du banc (≤ 20 caractères, contrainte du backend). */
    pseudo(name) {
        return `e2e${this.tag}${name}`.slice(0, 20)
    }

    async connect(label, playerId = null) {
        const client = await connect({ ...this.target.realtime, label, recorder: this.recorder, playerId })
        this.clients.add(client)
        return client
    }

    /** Jeton admin : demandé à l'API comme le fait le front. */
    async loginAdmin() {
        const { data } = await this.api.post('/access/characters', { body: { password: this.target.adminPassword }, expect: 200 })
        this.api.adminToken = data.token
        // Même dérivation que EnsureCharactersAccess::token() : garde-fou du contrat
        const expected = createHmac('sha256', this.target.adminPassword).update('binome-characters-access').digest('hex')
        this.check(data.token === expected, 'jeton admin = HMAC-SHA256(mot de passe, « binome-characters-access »)')
    }

    check(condition, message, detail = undefined) {
        this.checks.push({ ok: Boolean(condition), message, detail })
        if (!condition) console.log(`   ✗ ${message}${detail !== undefined ? ` — ${JSON.stringify(detail).slice(0, 300)}` : ''}`)
        else if (this.verbose) console.log(`   ✓ ${message}`)
        return Boolean(condition)
    }

    /**
     * Écart connu de la référence : la règle documentée n'est pas respectée par
     * Laravel. Signalé à part (pas compté comme échec) sur `knownOn`, vérifié
     * normalement sur les autres cibles.
     */
    deviation(condition, message, knownOn, detail) {
        if (!condition && knownOn === this.target.name) {
            this.checks.push({ ok: true, known: true, message })
            console.log(`   ⚠ écart connu de ${knownOn} : ${message}`)
            return false
        }
        return this.check(condition, message, detail)
    }

    /** Appel censé échouer avec un statut précis (règle métier, validation…). */
    async refused(status, method, template, options, message, { knownOn = null } = {}) {
        const res = await this.api.call(method, template, options)
        if (knownOn) return this.deviation(res.status === status, `${message} → ${status}`, knownOn), res
        this.check(res.status === status, `${message} → ${status}`, res.status === status ? undefined : { status: res.status, data: res.data })
        if (res.status === status && status !== 404 && res.data && typeof res.data === 'object') {
            this.check(typeof res.data.message === 'string' && res.data.message.length > 0, `${message} : message d'erreur présent`)
        }
        return res
    }

    closeAll() {
        for (const c of this.clients) c.close()
    }
}
