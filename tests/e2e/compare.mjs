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
const issues = compareCatalogs(strip(load(refName)), strip(load(candName)))

if (issues.length === 0) {
    console.log(`Contrats identiques : ${candName} reproduit ${refName}.`)
    process.exit(0)
}
console.log(`${issues.length} écart(s) entre ${refName} et ${candName} :\n`)
for (const i of issues) console.log(`  ${i}`)
process.exit(1)
