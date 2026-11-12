//! Routes HTTP. Chaque handler : lecture des entrées → service dans une
//! transaction → publication des events APRÈS le commit → réponse.

pub mod input;

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use rusqlite::{Connection, Transaction};
use tower_http::services::{ServeDir, ServeFile};

use crate::error::{AppError, AppResult};
use crate::realtime::ws;
use crate::services::common::Env;
use crate::services::{admin, catalog, game, history, play, rooms, score, social, Followup, Out};
use crate::state::AppState;
use crate::timers;
use input::Input;

type Params = Path<HashMap<String, String>>;

/// Paramètre de route numérique ; non numérique → 404 (comme une liaison de modèle).
fn id(p: &HashMap<String, String>, name: &str) -> AppResult<i64> {
    p.get(name).and_then(|v| v.parse().ok()).ok_or(AppError::NotFound)
}

impl AppState {
    fn env(&self) -> Env {
        Env { prefix: self.prefix(), hub: self.hub.clone() }
    }

    /// Réponse d'une opération : publication des events et suites, puis JSON.
    fn finish(&self, out: Out) -> Response {
        self.publish(&out.events);
        for f in out.followups {
            match f {
                Followup::ExpireCardDraw { action_id, deadline } => timers::schedule_card_expiry(self.clone(), action_id, &deadline),
            }
        }
        (out.status, Json(out.body)).into_response()
    }

    async fn write<F>(&self, f: F) -> Response
    where
        F: FnOnce(&Transaction, &Env) -> AppResult<Out> + Send + 'static,
    {
        let env = self.env();
        match self.db.write(move |tx| f(tx, &env)).await {
            Ok(out) => self.finish(out),
            Err(e) => e.into_response(),
        }
    }

    async fn read<F>(&self, f: F) -> Response
    where
        F: FnOnce(&Connection, &Env) -> AppResult<Out> + Send + 'static,
    {
        let env = self.env();
        match self.db.read(move |c| f(c, &env)).await {
            Ok(out) => self.finish(out),
            Err(e) => e.into_response(),
        }
    }

    /// Limite de débit ; `None` si autorisé.
    fn throttle(&self, bucket: &str, key: &str, per_minute: usize) -> Option<Response> {
        (!self.limiter.hit(bucket, key, per_minute)).then(|| AppError::TooManyRequests.into_response())
    }
}

fn raw_player_key(input: &Input) -> String {
    format!("p:{}", input.get("player_id").map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string)).unwrap_or_default())
}

/// Image binaire en cache long (URL versionnée `?v=`).
fn image_response(mime: &str, bytes: Vec<u8>) -> Response {
    let mut res = Response::new(Body::from(bytes));
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_str(mime).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    res
}

// ─── Middleware admin ───────────────────────────────────────────────────────

async fn require_admin(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let provided = req.headers().get("x-characters-access").and_then(|v| v.to_str().ok());
    match catalog::check_access(st.cfg.admin_password_hash.as_deref(), provided) {
        Ok(()) => next.run(req).await,
        Err(e) => e.into_response(),
    }
}

// ─── Salons ─────────────────────────────────────────────────────────────────

async fn rooms_store(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, env| rooms::store(tx, env, &input)).await
}
async fn rooms_join(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, env| rooms::join(tx, env, &input)).await
}
async fn rooms_show(State(st): State<AppState>, Path(p): Params) -> Response {
    st.read(move |c, env| rooms::show(c, env, id(&p, "room")?)).await
}
async fn rooms_settings(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| rooms::settings(tx, id(&p, "room")?, &input)).await
}
async fn rooms_ready(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, env| rooms::ready(tx, env, id(&p, "room")?, &input)).await
}
async fn rooms_leave(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, env| rooms::leave(tx, env, id(&p, "room")?, &input)).await
}
async fn rooms_start(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, env| game::start(tx, env, id(&p, "room")?, &input)).await
}

// ─── Parties ────────────────────────────────────────────────────────────────

