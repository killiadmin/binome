//! État partagé par toutes les requêtes.

use std::sync::Arc;

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
