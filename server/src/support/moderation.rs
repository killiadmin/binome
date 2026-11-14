//! Filtre des insultes et injures en français (boîte à idées).
//!
//! Le texte est découpé en mots normalisés : minuscules, sans accents, chiffres
//! et symboles lus comme des lettres (`c0nn4rd`, `s@lope`), lettres étirées
//! ramenées (`connnnard`, `salooope`), lettres espacées recollées (`p u t e`,
//! `e.n.c.u.l.e`). Trois façons de comparer, pour éviter les faux positifs :
//!
//!  - mot exact (au pluriel près) pour les termes courts ou ambigus : `con` ne
//!    doit pas bloquer « contre », `pute` « réputation », `bite` « habite » ;
//!  - début de mot pour les racines sans ambiguïté (`encul…`, `connard…`) ;
//!  - expressions (« fils de pute », « ta gueule »), pour les mots anodins seuls.
//!
//! Les jurons d'exclamation (« merde », « putain ») ne sont pas des insultes et
//! passent. Un filtre se contourne toujours : la suppression par un
//! administrateur reste le dernier recours.

use deunicode::deunicode;

/// Termes refusés comme mot entier (après normalisation, pluriel compris).
const EXACT: &[&str] = &[
    // Insultes
    "con", "conne", "pute", "fdp", "ntm", "tg", "abruti", "abrutie", "cretin", "cretine", "imbecile",
    "gogol", "triso", "boloss", "bolos", "trouduc", "niquer", "niquez", "teub", "zob", "bite",
    // Injures homophobes, racistes, antisémites
    "pd", "pede", "gouine", "travelo", "negre", "negresse", "negro", "bicot", "youpin", "youtre",
    "chinetoque", "chintok", "niakoue", "bamboula",
];

/// Racines refusées en début de mot (enculé, enculer, connards, salopard…).
const PREFIX: &[&str] = &[
    "encul", "connard", "connass", "salop", "salaud", "enfoir", "batard", "petass", "pouffias", "poufias",
    "grognass", "branleu", "tarlouz", "tafiol", "bougnoul", "suceus", "suceur",
];

/// Mots légitimes qui commencent par une racine refusée.
const ALLOW: &[&str] = &["salopette"];

/// Expressions refusées (suite de mots normalisés).
const PHRASES: &[&[&str]] = &[
    &["fils", "de", "pute"],
    &["nique", "ta"],
    &["nique", "sa"],
    &["nique", "ton"],
    &["nique", "tes"],
    &["ta", "gueule"],
    &["te", "faire", "foutre"],
    &["trou", "du", "cul"],
    &["leche", "cul"],
    &["tete", "de", "noeud"],
    &["suce", "ma"],
    &["sous", "merde"],
    &["sale", "juif"],
    &["sale", "juive"],
    &["sale", "arabe"],
    &["sale", "noir"],
    &["sale", "noire"],
    &["sale", "gay"],
    &["sale", "race"],
];

/// Chiffres et symboles utilisés comme lettres.
fn leet(c: char) -> char {
    match c {
        '0' => 'o',
        '1' => 'i',
        '3' => 'e',
        '4' | '@' => 'a',
        '5' | '$' => 's',
        '7' => 't',
        _ => c,
    }
}

fn tokens(text: &str) -> Vec<String> {
    let mapped: String = text.chars().map(leet).collect();
    deunicode(&mapped)
        .to_lowercase()
        .split(|c: char| !c.is_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Ramène à `keep` lettres toute suite d'au moins 3 lettres identiques.
fn squeeze(t: &str, keep: usize) -> String {
    let chars: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    let mut i = 0;
    while i < chars.len() {
        let mut j = i;
        while j < chars.len() && chars[j] == chars[i] {
            j += 1;
        }
        let run = j - i;
        let n = if run >= 3 { keep } else { run };
        for _ in 0..n {
            out.push(chars[i]);
        }
        i = j;
    }
    out
}

fn singular(t: &str) -> &str {
    if t.len() >= 4 && (t.ends_with('s') || t.ends_with('x')) { &t[..t.len() - 1] } else { t }
}

fn word_is_insult(word: &str) -> bool {
    [word.to_string(), squeeze(word, 1), squeeze(word, 2)].iter().any(|w| {
        let w = w.as_str();
        if ALLOW.iter().any(|a| singular(w) == *a) {
            return false;
        }
        EXACT.iter().any(|e| w == *e || singular(w) == *e) || PREFIX.iter().any(|p| w.starts_with(p))
    })
}

/// Le texte contient-il une insulte ou une injure ?
pub fn contains_insult(text: &str) -> bool {
    let words = tokens(text);
    if words.iter().any(|w| word_is_insult(w)) {
        return true;
    }
    // Lettres espacées (« p u t e », « c.o.n ») : au moins 3 lettres seules d'affilée
    let mut run = String::new();
    let mut run_len = 0;
    for w in words.iter().map(String::as_str).chain(std::iter::once("--")) {
        if w.len() == 1 {
            run.push_str(w);
            run_len += 1;
            continue;
        }
        if run_len >= 3 && word_is_insult(&run) {
            return true;
        }
        run.clear();
        run_len = 0;
    }
    // Expressions, lettres étirées ramenées à une seule
    let flat: Vec<String> = words.iter().map(|w| squeeze(w, 1)).collect();
    PHRASES.iter().any(|p| flat.windows(p.len()).any(|win| win.iter().zip(p.iter()).all(|(a, b)| a.as_str() == *b || singular(a) == *b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insults_are_caught() {
        for text in [
            "T'es qu'un con",
            "bande de CONS",
            "Espèce de connard",
            "les connards du lobby",
            "Enculé !",
            "va te faire enculer",
            "sale pute",
            "fils de pute",
            "FDP",
            "nique ta mère",
            "Ta gueule",
            "ferme ta gueuuule",
            "salope",
            "quelle salooooope",
            "connnnnard",
            "c0nn4rd",
            "s@lope",
            "3ncul3",
            "p u t e",
            "e.n.c.u.l.é",
            "c-o-n",
            "un gros pédé",
            "sale PD",
            "trou du cul",
            "tête de nœud",
            "bâtard",
            "pétasse",
            "sale arabe",
        ] {
            assert!(contains_insult(text), "aurait dû être refusé : {text}");
        }
    }

    #[test]
    fn ordinary_french_passes() {
        for text in [
            "Ajouter un mode contre-la-montre",
            "Un concours mensuel avec un classement",
            "Mon conseil : afficher le score",
            "Pouvoir choisir son personnage",
            "Une meilleure réputation pour les joueurs fair-play",
            "Disputer une revanche sans recréer le salon",
            "La personne qui habite le plus loin commence",
            "Le calcul des points est flou",
            "Une salopette pour l'avatar",
            "Un pique-nique entre binômes",
            "Plus de personnages de dessins animés",
            "Le bouton est trop petit sur mobile, merde",
            "Afficher un récap pédagogique en fin de partie",
            "Garder le chat ouvert pendant la partie",
            "Ajouter un mode à 3 équipes de 2",
            "Le jeu est génial !!! Bravo",
            "Il y a un bug au tour 10",
            "Pouvoir noircir ses notes",
            "La sale manie du timer",
        ] {
            assert!(!contains_insult(text), "aurait dû passer : {text}");
        }
    }

    #[test]
    fn squeeze_runs() {
        assert_eq!(squeeze("connnnard", 2), "connard");
        assert_eq!(squeeze("salooope", 1), "salope");
        assert_eq!(squeeze("connard", 1), "connard"); // les doubles restent
    }
}
