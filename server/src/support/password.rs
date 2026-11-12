//! Mots de passe d'administration et de suppression des parties : seule leur
//! empreinte est stockée (BINOME_ADMIN_PASSWORD_HASH, BINOME_DELETE_PASSWORD_HASH),
//! jamais le mot de passe lui-même.
//!
//! Format : `pbkdf2-sha256:<itérations>:<sel hex>:<empreinte hex>` — sans `$`
//! (Docker Compose l'interpréterait dans un fichier .env).
//! PBKDF2-HMAC-SHA256 (RFC 8018), un seul bloc de 32 octets, construit sur
//! `hmac` + `sha2` déjà présents : aucune dépendance de plus.

use hmac::{Hmac, KeyInit, Mac};
use rand::RngExt;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Coût recommandé (OWASP 2023 pour PBKDF2-HMAC-SHA256).
pub const DEFAULT_ITERATIONS: u32 = 600_000;
const PREFIX: &str = "pbkdf2-sha256";

fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let prf = Hmac::<Sha256>::new_from_slice(password).expect("clé HMAC de toute longueur");
    let mut first = prf.clone();
    first.update(salt);
    first.update(&1u32.to_be_bytes());
    let mut u: [u8; 32] = first.finalize().into_bytes().into();
    let mut out = u;
    for _ in 1..iterations {
        let mut mac = prf.clone();
        mac.update(&u);
        u = mac.finalize().into_bytes().into();
        for (o, x) in out.iter_mut().zip(u) {
            *o ^= x;
        }
    }
    out
}

/// Empreinte d'un nouveau mot de passe (sel aléatoire de 16 octets).
pub fn hash(password: &str, iterations: u32) -> String {
    let mut salt = [0u8; 16];
    rand::rng().fill(&mut salt);
    let digest = pbkdf2_sha256(password.as_bytes(), &salt, iterations);
    format!("{PREFIX}:{iterations}:{}:{}", hex::encode(salt), hex::encode(digest))
}

/// Le mot de passe correspond-il à l'empreinte ? (comparaison à temps constant)
/// Une empreinte mal formée ne correspond à rien.
pub fn verify(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.trim().split(':').collect();
    let [prefix, iterations, salt, expected] = parts.as_slice() else { return false };
    let (Ok(iterations), Ok(salt), Ok(expected)) = (iterations.parse::<u32>(), hex::decode(salt), hex::decode(expected)) else {
        return false;
    };
    if *prefix != PREFIX || iterations == 0 || expected.len() != 32 {
        return false;
    }
    let digest = pbkdf2_sha256(password.as_bytes(), &salt, iterations);
    bool::from(digest.as_slice().ct_eq(&expected))
}

/// L'empreinte est-elle exploitable ? (contrôle au démarrage)
pub fn is_well_formed(stored: &str) -> bool {
    let parts: Vec<&str> = stored.trim().split(':').collect();
    matches!(parts.as_slice(), [p, i, s, h] if *p == PREFIX && i.parse::<u32>().is_ok_and(|n| n > 0)
        && hex::decode(s).is_ok() && hex::decode(h).is_ok_and(|d| d.len() == 32))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vecteurs PBKDF2-HMAC-SHA256 de référence (RFC 7914 §11, 32 premiers octets).
    #[test]
    fn rfc_vectors() {
        assert_eq!(hex::encode(pbkdf2_sha256(b"passwd", b"salt", 1)), "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc");
        assert_eq!(hex::encode(pbkdf2_sha256(b"Password", b"NaCl", 80_000)), "4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab56");
    }

    #[test]
    fn hash_and_verify() {
        let h = hash("mot de passe très secret", 1000);
        assert!(is_well_formed(&h) && !h.contains('$'));
        assert!(verify("mot de passe très secret", &h));
        assert!(!verify("mot de passe tres secret", &h));
        assert!(!verify("", &h));
        assert!(!verify("x", "pbkdf2-sha256:0:00:00"));
        assert!(!verify("x", "n'importe quoi"));
        assert_ne!(hash("même", 1000), hash("même", 1000)); // sel aléatoire
    }
}