async fn games_show(State(st): State<AppState>, Path(p): Params) -> Response {
    st.read(move |c, env| game::show(c, env, id(&p, "game")?)).await
}
async fn games_me(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.read(move |c, env| game::me(c, env, id(&p, "game")?, &input)).await
}
async fn games_recap(State(st): State<AppState>, Path(p): Params) -> Response {
    st.read(move |c, env| score::recap(c, env, id(&p, "game")?)).await
}
async fn games_reveal(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.read(move |c, env| game::reveal(c, env, id(&p, "game")?, &input)).await
}
async fn games_question_vote(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| score::vote_question(tx, id(&p, "game")?, &input)).await
}
async fn games_character_vote(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, env| game::vote_character(tx, env, id(&p, "game")?, &input)).await
}
async fn games_react(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    if let Some(r) = st.throttle("reactions", &raw_player_key(&input), 30) {
        return r;
    }
    st.write(move |tx, env| social::react(tx, env, id(&p, "game")?, &input)).await
}
async fn games_skip(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, env| play::skip_turn(tx, env, id(&p, "game")?, &input)).await
}
async fn games_exclude(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, env| play::exclude(tx, env, id(&p, "game")?, id(&p, "player")?, &input)).await
}
async fn games_correct(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| play::correct_answer(tx, id(&p, "game")?, id(&p, "action")?, &input)).await
}
async fn rounds_question(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| play::question(tx, id(&p, "game")?, id(&p, "round")?, &input)).await
}
async fn rounds_accusation(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| play::accusation(tx, id(&p, "game")?, id(&p, "round")?, &input)).await
}
async fn actions_answer(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| play::answer(tx, id(&p, "game")?, id(&p, "round")?, id(&p, "action")?, &input)).await
}
async fn actions_confirm(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| play::confirm(tx, id(&p, "game")?, id(&p, "round")?, id(&p, "action")?, &input)).await
}
async fn actions_draw(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| play::draw(tx, id(&p, "game")?, id(&p, "round")?, id(&p, "action")?, &input)).await
}
async fn actions_decide(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| play::decide(tx, id(&p, "game")?, id(&p, "round")?, id(&p, "action")?, &input)).await
}

// ─── Historique, chat, images, réactions ────────────────────────────────────

async fn history_games(State(st): State<AppState>) -> Response {
    st.read(|c, _| history::list_games(c, true)).await
}
async fn history_game(State(st): State<AppState>, Path(p): Params) -> Response {
    st.read(move |c, env| history::detail(c, env, id(&p, "game")?, true)).await
}
async fn history_leaderboard(State(st): State<AppState>) -> Response {
    st.read(|c, env| history::leaderboard(c, env)).await
}
async fn history_player(State(st): State<AppState>, input: Input) -> Response {
    st.read(move |c, _| history::player_breakdown(c, &input)).await
}

async fn chat_index(State(st): State<AppState>) -> Response {
    st.write(|tx, _| social::chat_index(tx)).await
}
async fn chat_store(State(st): State<AppState>, input: Input) -> Response {
    if let Some(r) = st.throttle("chat", &social::chat_limit_key(&input), 20) {
        return r;
    }
    st.write(move |tx, _| social::chat_store(tx, &input)).await
}

async fn avatar_show(State(st): State<AppState>, Path(p): Params) -> Response {
    match st.db.read(move |c| social::avatar_image(c, id(&p, "avatar")?)).await {
        Ok(bytes) => image_response("image/jpeg", bytes),
        Err(e) => e.into_response(),
    }
}
async fn avatar_update(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    if let Some(r) = st.throttle("avatars", &format!("p:{}", p.get("player").cloned().unwrap_or_default()), 10) {
        return r;
    }
    st.write(move |tx, env| social::avatar_update(tx, env, id(&p, "player")?, &input)).await
}
async fn avatar_destroy(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    if let Some(r) = st.throttle("avatars", &format!("p:{}", p.get("player").cloned().unwrap_or_default()), 10) {
        return r;
    }
    st.write(move |tx, _| social::avatar_destroy(tx, id(&p, "player")?, &input)).await
}
async fn character_image(State(st): State<AppState>, Path(p): Params) -> Response {
    match st.db.read(move |c| catalog::character_image(c, id(&p, "character")?)).await {
        Ok((mime, bytes)) => image_response(&mime, bytes),
        Err(e) => e.into_response(),
    }
}

