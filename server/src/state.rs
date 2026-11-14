//! État partagé par toutes les requêtes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::db::Db;
use crate::ratelimit::RateLimiter;
use crate::realtime::hub::{Event, Hub};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub hub: Arc<Hub>,
    pub cfg: Arc<Config>,
    pub limiter: Arc<RateLimiter>,
    /// Clé de signature des jetons de profil (table `app_secrets`).
    pub profile_secret: Arc<Vec<u8>>,
    /// Dernier jour (n° depuis l'époque Unix) où l'activité de chaque joueur a été
    /// écrite (`players.last_seen_at`) : au plus une écriture par joueur et par jour.
    pub last_seen: Arc<Mutex<HashMap<i64, i64>>>,
}

impl AppState {
    pub fn publish(&self, events: &[Event]) {
        self.hub.publish_all(events);
    }

    /// Préfixe public des URLs d'images (`/binome`).
    pub fn prefix(&self) -> String {
        self.cfg.public_prefix.clone()
    }
}
