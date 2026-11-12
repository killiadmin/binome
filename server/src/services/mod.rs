//! Logique métier. Chaque opération s'exécute dans UNE transaction et renvoie
//! un [`Out`] : statut + corps HTTP, events à diffuser **après le commit**, et
//! suites à donner (minuteries). Un refus qui doit malgré tout valider des
//! effets (tirage expiré clos puis refusé) renvoie simplement un statut d'erreur
//! dans `Out` au lieu d'une `AppError` (qui, elle, annule la transaction).

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::realtime::hub::Event;

pub mod admin;
pub mod catalog;
pub mod common;
pub mod game;
pub mod history;
pub mod play;
pub mod profile;
pub mod rooms;
pub mod score;
pub mod social;

/// Suite à donner après le commit.
#[derive(Debug, Clone)]
pub enum Followup {
    /// Clore le tirage de carte de cette action à l'échéance (`card_draw_deadline`).
    ExpireCardDraw { action_id: i64, deadline: String },
}

#[derive(Debug)]
pub struct Out {
    pub status: StatusCode,
    pub body: Value,
    pub events: Vec<Event>,
    pub followups: Vec<Followup>,
}

impl Out {
    pub fn ok(body: Value) -> Self {
        Self { status: StatusCode::OK, body, events: vec![], followups: vec![] }
    }
    pub fn created(body: Value) -> Self {
        Self { status: StatusCode::CREATED, ..Self::ok(body) }
    }
    pub fn message(msg: &str) -> Self {
        Self::ok(json!({ "message": msg }))
    }
    pub fn status(mut self, s: StatusCode) -> Self {
        self.status = s;
        self
    }
    pub fn with(mut self, events: Vec<Event>) -> Self {
        self.events.extend(events);
        self
    }
    pub fn event(mut self, e: Event) -> Self {
        self.events.push(e);
        self
    }
}

/// Accumulateur d'events pendant une opération.
pub type Events = Vec<Event>;