async fn reactions_index(State(st): State<AppState>) -> Response {
    st.read(|c, env| social::reactions_index(c, env)).await
}
async fn reactions_store(State(st): State<AppState>, input: Input) -> Response {
    if let Some(r) = st.throttle("custom-reactions", &raw_player_key(&input), 5) {
        return r;
    }
    st.write(move |tx, env| social::reaction_store(tx, env, &input)).await
}
async fn reactions_image(State(st): State<AppState>, Path(p): Params) -> Response {
    match st.db.read(move |c| social::reaction_image(c, id(&p, "reaction")?)).await {
        Ok(bytes) => image_response("image/jpeg", bytes),
        Err(e) => e.into_response(),
    }
}

// ─── Catalogue et administration ────────────────────────────────────────────

/// Connexion admin : au plus 2 tentatives par minute, tous clients confondus
/// (ni player_id ni IP : falsifiables, ou identiques derrière le portail).
/// La tentative est réservée avant la vérification (PBKDF2, hors des threads
/// réseau) puis rendue si elle réussit : seuls les échecs comptent.
async fn access_login(State(st): State<AppState>, input: Input) -> Response {
    const BUCKET: (&str, &str) = ("admin-login", "tentatives");
    let given = match catalog::login_input(&input) {
        Ok(g) => g,
        Err(e) => return e.into_response(),
    };
    if !st.limiter.hit(BUCKET.0, BUCKET.1, 2) {
        tracing::warn!("connexion admin refusée : trop de tentatives");
        return AppError::TooManyRequests.into_response();
    }
    let stored = st.cfg.admin_password_hash.clone();
    let result = tokio::task::spawn_blocking(move || catalog::login(stored.as_deref(), &given))
        .await
        .unwrap_or_else(|e| Err(AppError::Internal(format!("vérification du mot de passe admin : {e}"))));
    match result {
        Ok(out) => {
            st.limiter.release(BUCKET.0, BUCKET.1);
            st.finish(out)
        }
        Err(e) => {
            tracing::warn!("connexion admin refusée : mot de passe incorrect");
            e.into_response()
        }
    }
}
async fn cosmos_index(State(st): State<AppState>) -> Response {
    st.read(|c, _| catalog::cosmos_index(c)).await
}
async fn cosmos_store(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, _| catalog::cosmos_store(tx, &input)).await
}
async fn universes_index(State(st): State<AppState>) -> Response {
    st.read(|c, _| catalog::universes_index(c)).await
}
async fn universes_store(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, _| catalog::universes_store(tx, &input)).await
}
async fn universes_update(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| catalog::universes_update(tx, id(&p, "universe")?, &input)).await
}
async fn characters_index(State(st): State<AppState>) -> Response {
    st.read(|c, env| catalog::characters_index(c, env)).await
}
async fn characters_pending(State(st): State<AppState>) -> Response {
    st.read(|c, env| catalog::characters_pending(c, env)).await
}
async fn characters_accept(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, _| catalog::accept(tx, &input)).await
}
async fn characters_reject(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, _| catalog::reject(tx, &input)).await
}
async fn characters_restore(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, _| catalog::restore(tx, &input)).await
}
async fn characters_words(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, _| catalog::forbidden_words(tx, id(&p, "character")?, &input)).await
}
async fn characters_destroy(State(st): State<AppState>, Path(p): Params) -> Response {
    st.write(move |tx, _| catalog::character_destroy(tx, id(&p, "character")?)).await
}

