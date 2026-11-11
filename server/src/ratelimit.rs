//! Limites de débit en mémoire (fenêtre glissante d'une minute).
//!
//! Indexées sur l'identité du joueur, jamais sur l'IP : derrière le proxy, tous
//! les joueurs du réseau local arrivent avec la même adresse.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct RateLimiter {
    hits: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl RateLimiter {
    /// La limite est-elle atteinte (sans rien enregistrer) ?
    pub fn is_limited(&self, bucket: &str, key: &str, per_minute: usize) -> bool {
        let now = Instant::now();
        let mut map = self.hits.lock().unwrap_or_else(|p| p.into_inner());
        let Some(q) = map.get_mut(&format!("{bucket}|{key}")) else { return false };
        while q.front().is_some_and(|t| now.duration_since(*t) >= WINDOW) {
            q.pop_front();
        }
        q.len() >= per_minute
    }

    /// Enregistre un événement (ex. un échec) sans contrôle.
    pub fn record(&self, bucket: &str, key: &str) {
        let mut map = self.hits.lock().unwrap_or_else(|p| p.into_inner());
        map.entry(format!("{bucket}|{key}")).or_default().push_back(Instant::now());
    }

    /// Enregistre une tentative ; `false` si la limite `per_minute` est atteinte.
    pub fn hit(&self, bucket: &str, key: &str, per_minute: usize) -> bool {
        let now = Instant::now();
        let mut map = self.hits.lock().unwrap_or_else(|p| p.into_inner());
        // Ménage occasionnel des clés inactives
        if map.len() > 10_000 {
            map.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) < WINDOW));
        }
        let q = map.entry(format!("{bucket}|{key}")).or_default();
        while q.front().is_some_and(|t| now.duration_since(*t) >= WINDOW) {
            q.pop_front();
        }
        if q.len() >= per_minute {
            return false;
        }
        q.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_per_key() {
        let l = RateLimiter::default();
        for _ in 0..20 {
            assert!(l.hit("chat", "a:1", 20));
        }
        assert!(!l.hit("chat", "a:1", 20));
        assert!(l.hit("chat", "a:2", 20));
        assert!(l.hit("reactions", "a:1", 20));
    }

    #[test]
    fn counts_only_recorded_failures() {
        let l = RateLimiter::default();
        assert!(!l.is_limited("delete", "admin", 2));
        l.record("delete", "admin");
        assert!(!l.is_limited("delete", "admin", 2));
        l.record("delete", "admin");
        assert!(l.is_limited("delete", "admin", 2));
    }
}
