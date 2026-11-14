//! En-têtes de sécurité posés sur toutes les réponses (API, WebSocket, front
//! statique, repli SPA) et collecte des rapports CSP.
//!
//! Sur Railway il n'y a pas de Caddy devant le serveur : c'est donc ici que
//! vivent ces en-têtes. Le serveur de dev Vite (:5173) sert lui-même le front et
//! n'est pas concerné.

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::state::AppState;

/// `false` : CSP en `Report-Only` (rien n'est bloqué, les violations sont
/// envoyées à `/api/csp-report` et journalisées). Passer à `true` une fois les
/// journaux propres sur de vrais téléphones.
const CSP_ENFORCE: bool = false;

/// Rapports CSP acceptés par minute, tous clients confondus (au-delà : ignorés).
const CSP_REPORTS_PER_MINUTE: usize = 30;
/// Taille maximale d'un rapport lu.
const CSP_REPORT_MAX_BYTES: usize = 16 * 1024;

/// Politique de contenu du front :
/// - `blob:` : aperçus des photos recadrées (`URL.createObjectURL`) ;
/// - `data:` : petites images SVG intégrées par Bootstrap ;
/// - `connect-src 'self'` couvre l'API et le WebSocket de même origine (ws/wss).
fn csp(prefix: &str) -> String {
    format!(
        "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; \
         media-src 'self' blob:; font-src 'self'; connect-src 'self'; object-src 'none'; \
         base-uri 'self'; form-action 'self'; frame-ancestors 'none'; \
         report-uri {prefix}/api/csp-report"
    )
}

/// Pose les en-têtes de sécurité. `html` : la CSP ne concerne que les documents.
pub fn apply(headers: &mut HeaderMap, html: bool, hsts: bool, prefix: &str) {
    let fixed: [(HeaderName, &str); 4] = [
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
        // Caméra autorisée : photos de profil (usePhotoCapture.js).
        (HeaderName::from_static("permissions-policy"), "camera=(self), microphone=(), geolocation=()"),
    ];
    for (name, value) in fixed {
        headers.insert(name, HeaderValue::from_static(value));
    }
    if hsts {
        headers.insert(header::STRICT_TRANSPORT_SECURITY, HeaderValue::from_static("max-age=31536000"));
    }
    if html {
        let name = if CSP_ENFORCE { header::CONTENT_SECURITY_POLICY } else { header::CONTENT_SECURITY_POLICY_REPORT_ONLY };
        if let Ok(value) = HeaderValue::from_str(&csp(prefix)) {
            headers.insert(name, value);
        }
    }
}

/// Middleware appliqué au routeur complet (préfixe compris).
pub async fn headers(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let html = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    apply(res.headers_mut(), html, st.cfg.hsts, &st.prefix());
    res
}

/// `POST /api/csp-report` : rapport de violation envoyé par le navigateur
/// (`application/csp-report`, ou `application/reports+json` de l'API Reporting).
/// Journalisé en avertissement ; toujours 204, même illisible.
pub async fn report(State(st): State<AppState>, req: Request<Body>) -> Response {
    if !st.limiter.hit("csp-report", "global", CSP_REPORTS_PER_MINUTE) {
        return StatusCode::NO_CONTENT.into_response();
    }
    if let Ok(bytes) = to_bytes(req.into_body(), CSP_REPORT_MAX_BYTES).await {
        if let Ok(json) = serde_json::from_slice::<Value>(&bytes) {
            for line in summarize(&json) {
                tracing::warn!("CSP : {line}");
            }
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Lignes lisibles d'un rapport CSP, quel que soit son format.
fn summarize(json: &Value) -> Vec<String> {
    let one = |r: &Value, keys: [&str; 5]| -> String {
        let field = |k: &str| match r.get(k) {
            Some(Value::String(s)) => truncate(s),
            Some(Value::Number(n)) => n.to_string(),
            _ => "?".into(),
        };
        let [directive, blocked, document, source, line] = keys;
        format!("violation {} : {} sur {} ({}:{})", field(directive), field(blocked), field(document), field(source), field(line))
    };
    let legacy = ["effective-directive", "blocked-uri", "document-uri", "source-file", "line-number"];
    let modern = ["effectiveDirective", "blockedURL", "documentURL", "sourceFile", "lineNumber"];
    match json {
        Value::Object(o) => o.get("csp-report").map(|r| vec![one(r, legacy)]).unwrap_or_default(),
        Value::Array(list) => list
            .iter()
            .filter(|r| r.get("type").and_then(Value::as_str) == Some("csp-violation"))
            .filter_map(|r| r.get("body"))
            .take(10)
            .map(|b| one(b, modern))
            .collect(),
        _ => Vec::new(),
    }
}

/// Champ fourni par le client : sans caractères de contrôle (journal sur une ligne), borné.
fn truncate(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn get<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
        h.get(name).and_then(|v| v.to_str().ok())
    }

    #[test]
    fn common_headers_on_every_response() {
        let mut h = HeaderMap::new();
        apply(&mut h, false, false, "/binome");
        assert_eq!(get(&h, "x-content-type-options"), Some("nosniff"));
        assert_eq!(get(&h, "x-frame-options"), Some("DENY"));
        assert_eq!(get(&h, "referrer-policy"), Some("strict-origin-when-cross-origin"));
        assert!(get(&h, "permissions-policy").unwrap().contains("camera=(self)"));
        // Ni HSTS sans l'option, ni CSP hors HTML
        assert!(h.get("strict-transport-security").is_none());
        assert!(h.get("content-security-policy").is_none());
        assert!(h.get("content-security-policy-report-only").is_none());
    }

    #[test]
    fn hsts_only_when_enabled() {
        let mut h = HeaderMap::new();
        apply(&mut h, false, true, "");
        assert_eq!(get(&h, "strict-transport-security"), Some("max-age=31536000"));
    }

    #[test]
    fn csp_on_html_with_prefixed_report_uri() {
        let name = if CSP_ENFORCE { "content-security-policy" } else { "content-security-policy-report-only" };
        let mut h = HeaderMap::new();
        apply(&mut h, true, false, "/binome");
        let csp = get(&h, name).unwrap();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("img-src 'self' data: blob:"));
        assert!(csp.ends_with("report-uri /binome/api/csp-report"));

        let mut h = HeaderMap::new();
        apply(&mut h, true, false, "");
        assert!(get(&h, name).unwrap().ends_with("report-uri /api/csp-report"));
    }

    #[test]
    fn summarizes_both_report_formats() {
        let legacy = json!({"csp-report": {
            "document-uri": "https://x/rooms", "effective-directive": "style-src-attr",
            "blocked-uri": "inline", "source-file": "https://x/assets/a.js", "line-number": 12
        }});
        assert_eq!(summarize(&legacy), vec!["violation style-src-attr : inline sur https://x/rooms (https://x/assets/a.js:12)"]);

        let modern = json!([
            {"type": "csp-violation", "body": {"documentURL": "https://x/", "effectiveDirective": "img-src", "blockedURL": "https://evil/p.png"}},
            {"type": "deprecation", "body": {}}
        ]);
        assert_eq!(summarize(&modern), vec!["violation img-src : https://evil/p.png sur https://x/ (?:?)"]);
        assert!(summarize(&json!("n'importe quoi")).is_empty());
    }
}