/// Entrées d'un formulaire : multipart (fichiers) ou JSON / urlencoded.
async fn form_input(req: Request) -> AppResult<(Input, Option<Vec<u8>>)> {
    let query = req.uri().query().map(str::to_string);
    let is_multipart = req.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|c| c.starts_with("multipart/form-data"));
    if is_multipart {
        let mp = <Multipart as axum::extract::FromRequest<()>>::from_request(req, &()).await.map_err(|_| AppError::unprocessable("Formulaire invalide."))?;
        Input::from_multipart(mp, query.as_deref()).await
    } else {
        Ok((<Input as axum::extract::FromRequest<()>>::from_request(req, &()).await?, None))
    }
}

async fn characters_store(State(st): State<AppState>, Path(p): Params, req: Request) -> Response {
    match form_input(req).await {
        Ok((input, file)) => st.write(move |tx, env| catalog::character_store(tx, env, id(&p, "universe")?, &input, file)).await,
        Err(e) => e.into_response(),
    }
}
async fn characters_put(State(st): State<AppState>, Path(p): Params, req: Request) -> Response {
    match form_input(req).await {
        Ok((input, file)) => st.write(move |tx, env| catalog::character_update(tx, env, id(&p, "character")?, &input, file)).await,
        Err(e) => e.into_response(),
    }
}
/// POST /characters/{id} + `_method=PUT` (method spoofing utilisé par le front).
async fn characters_post_spoofed(State(st): State<AppState>, Path(p): Params, req: Request) -> Response {
    match form_input(req).await {
        Ok((input, file)) if input.get("_method").and_then(|v| v.as_str()).is_some_and(|m| m.eq_ignore_ascii_case("PUT")) => {
            st.write(move |tx, env| catalog::character_update(tx, env, id(&p, "character")?, &input, file)).await
        }
        Ok(_) => StatusCode::METHOD_NOT_ALLOWED.into_response(),
        Err(e) => e.into_response(),
    }
}
async fn characters_image(State(st): State<AppState>, Path(p): Params, mp: Multipart) -> Response {
    match Input::from_multipart(mp, None).await {
        Ok((input, file)) => st.write(move |tx, env| catalog::upload_image(tx, env, id(&p, "character")?, &input, file)).await,
        Err(e) => e.into_response(),
    }
}

/// Suppression de parties : mot de passe dédié (en-tête `X-Delete-Password`),
/// en plus de l'accès admin. Vérification coûteuse (PBKDF2) hors des threads
/// réseau ; après 5 échecs en une minute, plus aucun essai n'est accepté (429)
/// jusqu'à la fin de la minute. Les suppressions réussies ne comptent pas.
async fn require_delete_password(st: &AppState, headers: &axum::http::HeaderMap) -> Result<(), Response> {
    let Some(stored) = st.cfg.delete_password_hash.clone() else {
        return Err(AppError::forbidden(
            "La suppression de parties est désactivée : aucun mot de passe de suppression n'est configuré sur le serveur.",
        )
        .into_response());
    };
    if st.limiter.is_limited("delete-password", "échecs", 5) {
        return Err(AppError::TooManyRequests.into_response());
    }
    let given = headers.get("x-delete-password").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
    let ok = tokio::task::spawn_blocking(move || !given.is_empty() && crate::support::password::verify(&given, &stored))
        .await
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        st.limiter.record("delete-password", "échecs");
        tracing::warn!("suppression refusée : mot de passe de suppression incorrect");
        Err(AppError::forbidden("Mot de passe de suppression incorrect.").into_response())
    }
}

