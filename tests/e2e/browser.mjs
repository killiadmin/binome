#!/usr/bin/env node
// Test navigateur du VRAI front (Vue compilé, servi par le serveur Rust) :
// 4 joueurs dans 4 navigateurs (format mobile), création / arrivée par l'interface,
// prêt, lancement, vote des personnages jusqu'au round 1, puis redémarrage du
// serveur en pleine partie : chaque page doit se reconnecter et se resynchroniser.
//
//   STATIC_DIR=…/dist tests/e2e/rust-stack.sh up
//   node tests/e2e/browser.mjs [--base http://127.0.0.1:18101/binome] [--headed]
//
// Playwright : on réutilise playwright-core d'un projet voisin (PLAYWRIGHT_CORE),
// sans l'ajouter aux dépendances de Binome.

import { execSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { Context, TARGETS } from './lib/context.mjs'

const args = process.argv.slice(2)
const base = args.includes('--base') ? args[args.indexOf('--base') + 1] : 'http://127.0.0.1:18101/binome'
const corePath = process.env.PLAYWRIGHT_CORE ?? '/Users/killiadmin/Meszappli/savoir-lire/node_modules/playwright-core'
const { chromium } = createRequire(import.meta.url)(corePath)

const ctx = new Context(TARGETS.rust())
await ctx.loginAdmin()
const results = []
const ok = (cond, msg) => { results.push({ ok: !!cond, msg }); console.log(`${cond ? '✓' : '✗'} ${msg}`) }

const browser = await chromium.launch({ headless: !args.includes('--headed') })
const names = ['Hote', 'Bea', 'Cle', 'Dan'].map((n) => ctx.pseudo(n))
const pages = []
const problems = []
for (const name of names) {
    const c = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true })
    const page = await c.newPage()
    page.on('pageerror', (e) => problems.push(`${name} : ${e.message}`))
    // lan-url.json absent hors réseau local : 404 attendu, géré par le front
    // (services/joinUrl.js) ; le navigateur le signale quand même en console.
    const expected404 = (url) => /\/lan-url\.json/.test(url)
    page.on('console', (m) => {
        if (m.type() === 'error' && !(m.text().includes('404') && expected404(m.location()?.url ?? ''))) problems.push(`${name} console : ${m.text()}`)
    })
    page.on('response', (r) => { if (r.status() >= 400 && !expected404(r.url())) problems.push(`${name} HTTP ${r.status()} ${r.request().method()} ${new URL(r.url()).pathname}`) })
    pages.push(page)
}
const [host, ...guests] = pages
const modal = (page) => page.locator('.modal.show')

