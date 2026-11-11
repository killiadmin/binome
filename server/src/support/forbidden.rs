//! Détection des mots interdits dans une question (récap : trophées « Maladroit »
//! et « Permis de mentir »). Port exact de `frontend/src/services/forbiddenWords.js`,
//! seul détecteur utilisé pendant la partie : garder les deux synchronisés
//! (le test `parity_with_js` compare aux résultats du JS).
//!
//!  - exact (score 1)    : le mot, à la casse, aux accents et au pluriel près ;
//!  - proche (≥ 0,8)     : faute de frappe, conjugaison, dérivé.

use deunicode::deunicode;

pub const NEAR_THRESHOLD: f64 = 0.8;

const SUFFIXES: [&str; 33] = [
    "issements", "issement", "eraient", "assions", "erions", "eriez", "erons", "eront", "erais", "erait",
    "aient", "ement", "ments", "ment", "euse", "eurs", "ions", "eur", "ees", "iez", "ais", "ait", "ant",
    "ent", "es", "ee", "er", "ir", "ez", "ai", "e", "s", "x",
];

fn tokenize(text: &str) -> Vec<String> {
    deunicode(&text.to_lowercase())
        .to_lowercase()
        .split(|c: char| !c.is_ascii_lowercase() && !c.is_ascii_digit())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn singular(t: &str) -> &str {
    if t.len() > 3 && (t.ends_with('s') || t.ends_with('x')) { &t[..t.len() - 1] } else { t }
}

fn stem(t: &str) -> &str {
    for s in SUFFIXES {
        if t.ends_with(s) && t.len() - s.len() >= 3 {
            return &t[..t.len() - s.len()];
        }
    }
    t
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

fn similarity(q: &str, f: &str) -> f64 {
    if singular(q) == singular(f) {
        return 1.0;
    }
    if q.len() < 3 || f.len() < 3 {
        return 0.0;
    }
    let mut score = 1.0 - levenshtein(q, f) as f64 / q.len().max(f.len()) as f64;
    if stem(q) == stem(f) {
        score = score.max(0.9);
    }
    score
}

fn best_score(tokens: &[String], forbidden: &str) -> Option<f64> {
    let f = tokenize(forbidden);
    if f.is_empty() {
        return None;
    }
    let n = f.len();
    let joined = f.concat();
    let mut best: Option<f64> = None;
    let mut consider = |s: f64| best = Some(best.map_or(s, |b: f64| b.max(s)));

    for i in 0..tokens.len().saturating_sub(n - 1) {
        if i + n > tokens.len() {
            break;
        }
        let window = &tokens[i..i + n];
        let per_token = window.iter().zip(&f).map(|(t, w)| similarity(t, w)).fold(f64::INFINITY, f64::min);
        let whole = if n > 1 { similarity(&window.concat(), &joined) } else { 0.0 };
        consider(per_token.max(whole));
    }
    if n > 1 {
        for t in tokens {
            consider(similarity(t, &joined));
        }
    }
    best
}

/// Mots interdits trouvés : (exacts, proches).
pub fn matches(question: &str, forbidden: &[String]) -> (Vec<String>, Vec<String>) {
    let tokens = tokenize(question);
    let (mut exact, mut near) = (vec![], vec![]);
    if tokens.is_empty() {
        return (exact, near);
    }
    for word in forbidden {
        match best_score(&tokens, word) {
            Some(s) if s >= 1.0 => exact.push(word.clone()),
            Some(s) if s >= NEAR_THRESHOLD => near.push(word.clone()),
            _ => {}
        }
    }
    (exact, near)
}

/// La question contient-elle un mot interdit (exact ou proche) ?
pub fn matches_any(question: &str, forbidden: &[String]) -> bool {
    let (e, n) = matches(question, forbidden);
    !e.is_empty() || !n.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn readme_examples() {
        let peter_pan = words(&["costume", "poudre", "voler"]);
        assert_eq!(matches("Peut-il voler ?", &peter_pan).0, vec!["voler"]);
        let (e, n) = matches("Est-ce que ton personnage vol ?", &peter_pan);
        assert!(e.is_empty() && n == vec!["voler"]);
        assert_eq!(matches("Porte-t-il des COSTUMES ?", &peter_pan).0, vec!["costume"]);
        assert!(!matches_any("Est-il grand ?", &peter_pan));
    }

    #[test]
    fn compound_words() {
        let w = words(&["Spider-Man", "Pays imaginaire"]);
        assert_eq!(matches("C'est spiderman ?", &w).0, vec!["Spider-Man"]);
        assert_eq!(matches("Vit-il au pays imaginaire ?", &w).0, vec!["Pays imaginaire"]);
    }

    /// Vecteurs produits par le détecteur JS du front (server/testdata/forbidden-vectors.json, généré par tests/e2e/gen-forbidden-vectors.mjs).
    #[test]
    fn parity_with_js() {
        let raw = include_str!("../../testdata/forbidden-vectors.json");
        let cases: Vec<serde_json::Value> = serde_json::from_str(raw).unwrap();
        assert!(cases.len() > 50);
        for c in cases {
            let q = c["question"].as_str().unwrap();
            let w: Vec<String> = serde_json::from_value(c["words"].clone()).unwrap();
            let exp_e: Vec<String> = serde_json::from_value(c["exact"].clone()).unwrap();
            let exp_n: Vec<String> = serde_json::from_value(c["near"].clone()).unwrap();
            let (e, n) = matches(q, &w);
            assert_eq!((e, n), (exp_e, exp_n), "question « {q} », mots {w:?}");
        }
    }
}