async fn admin_games(State(st): State<AppState>) -> Response {
    st.read(|c, _| history::list_games(c, false)).await
}
async fn admin_game(State(st): State<AppState>, Path(p): Params) -> Response {
    st.read(move |c, env| history::detail(c, env, id(&p, "game")?, false)).await
}
async fn admin_game_destroy(State(st): State<AppState>, Path(p): Params, headers: axum::http::HeaderMap) -> Response {
    if let Err(r) = require_delete_password(&st, &headers).await {
        return r;
    }
    st.write(move |tx, _| admin::destroy_game(tx, id(&p, "game")?)).await
}
async fn admin_players(State(st): State<AppState>) -> Response {
    st.read(|c, env| admin::players_index(c, env)).await
}
async fn admin_player_room(State(st): State<AppState>, Path(p): Params) -> Response {
    st.write(move |tx, env| admin::remove_from_room(tx, env, id(&p, "player")?)).await
}
async fn admin_player_destroy(State(st): State<AppState>, Path(p): Params, headers: axum::http::HeaderMap, input: Input) -> Response {
    // Supprimer un joueur « avec ses parties » supprime des parties : même protection
    if admin::with_games(&input) {
        if let Err(r) = require_delete_password(&st, &headers).await {
            return r;
        }
    }
    st.write(move |tx, env| admin::destroy_player(tx, env, id(&p, "player")?, &input)).await
}
async fn admin_reactions(State(st): State<AppState>) -> Response {
    st.read(|c, env| social::admin_reactions_index(c, env)).await
}
async fn admin_reaction_store(State(st): State<AppState>, input: Input) -> Response {
    st.write(move |tx, env| social::admin_reaction_store(tx, env, &input)).await
}
async fn admin_reaction_update(State(st): State<AppState>, Path(p): Params, input: Input) -> Response {
    st.write(move |tx, env| social::admin_reaction_update(tx, env, id(&p, "reaction")?, &input)).await
}
async fn admin_reaction_destroy(State(st): State<AppState>, Path(p): Params) -> Response {
    st.write(move |tx, _| social::admin_reaction_destroy(tx, id(&p, "reaction")?)).await
}

/// Journalise les requêtes de plus d'une seconde (diagnostic de blocages) et pose
/// le cache des fichiers du front : assets à empreinte (`/assets/…-HASH.js`) en
/// cache d'un an, le reste (index.html) toujours revalidé.
async fn log_slow(req: Request, next: Next) -> Response {
    let (method, path) = (req.method().clone(), req.uri().path().to_string());
    let started = std::time::Instant::now();
    let mut res = next.run(req).await;
    if res.status().is_success() && !path.contains("/api/") && !res.headers().contains_key(header::CACHE_CONTROL) {
        let policy = if path.contains("/assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    }
    let ms = started.elapsed().as_millis();
    if ms >= 1000 {
        tracing::warn!("requête lente : {method} {path} → {} en {ms} ms", res.status().as_u16());
    }
    res
}

/// `lan-url.json` relu à chaque requête (le fichier est remplacé quand le Wi-Fi change).
async fn lan_url(State(st): State<AppState>) -> Response {
    let Some(path) = st.cfg.lan_url_file.clone() else { return AppError::NotFound.into_response() };
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mut res = Response::new(Body::from(bytes));
            res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            res
        }
        Err(_) => AppError::NotFound.into_response(),
    }
}

async fn not_found() -> Response {
    AppError::NotFound.into_response()
}

