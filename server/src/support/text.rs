//! Normalisations de texte.

use deunicode::deunicode;

/// Clé de comparaison insensible à la casse, aux accents et aux espaces finaux
/// (équivalent pratique de la collation MySQL `utf8mb4_unicode_ci`).
/// Sert à la collation SQLite `BINOME_CI`.
pub fn ci_key(s: &str) -> String {
    deunicode(s).to_lowercase().trim_end().to_string()
}

/// `Str::of($s)->ascii()->lower()->squish()` : ASCII, minuscules, espaces réduits.
pub fn squish_ascii(s: &str) -> String {
    deunicode(s).to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `Str::slug()` : ASCII, minuscules, séparateurs `-`.
pub fn slug(s: &str) -> String {
    let ascii = deunicode(&s.replace('@', " at ")).to_lowercase();
    let mut out = String::with_capacity(ascii.len());
    let mut dash = false;
    for c in ascii.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        assert_eq!(ci_key("Killián "), ci_key("killian"));
        assert_ne!(ci_key("Killian"), ci_key("Kilian"));
        assert_eq!(squish_ascii("  Iron   Mán "), "iron man");
        assert_eq!(slug("Élodie Test !"), "elodie-test");
        assert_eq!(slug("Spider-Man (2002)"), "spider-man-2002");
        assert_eq!(slug("Animé Japonais"), "anime-japonais");
    }
}
