#!/usr/bin/env node
// Génère server/testdata/forbidden-vectors.json avec le détecteur JS du front : le port
// Rust (server/src/support/forbidden.rs) doit donner exactement les mêmes résultats.
import { writeFileSync } from 'node:fs'
import { matchForbiddenWords } from '../../frontend/src/services/forbiddenWords.js'

const wordSets = [
    ['costume', 'poudre', 'voler'],
    ['Spider-Man', 'toile', 'araignée'],
    ['Pays imaginaire', 'fée', 'crochet'],
    ['épée', 'château', 'chevalier'],
    ['œil', 'bœuf', 'Straße'],
    ['magie', 'baguette', 'sorcier'],
    ['eau', 'île', 'roi'],
    ['courir', 'vitesse', 'éclair'],
]
const questions = [
    'Peut-il voler ?', 'Est-ce que ton personnage vol ?', 'Porte-t-il des COSTUMES ?', 'Est-il grand ?',
    'Utilise-t-il de la poudre magique ?', 'Il volait souvent ?', 'Ses costums sont rouges ?',
    "C'est spiderman ?", 'Il tisse une toile ?', 'Spider Man est-il ton ami ?', 'Les araignees le piquent ?',
    'Vit-il au pays imaginaire ?', 'Une fee le suit ?', 'Il a un crochet ?', 'Le capitaine Crochet ?',
    'A-t-il une épée ?', 'Il vit dans un chateau ?', 'Est-ce un chevalier ?', 'Les chevaliers le servent ?',
    "Il a un œil de verre ?", 'Il mange du boeuf ?', 'Il habite dans une strasse ?', 'oeil',
    'Fait-il de la magie ?', 'Il a une baguette ?', 'Les sorciers le craignent ?', 'magicien ?',
    "Il boit de l'eau ?", 'Il vit sur une ile ?', 'Est-il roi ?', 'Il est royal ?',
    'Il court vite ?', 'Il aime la vitesse ?', 'Il est rapide comme l\'éclair ?', 'courrir',
    '', '???', 'a b c', 'Volera-t-il demain ?', 'Les voleurs ?', 'poudres', 'POUDRE !',
    'Il est costumé ?', 'Costumier ?', 'Il crochète ?', 'Elle est féerique ?', 'Tisser des toiles ?',
    'Sorcière ?', 'Il chevauche ?', 'chateaux forts', 'Est-il roi ou reine ?', 'Iles lointaines ?',
]
const cases = []
for (const words of wordSets) for (const question of questions) {
    const r = matchForbiddenWords(question, words)
    cases.push({ question, words, exact: r.exact.map((x) => x.word), near: r.near.map((x) => x.word) })
}
writeFileSync(new URL('../../server/testdata/forbidden-vectors.json', import.meta.url), JSON.stringify(cases, null, 1) + '\n')
const hits = cases.filter((c) => c.exact.length || c.near.length).length
console.log(`${cases.length} cas, dont ${hits} avec au moins un mot détecté`)
