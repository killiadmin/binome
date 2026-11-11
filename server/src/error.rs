//! Erreurs de l'application et leur rendu HTTP, au format attendu par le front
//! (celui de Laravel) :
//!   422 → { message, errors: { champ: [messages] } }
//!   403 / 404 / 409 / 429 → { message }
//! Jamais de pile d'appels ni de détail interne dans la réponse.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Map, Value};

#[derive(Debug)]
pub enum AppError {
    /// Erreurs de validation, dans l'ordre des champs (422).
    Validation(Vec<(String, String)>),
    /// Refus avec message, statut choisi (403, 409, 422 « métier »…).
    Status(StatusCode, String),
    /// Ressource inexistante (404).
    NotFound,
    /// Limite de débit dépassée (429).
    TooManyRequests,
    /// Erreur interne : journalisée, jamais détaillée au client (500).
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// Refus d'une règle du jeu (409), équivalent de GameRuleException.
    pub fn rule(msg: impl Into<String>) -> Self {
        Self::Status(StatusCode::CONFLICT, msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Status(StatusCode::FORBIDDEN, msg.into())
    }
    /// 422 avec un simple message (refus métier rendu en 422 par Laravel).
    pub fn unprocessable(msg: impl Into<String>) -> Self {
        Self::Status(StatusCode::UNPROCESSABLE_ENTITY, msg.into())
    }
    /// 422 portant sur un champ (format validation).
    pub fn field(field: &str, msg: impl Into<String>) -> Self {
        Self::Validation(vec![(field.to_string(), msg.into())])
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Internal(format!("sqlite: {e}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Validation(errors) => {
                let mut fields: Map<String, Value> = Map::new();
                for (field, msg) in &errors {
                    fields
                        .entry(field.clone())
                        .or_insert_with(|| Value::Array(vec![]))
                        .as_array_mut()
                        .expect("tableau")
                        .push(Value::String(msg.clone()));
                }
                // Laravel : « premier message (and N more errors) »
                let first = errors.first().map(|(_, m)| m.clone()).unwrap_or_default();
                let more = errors.len().saturating_sub(1);
                let message = match more {
                    0 => first,
                    1 => format!("{first} (and 1 more error)"),
                    n => format!("{first} (and {n} more errors)"),
                };
                (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "message": message, "errors": fields }))).into_response()
            }
            AppError::Status(code, message) => (code, Json(json!({ "message": message }))).into_response(),
            AppError::NotFound => (StatusCode::NOT_FOUND, Json(json!({ "message": "Not Found" }))).into_response(),
            AppError::TooManyRequests => {
                (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "message": "Too Many Attempts." }))).into_response()
            }
            AppError::Internal(detail) => {
                tracing::error!("erreur interne : {detail}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "message": "Server Error" }))).into_response()
            }
        }
    }
}
