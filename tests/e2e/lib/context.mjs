import { Api } from './http.mjs'
import { Recorder } from './recorder.mjs'
import { connect } from './realtime.mjs'

/**
 * Cible du banc : le serveur Rust de test (copie de la base), lancé par
 * tests/e2e/rust-stack.sh. On n'ouvre jamais la pile en service : le banc crée,
 * modifie et supprime des données. Le contrat de l'ancien backend Laravel est
 * figé dans contracts/laravel-ref.json (référence de compare.mjs).
 */
export const TARGETS = {
    rust: () => ({
        name: 'rust',
        baseUrl: process.env.E2E_BASE_URL ?? 'http://127.0.0.1:18101/binome',
        realtime: { protocol: 'native', wsUrl: process.env.E2E_WS_URL ?? 'ws://127.0.0.1:18101/binome/ws' },
        // Mots de passe du serveur de test (voir rust-stack.sh), jamais les vrais
        adminPassword: process.env.E2E_ADMIN_PASSWORD ?? 'e2e-admin-test',
        deletePassword: process.env.E2E_DELETE_PASSWORD ?? 'e2e-suppression-test',
        // Pseudo du profil administrateur du serveur de test (BINOME_ADMIN_PSEUDO)
        adminPseudo: process.env.E2E_ADMIN_PSEUDO ?? 'e2eAdminBanc',
    }),
}

export class Context {
    constructor(target, { verbose = false, quiet = false } = {}) {
        this.quiet = quiet
        this.target = target
        this.verbose = verbose
        this.recorder = new Recorder()
        this.api = new Api({ baseUrl: target.baseUrl, recorder: this.recorder })
        this.api.deletePassword = target.deletePassword
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

    /** Connexion WebSocket ; `token` explicite pour les tests de refus (sinon celui du joueur). */
    async connect(label, playerId = null, token = undefined) {
        const t = token !== undefined ? token : playerId ? this.api.tokens.get(Number(playerId)) : null
        const client = await connect({ ...this.target.realtime, label, recorder: this.recorder, playerId, token: t })
        this.clients.add(client)
        return client
    }

    /** Jeton admin : demandé à l'API comme le fait le front. */
    async loginAdmin() {
        const { data } = await this.api.post('/access/characters', { body: { password: this.target.adminPassword }, expect: 200 })
        this.api.adminToken = data.token
        // Jeton = HMAC-SHA256(empreinte configurée, « binome-characters-access ») :
        // non recalculable ici, on vérifie sa forme (son acceptation est testée par les routes admin)
        this.check(/^[0-9a-f]{64}$/.test(data.token), 'jeton admin : 64 caractères hexadécimaux')
    }

    check(condition, message, detail = undefined) {
        this.checks.push({ ok: Boolean(condition), message, detail })
        if (!condition) console.log(`   ✗ ${message}${detail !== undefined ? ` — ${JSON.stringify(detail).slice(0, 300)}` : ''}`)
        else if (this.verbose) console.log(`   ✓ ${message}`)
        return Boolean(condition)
    }

    /** Appel censé échouer avec un statut précis (règle métier, validation…). */
    async refused(status, method, template, options, message) {
        const res = await this.api.call(method, template, options)
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