try {
    // ── Création du salon par l'interface ──────────────────────────────────
    await host.goto(`${base}/rooms`)
    await host.getByRole('button', { name: /créer/i }).first().click()
    await modal(host).getByPlaceholder('Entrez votre nom').fill(names[0])
    await modal(host).getByRole('button', { name: 'Créer' }).click()
    await host.waitForFunction(() => JSON.parse(localStorage.getItem('session') ?? '{}').roomId)
    const session = await host.evaluate(() => JSON.parse(localStorage.getItem('session')))
    ok(session.gameCode?.length === 6, `salon créé par l'interface (code ${session.gameCode})`)
    ctx.created.players.add(session.playerId)

    // ── Arrivée des invités par l'interface ────────────────────────────────
    for (const [i, page] of guests.entries()) {
        await page.goto(`${base}/rooms`)
        await page.getByRole('button', { name: /rejoindre/i }).first().click()
        await modal(page).getByPlaceholder('Entrez votre nom').fill(names[i + 1])
        await modal(page).getByPlaceholder('Entrez le code').fill(session.gameCode)
        await modal(page).getByRole('button', { name: 'Rejoindre' }).click()
        await page.waitForFunction(() => JSON.parse(localStorage.getItem('session') ?? '{}').roomId)
        ctx.created.players.add(await page.evaluate(() => JSON.parse(localStorage.getItem('session')).playerId))
    }
    // Temps réel : l'hôte voit arriver tout le monde sans recharger
    for (const n of names.slice(1)) await host.getByText(n).first().waitFor({ timeout: 8000 })
    ok(true, 'l\'hôte voit les 3 invités arriver en temps réel (player.joined)')

    // ── Prêts puis lancement ───────────────────────────────────────────────
    for (const page of pages) await page.locator('button[title="Je suis prêt !"]').click()
    const start = host.getByRole('button', { name: /démarrer la partie/i })
    await start.waitFor({ timeout: 8000 })
    await host.waitForFunction(() => !document.querySelector('button.play-btn')?.disabled, null, { timeout: 8000 })
    ok(true, 'statut « prêt » des 4 joueurs reçu en temps réel par l\'hôte (player.ready)')
    await start.click()
    await Promise.all(pages.map((p) => p.waitForURL(/\/game\/\d+/, { timeout: 10_000 })))
    const gameId = Number(host.url().match(/game\/(\d+)/)[1])
    ctx.created.games.add(gameId)
    ok(true, `les 4 pages basculent sur la partie ${gameId} (game.started)`)

    // ── Vote des personnages ───────────────────────────────────────────────
    for (const page of pages) {
        const btn = page.getByRole('button', { name: /je le connais/i })
        await btn.waitFor({ timeout: 10_000 })
        await page.waitForFunction(() => [...document.querySelectorAll('button')].some((b) => /je le connais/i.test(b.textContent) && !b.disabled), null, { timeout: 10_000 })
        await btn.click()
    }
    // Le joueur courant voit « Question ou accusation », les autres « À toi de deviner… »
    const turnCounts = () => Promise.all(pages.map((p) => p.getByText('Question ou accusation').count()))
    let myTurn = await turnCounts()
    for (let i = 0; i < 40 && myTurn.filter((n) => n > 0).length !== 1; i++) {
        await new Promise((r) => setTimeout(r, 250))
        myTurn = await turnCounts()
    }
    ok(myTurn.filter((n) => n > 0).length === 1, `round 1 lancé chez tous, un seul joueur a la main (round.started) [${myTurn}]`)

    // ── Redémarrage du serveur en pleine partie ────────────────────────────
    execSync(`docker restart ${process.env.RESTART_CONTAINER ?? 'binome_rust_test'}`, { stdio: 'ignore' })
    await Promise.all(pages.map((p) => p.getByText(/Connexion perdue/).first().waitFor({ timeout: 15_000 }).catch(() => null)))
    const lostSeen = await Promise.all(pages.map((p) => p.getByText(/Connexion perdue/).count()))
    ok(lostSeen.some((n) => n > 0), 'coupure détectée par le front (« Connexion perdue »)')
    await Promise.all(pages.map((p) => p.waitForFunction(() => !/Connexion perdue/.test(document.body.innerText), null, { timeout: 30_000 })))
    ok(true, 'les 4 pages se reconnectent seules après le redémarrage du serveur')
    // Présence restaurée : aucun joueur affiché « hors ligne » chez l'hôte
    await host.waitForFunction(() => !/hors ligne/.test(document.body.innerText), null, { timeout: 15_000 })
    ok(true, 'présence restaurée : personne « hors ligne » après reconnexion')
    const turnAfter = await turnCounts()
    ok(JSON.stringify(turnAfter) === JSON.stringify(myTurn), `état de la partie identique après resynchronisation [${myTurn}] → [${turnAfter}]`)

    // Largeur mobile (exigence 320–390 px sans défilement horizontal)
    const widths = await Promise.all(pages.map((p) => p.evaluate(() => document.documentElement.scrollWidth)))
    ok(widths.every((w) => w <= 390), `pas de défilement horizontal en 390 px (${widths.join(', ')})`)
} catch (e) {
    ok(false, `scénario interrompu : ${e.message.split('\n')[0]}`)
    await host.screenshot({ path: new URL('./results/browser-failure.png', import.meta.url).pathname }).catch(() => {})
} finally {
    ok(problems.length === 0, `aucune erreur JavaScript dans les pages${problems.length ? ` : ${problems.slice(0, 5).join(' | ')}` : ''}`)
    await browser.close()
    for (const id of ctx.created.games) await ctx.api.delete('/admin/games/:game', { params: { game: id }, admin: true }).catch(() => {})
    for (const id of ctx.created.players) await ctx.api.delete('/admin/players/:player', { params: { player: id }, query: { with_games: 1 }, admin: true }).catch(() => {})
}

const failed = results.filter((r) => !r.ok)
console.log(`\n${results.length - failed.length}/${results.length} vérifications navigateur OK`)
process.exit(failed.length ? 1 : 0)
