import { readFileSync } from 'node:fs'

/**
 * Client HTTP du banc. Chaque appel est décrit par un gabarit de route
 * (« /games/:game/me ») : c'est lui qui sert de clé au contrat, pas l'URL réelle.
 *
 *   await api.call('POST', '/rooms/:room/ready', { params: { room: 3 }, body: { player_id: 7 }, expect: 200 })
 */
export class Api {
    constructor({ baseUrl, prefix = '/api', recorder }) {
        this.baseUrl = baseUrl.replace(/\/$/, '')
        this.prefix = prefix
        this.recorder = recorder
        this.adminToken = null
    }

    async call(method, template, { params = {}, query = null, body = null, form = null, headers = {}, expect = null, admin = false, raw = false } = {}) {
        const path = template.replace(/:(\w+)/g, (_, name) => {
            if (params[name] === undefined) throw new Error(`Paramètre « ${name} » manquant pour ${template}`)
            return encodeURIComponent(params[name])
        })
        const qs = query ? `?${new URLSearchParams(query)}` : ''
        const url = `${this.baseUrl}${this.prefix}${path}${qs}`

        // Une requête bloquée doit échouer en nommant sa route, pas geler le banc
        const init = { method, headers: { Accept: 'application/json', ...headers }, signal: AbortSignal.timeout(30_000) }
        if (admin && this.adminToken) init.headers['X-Characters-Access'] = this.adminToken
        if (form) {
            init.body = form // FormData : fetch pose le boundary multipart
        } else if (body !== null) {
            init.headers['Content-Type'] = 'application/json'
            init.body = JSON.stringify(body)
        }

        const started = performance.now()
        let res
        try {
            res = await fetch(url, init)
        } catch (e) {
            throw new Error(`${method} ${this.prefix}${template} : ${e.name === 'TimeoutError' ? 'aucune réponse en 30 s' : e.message}`)
        }
        const elapsed = performance.now() - started
        const routeKey = `${method} ${this.prefix}${template}`
        this.recorder?.latency(routeKey, elapsed)

        let data
        const type = res.headers.get('content-type') ?? ''
        if (raw || !type.includes('json')) {
            data = { contentType: type, bytes: (await res.arrayBuffer()).byteLength, cacheControl: res.headers.get('cache-control') }
            this.recorder?.record(`${routeKey} ${res.status} [binaire]`, { content_type: type.split(';')[0], cache_control: data.cacheControl })
        } else {
            const text = await res.text()
            data = text ? JSON.parse(text) : null
            // APP_DEBUG=true : Laravel joint la pile d'appels aux erreurs. Ce n'est
            // pas le contrat (le front ne lit que `message` / `errors`) : on l'écarte.
            let recorded = data
            if (data && typeof data === 'object' && 'trace' in data && 'exception' in data) {
                const { exception, file, line, trace, ...rest } = data
                recorded = rest
            }
            this.recorder?.record(`${routeKey} ${res.status}`, recorded)
        }

        const expected = expect === null ? null : [].concat(expect)
        if (expected && !expected.includes(res.status)) {
            const err = new Error(`${method} ${url} → ${res.status} (attendu ${expected.join('|')}) : ${JSON.stringify(data).slice(0, 400)}`)
            err.status = res.status
            err.data = data
            throw err
        }
        return { status: res.status, data, headers: res.headers }
    }

    get(t, o) { return this.call('GET', t, o) }
    post(t, o) { return this.call('POST', t, o) }
    patch(t, o) { return this.call('PATCH', t, o) }
    put(t, o) { return this.call('PUT', t, o) }
    delete(t, o) { return this.call('DELETE', t, o) }
}

const FIXTURES = new URL('../fixtures/', import.meta.url)

/** Data URI JPEG d'une image du dossier fixtures. */
export function jpegDataUri(name) {
    return `data:image/jpeg;base64,${readFileSync(new URL(name, FIXTURES)).toString('base64')}`
}

/** Fichier du dossier fixtures, prêt pour un FormData. */
export function fixtureBlob(name, type) {
    return new Blob([readFileSync(new URL(name, FIXTURES))], { type })
}