/// Toutes les routes, sous le préfixe public (`/binome`).
pub fn router(state: AppState) -> Router {
    let admin = Router::new()
        .route("/cosmos", post(cosmos_store))
        .route("/universes", get(universes_index).post(universes_store))
        .route("/universes/{universe}", patch(universes_update))
        .route("/universes/{universe}/characters", post(characters_store))
        .route("/characters", get(characters_index))
        .route("/characters/pending", get(characters_pending))
        .route("/characters/validation/accept", post(characters_accept))
        .route("/characters/validation/reject", post(characters_reject))
        .route("/characters/validation/restore", post(characters_restore))
        .route("/characters/{character}", post(characters_post_spoofed).put(characters_put).delete(characters_destroy))
        .route("/characters/{character}/forbidden-words", patch(characters_words))
        .route("/characters/{character}/image", post(characters_image))
        .route("/admin/games", get(admin_games))
        .route("/admin/games/{game}", get(admin_game).delete(admin_game_destroy))
        .route("/admin/players", get(admin_players))
        .route("/admin/players/{player}/room", axum::routing::delete(admin_player_room))
        .route("/admin/players/{player}", axum::routing::delete(admin_player_destroy))
        .route("/admin/reactions", get(admin_reactions).post(admin_reaction_store))
        .route("/admin/reactions/{reaction}", patch(admin_reaction_update).delete(admin_reaction_destroy))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    let api = Router::new()
        .route("/access/characters", post(access_login))
        .route("/cosmos", get(cosmos_index))
        .route("/reactions", get(reactions_index).post(reactions_store))
        .route("/reactions/{reaction}/image", get(reactions_image))
        .route("/history/games", get(history_games))
        .route("/history/games/{game}", get(history_game))
        .route("/history/leaderboard", get(history_leaderboard))
        .route("/history/players", get(history_player))
        .route("/chat/messages", get(chat_index).post(chat_store))
        .route("/avatars/{avatar}", get(avatar_show))
        .route("/character-images/{character}", get(character_image))
        .route("/players/{player}/avatar", post(avatar_update).delete(avatar_destroy))
        .route("/rooms", post(rooms_store))
        .route("/rooms/join", post(rooms_join))
        .route("/rooms/{room}", get(rooms_show))
        .route("/rooms/{room}/settings", patch(rooms_settings))
        .route("/rooms/{room}/ready", patch(rooms_ready))
        .route("/rooms/{room}/start", post(rooms_start))
        .route("/rooms/{room}/leave", axum::routing::delete(rooms_leave))
        .route("/games/{game}", get(games_show))
        .route("/games/{game}/me", get(games_me))
        .route("/games/{game}/recap", get(games_recap))
        .route("/games/{game}/reveal", get(games_reveal))
        .route("/games/{game}/question-votes", post(games_question_vote))
        .route("/games/{game}/character-vote", post(games_character_vote))
        .route("/games/{game}/reactions", post(games_react))
        .route("/games/{game}/skip-turn", post(games_skip))
        .route("/games/{game}/players/{player}/exclude", post(games_exclude))
        .route("/games/{game}/actions/{action}/correct-answer", post(games_correct))
        .route("/games/{game}/rounds/{round}/question", post(rounds_question))
        .route("/games/{game}/rounds/{round}/accusation", post(rounds_accusation))
        .route("/games/{game}/rounds/{round}/actions/{action}/answer", post(actions_answer))
        .route("/games/{game}/rounds/{round}/actions/{action}/confirm", post(actions_confirm))
        .route("/games/{game}/rounds/{round}/actions/{action}/draw", post(actions_draw))
        .route("/games/{game}/rounds/{round}/actions/{action}/draw/decision", post(actions_decide))
        .merge(admin)
        .fallback(not_found);

    let prefix = state.cfg.public_prefix.clone();
    let index = state.cfg.static_dir.as_ref().map(|d| d.join("index.html"));
    let mut app = Router::new()
        .nest("/api", api)
        .route("/ws", get(ws::upgrade))
        .route("/up", get(|| async { "ok" }))
        .route("/lan-url.json", get(lan_url));

    // Front compilé : fichiers statiques, repli SPA sur index.html
    if let Some(dir) = state.cfg.static_dir.clone() {
        // .gz produits au build (aucune compression à l'exécution)
        let spa = ServeDir::new(&dir)
            .append_index_html_on_directories(true)
            .precompressed_gzip()
            .fallback(ServeFile::new(dir.join("index.html")).precompressed_gzip());
        app = app.fallback_service(spa);
    }

    let app = app.layer(DefaultBodyLimit::max(6 * 1024 * 1024)).layer(middleware::from_fn(log_slow)).with_state(state);
    if prefix.is_empty() {
        app
    } else {
        let home = format!("{prefix}/");
        let mut outer = Router::new().route("/", get({
            let home = home.clone();
            move || async move { Redirect::temporary(&home) }
        }));
        // `/binome/` (lien partagé aux joueurs) : axum ne l'envoie pas au routeur
        // imbriqué sous `/binome`, on le déclare donc ici.
        outer = match index {
            Some(index) => outer.route_service(&home, ServeFile::new(index).precompressed_gzip()),
            None => outer,
        };
        outer.nest(&prefix, app)
    }
}
