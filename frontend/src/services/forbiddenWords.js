/**
 * Détection des mots interdits dans une question, en deux niveaux :
 *
 *  - exact (100 %) : un mot de la question EST le mot interdit, à la casse,
 *    aux accents et au pluriel (s/x final) près. La cible DOIT mentir.
 *  - proche (≥ 80 %) : faute de frappe, conjugaison ou dérivé
 *    (« vol » / « voler », « costum » / « costume »). Le mensonge est légal.
 *
 * Purement indicatif : aucun bouton n'est bloqué, la cible garde toujours le
 * dernier mot (faute de frappe du questionneur, cas exceptionnel…).
 *
 * Port PHP à garder synchronisé : backend/app/Support/ForbiddenWordMatcher.php
 * (utilisé par le récap de fin de partie).
 */

export const NEAR_THRESHOLD = 0.8

// Terminaisons françaises courantes, de la plus longue à la plus courte :
// on n'en retire qu'une, et seulement s'il reste au moins 3 lettres.
const SUFFIXES = [
  'issements', 'issement', 'eraient', 'assions', 'erions', 'eriez', 'erons', 'eront',
  'erais', 'erait', 'aient', 'ement', 'ments', 'ment', 'euse', 'eurs', 'ions', 'eur',
  'ees', 'iez', 'ais', 'ait', 'ant', 'ent', 'es', 'ee', 'er', 'ir', 'ez', 'ai', 'e', 's', 'x',
]

export function normalize(text) {
  return String(text ?? '')
      .toLowerCase()
      .replace(/œ/g, 'oe').replace(/æ/g, 'ae').replace(/ß/g, 'ss')
      .normalize('NFD').replace(/[̀-ͯ]/g, '')
}

export function tokenize(text) {
  return normalize(text).split(/[^a-z0-9]+/).filter(Boolean)
}

// Retire la marque du pluriel : « costumes » compte comme « costume ».
function singular(token) {
  return token.length > 3 && /[sx]$/.test(token) ? token.slice(0, -1) : token
}

function stem(token) {
  for (const suffix of SUFFIXES) {
    if (token.endsWith(suffix) && token.length - suffix.length >= 3) {
      return token.slice(0, -suffix.length)
    }
  }
  return token
}

function levenshtein(a, b) {
  if (a === b) return 0
  let prev = Array.from({length: b.length + 1}, (_, j) => j)
  for (let i = 1; i <= a.length; i++) {
    const cur = [i]
    for (let j = 1; j <= b.length; j++) {
      cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1))
    }
    prev = cur
  }
  return prev[b.length]
}

/** Similarité entre deux mots normalisés, de 0 à 1 (1 = même mot). */
function similarity(q, f) {
  if (singular(q) === singular(f)) return 1
  // Mots très courts : seule l'égalité compte, sinon « est », « il »… collent à tout.
  if (q.length < 3 || f.length < 3) return 0
  let score = 1 - levenshtein(q, f) / Math.max(q.length, f.length)
  if (stem(q) === stem(f)) score = Math.max(score, 0.9)
  return score
}

/**
 * Meilleure correspondance d'un mot interdit (éventuellement composé :
 * « Pays imaginaire », « Spider-Man ») dans les tokens de la question.
 */
function bestMatch(questionTokens, forbidden) {
  const fTokens = tokenize(forbidden)
  if (!fTokens.length) return null
  const n = fTokens.length
  const fJoined = fTokens.join('')
  let best = null
  const consider = (score, start, end) => {
    if (!best || score > best.score) best = {score, excerpt: questionTokens.slice(start, end).join(' ')}
  }

  for (let i = 0; i + n <= questionTokens.length; i++) {
    const window = questionTokens.slice(i, i + n)
    const perToken = Math.min(...window.map((t, k) => similarity(t, fTokens[k])))
    consider(Math.max(perToken, n > 1 ? similarity(window.join(''), fJoined) : 0), i, i + n)
  }
  // « spiderman » écrit d'un bloc pour « Spider-Man ».
  if (n > 1) {
    questionTokens.forEach((t, i) => consider(similarity(t, fJoined), i, i + 1))
  }
  return best
}

/**
 * @returns {{exact: Array<{word, excerpt}>, near: Array<{word, excerpt, score}>}}
 */
export function matchForbiddenWords(question, forbiddenWords) {
  const tokens = tokenize(question)
  const exact = []
  const near = []
  for (const word of forbiddenWords ?? []) {
    const match = tokens.length ? bestMatch(tokens, word) : null
    if (!match) continue
    if (match.score >= 1) exact.push({word, excerpt: match.excerpt})
    else if (match.score >= NEAR_THRESHOLD) near.push({word, excerpt: match.excerpt, score: match.score})
  }
  return {exact, near}
}
