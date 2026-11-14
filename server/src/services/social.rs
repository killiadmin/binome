//! Chat du hall, photos de profil, emojis (joueurs + admin) et réactions en partie.

use rusqlite::Transaction;
use serde_json::{json, Value};
use unicode_segmentation::UnicodeSegmentation;

use crate::clock::{self, now_db};
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Caller, Input, V};
use crate::realtime::hub::Event;
use crate::services::common::*;
use crate::services::controls;
use crate::services::game::game_channel;
use crate::services::Out;
use crate::support::image::{self, JPEG_PREFIX};

// ─── Chat du hall (canal public `lobby`) ────────────────────────────────────

const CHAT_HISTORY: i64 = 50;
const CHAT_RETENTION_MINUTES: i64 = 60;

fn chat_cutoff() -> String {
    clock::in_seconds_db(-CHAT_RETENTION_MINUTES * 60)
}

fn prune_chat(tx: &Transaction) -> AppResult<()> {
    tx.run("DELETE FROM chat_messages WHERE created_at < ?1", [chat_cutoff()])?;
    Ok(())
}

/// Codes de salons réellement existants cités dans le message (6 caractères A-Z0-9).
fn mentioned_codes(c: &impl Q, body: &str) -> AppResult<Vec<String>> {
    // Équivalent de /\b[A-Za-z0-9]{6}\b/ : mots ASCII (le `_` compte comme lettre)
    let mut candidates: Vec<String> = vec![];
    for word in body.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
        if word.len() == 6 && word.bytes().all(|b| b.is_ascii_alphanumeric()) {
            let up = word.to_ascii_uppercase();
            if !candidates.contains(&up) {
                candidates.push(up);
            }
        }
    }
    if candidates.is_empty() {
        return Ok(vec![]);
    }
    c.all(
        "SELECT code FROM rooms WHERE code IN (SELECT value FROM json_each(?1)) ORDER BY id",
        [serde_json::to_string(&candidates).expect("json")],
        |r| r.get(0),
    )
}

fn chat_json(c: &impl Q, id: i64) -> AppResult<Value> {
    let (name, key, body, created): (String, String, String, Option<String>) = c
        .opt("SELECT author_name, author_key, body, created_at FROM chat_messages WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .ok_or(AppError::NotFound)?;
    Ok(json!({
        "id": id,
        "author_name": name,
        // Dérivé de author_key (immuable) : le joueur peut avoir été supprimé depuis
        "is_anonymous": key.starts_with("a:"),
        "author_key": key,
        "codes": mentioned_codes(c, &body)?,
        "body": body,
        "created_at": clock::iso(created.as_deref()),
    }))
}

/// GET /chat/messages — 50 derniers de moins d'une heure (purge au passage).
pub fn chat_index(tx: &Transaction) -> AppResult<Out> {
    prune_chat(tx)?;
    let mut ids: Vec<i64> = tx.all(
        "SELECT id FROM chat_messages WHERE created_at >= ?1 ORDER BY id DESC LIMIT ?2",
        (chat_cutoff(), CHAT_HISTORY),
        |r| r.get(0),
    )?;
    ids.reverse();
    let messages = ids.into_iter().map(|id| chat_json(tx, id)).collect::<AppResult<Vec<_>>>()?;
    Ok(Out::ok(json!({ "messages": messages })))
}

/// Clé anti-spam du chat : l'auteur, pas l'IP.
pub fn chat_limit_key(input: &Input) -> String {
    match input.get("player_id") {
        Some(p) => format!("p:{}", p.as_i64().map_or_else(|| p.as_str().unwrap_or_default().to_string(), |n| n.to_string())),
        None => format!("a:{}", input.get("anon_id").and_then(Value::as_str).unwrap_or("ip")),
    }
}

/// POST /chat/messages — l'auteur est toujours dérivé côté serveur.
pub fn chat_store(tx: &Transaction, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input).messages(&[
        ("body.required", "Le message ne peut pas être vide."),
        ("body.max", "Le message ne peut pas dépasser 500 caractères."),
        ("anon_id.required_without", "Identifiant anonyme manquant."),
    ]);
    let body = v.req_str("body", None, Some(500));
    let pid = v.opt_int("player_id");
    let pid = v.exists(tx, "player_id", pid, "SELECT 1 FROM players WHERE id = ?1")?;
    let anon = if input.get("player_id").is_none() && input.get("anon_id").is_none() {
        v.fail("anon_id", "required_without", "The anon id field is required when player id is not present.".into());
        None
    } else {
        v.opt_str("anon_id", None, None)
    };
    if anon.as_ref().is_some_and(|a| a.is_empty() || a.len() > 12 || !a.bytes().all(|b| b.is_ascii_alphanumeric())) {
        v.fail("anon_id", "regex", "The anon id field format is invalid.".into());
    }
    v.finish()?;
    // Message signé d'un joueur : seulement le sien (sinon anonyme, sans player_id)
    if let Some(pid) = pid {
        input.1.require(pid)?;
    }

    let player = pid.map(|id| player(tx, id)).transpose()?.flatten();
    let anon = anon.unwrap_or_default();
    let (name, key) = match &player {
        Some(p) => (p.pseudo.clone(), format!("p:{}", p.id)),
        None => (format!("Anonyme#{anon}"), format!("a:{anon}")),
    };
    let now = now_db();
    let id = tx.insert(
        "INSERT INTO chat_messages (player_id, author_name, author_key, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        (player.as_ref().map(|p| p.id), &name, &key, body.expect("validé"), &now),
    )?;
    prune_chat(tx)?;
    let message = chat_json(tx, id)?;
    Ok(Out::created(json!({ "message": message })).event(Event::on("lobby".into(), "chat.message", json!({ "message": message }))))
}

// ─── Photos de profil (indexées par pseudo) ─────────────────────────────────

/// Événement « photo changée » : chaque salon du joueur + sa partie en cours.
fn avatar_event(tx: &Transaction, p: &Player, url: Option<String>) -> AppResult<Event> {
    let rooms = tx.all("SELECT room_id FROM room_player WHERE player_id = ?1 ORDER BY id", [p.id], |r| r.get::<_, i64>(0))?;
    let mut channels = vec![];
    for room_id in rooms {
        channels.push(format!("room.{room_id}"));
        if let Some(g) = current_game_id(tx, room_id)? {
            channels.push(game_channel(g));
        }
    }
    Ok(Event::new(channels, "player.avatar.updated", json!({ "player_id": p.id, "avatar_url": url })))
}

fn same_player(v: &mut V, p: &Player) -> AppResult<()> {
    let pid = v.req_int("player_id");
    if let Some(pid) = pid {
        if pid != p.id {
            return Err(AppError::forbidden("Tu ne peux modifier que ta propre photo."));
        }
    }
    Ok(())
}

/// POST /players/{player}/avatar — JPEG carré 64–512 px ≤ 200 ko, recadré par le navigateur.
/// Seul le joueur lui-même (jeton de profil) change sa photo.
pub fn avatar_update(tx: &Transaction, env: &Env, player_id: i64, input: &Input) -> AppResult<Out> {
    let p = player_or_404(tx, player_id)?;
    let mut v = V::new(input);
    let pid = v.req_int("player_id");
    let data = v.req_str("image", None, Some(300_000));
    if data.as_ref().is_some_and(|d| !d.starts_with(JPEG_PREFIX)) {
        v.fail("image", "starts_with", format!("The image field must start with one of the following: {JPEG_PREFIX}."));
    }
    v.finish()?;
    if pid != Some(p.id) {
        return Err(AppError::forbidden("Tu ne peux modifier que ta propre photo."));
    }
    input.1.require(p.id)?;
    let bytes = image::check_square_jpeg(
        &data.expect("validé"),
        200 * 1024,
        64,
        512,
        ("La photo doit être une image JPEG de moins de 200 ko.", "La photo doit être carrée, entre 64 et 512 pixels de côté."),
    )
    .map_err(|m| AppError::field("image", m))?;
    let uri = image::jpeg_data_uri(&bytes);
    let now = now_db();
    if tx.run("UPDATE player_avatars SET image = ?1, updated_at = ?2 WHERE pseudo = ?3", (&uri, &now, &p.pseudo))? == 0 {
        tx.run("INSERT INTO player_avatars (pseudo, image, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)", (&p.pseudo, &uri, &now))?;
    }
    let (id, updated): (i64, Option<String>) = tx.opt("SELECT id, updated_at FROM player_avatars WHERE pseudo = ?1", [&p.pseudo], |r| Ok((r.get(0)?, r.get(1)?)))?.expect("avatar");
    let url = avatar_url(env, id, updated.as_deref());
    let event = avatar_event(tx, &p, Some(url.clone()))?;
    Ok(Out::ok(json!({ "message": "Photo enregistrée.", "avatar_url": url })).event(event))
}

/// DELETE /players/{player}/avatar
pub fn avatar_destroy(tx: &Transaction, player_id: i64, input: &Input) -> AppResult<Out> {
    let p = player_or_404(tx, player_id)?;
    let mut v = V::new(input);
    same_player(&mut v, &p)?;
    v.finish()?;
    input.1.require(p.id)?;
    tx.run("DELETE FROM player_avatars WHERE pseudo = ?1", [&p.pseudo])?;
    let event = avatar_event(tx, &p, None)?;
    Ok(Out::ok(json!({ "message": "Photo supprimée.", "avatar_url": null })).event(event))
}

/// GET /avatars/{avatar} → octets JPEG
pub fn avatar_image(c: &impl Q, id: i64) -> AppResult<Vec<u8>> {
    let data: String = c.opt("SELECT image FROM player_avatars WHERE id = ?1", [id], |r| r.get(0))?.ok_or(AppError::NotFound)?;
    decode_after_comma(&data)
}

fn decode_after_comma(data: &str) -> AppResult<Vec<u8>> {
    use base64::Engine;
    let b64 = data.split_once(',').map_or(data, |(_, b)| b);
    base64::engine::general_purpose::STANDARD.decode(b64).map_err(|_| AppError::NotFound)
}

// ─── Emojis de réaction ─────────────────────────────────────────────────────

const REACTION_COLS: &str = "id, name, emoji, created_by_pseudo, is_enabled, created_at, updated_at";
/// Ordre de la barre : emojis Unicode d'abord, puis images, par ancienneté.
const BAR_ORDER: &str = "ORDER BY (emoji IS NULL), id";

fn reaction_public(env: &Env, r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let id: i64 = r.get(0)?;
    let emoji: Option<String> = r.get(2)?;
    let updated: Option<String> = r.get(6)?;
    Ok(json!({
        "id": id,
        "name": r.get::<_, String>(1)?,
        "url": reaction_url(env, id, emoji.is_some(), updated.as_deref()),
        "emoji": emoji,
        "created_by": r.get::<_, Option<String>>(3)?,
    }))
}

fn reaction_admin(env: &Env, r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let mut v = reaction_public(env, r)?;
    v["is_enabled"] = json!(r.get::<_, bool>(4)?);
    v["created_at"] = json!(clock::eloquent(r.get::<_, Option<String>>(5)?.as_deref()));
    Ok(v)
}

/// GET /reactions — les emojis personnalisés (images) seulement pour un profil validé.
pub fn reactions_index(c: &impl Q, env: &Env, caller: Caller) -> AppResult<Out> {
    let images = if controls::caller_validated(c, env, caller)? { "" } else { "AND emoji IS NOT NULL" };
    let list = c.all(&format!("SELECT {REACTION_COLS} FROM custom_reactions WHERE is_enabled = 1 {images} {BAR_ORDER}"), [], |r| reaction_public(env, r))?;
    Ok(Out::ok(json!({ "reactions": list })))
}

/// GET /reactions/{reaction}/image — servie même masquée (URL déjà affichée).
pub fn reaction_image(c: &impl Q, id: i64) -> AppResult<Vec<u8>> {
    let data: Option<String> = c.opt("SELECT image FROM custom_reactions WHERE id = ?1", [id], |r| r.get(0))?.ok_or(AppError::NotFound)?;
    decode_after_comma(&data.ok_or(AppError::NotFound)?)
}

fn reaction_image_error(data: &str) -> Option<String> {
    image::check_square_jpeg(
        data,
        60 * 1024,
        64,
        256,
        ("L'emoji doit être une image JPEG de moins de 60 ko.", "L'emoji doit être carré, entre 64 et 256 pixels de côté."),
    )
    .err()
}

fn image_rule(v: &mut V, field: &str, data: Option<&String>) {
    if data.is_some_and(|d| d.chars().count() > 100_000) {
        v.fail(field, "max", format!("The {field} field must not be greater than 100000 characters."));
    } else if data.is_some_and(|d| !d.starts_with(JPEG_PREFIX)) {
        v.fail(field, "starts_with", format!("The {field} field must start with one of the following: {JPEG_PREFIX}."));
    }
}

/// POST /reactions — un joueur crée un emoji image, aussitôt disponible pour tous.
pub fn reaction_store(tx: &Transaction, env: &Env, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let name = v.req_str("name", None, Some(20));
    let data = v.req_str("image", None, None);
    image_rule(&mut v, "image", data.as_ref());
    v.finish()?;
    let p = player_or_404(tx, pid.expect("validé"))?;
    controls::require_validated(tx, env, input.1, "La création d'emojis")?;
    let count: i64 = tx.one("SELECT COUNT(*) FROM custom_reactions WHERE is_enabled = 1 AND created_by_pseudo = ?1", [&p.pseudo])?;
    if count >= 10 {
        return Err(AppError::field("name", "Tu as déjà créé 10 emojis."));
    }
    let data = data.expect("validé");
    if let Some(e) = reaction_image_error(&data) {
        return Err(AppError::field("image", e));
    }
    let now = now_db();
    let id = tx.insert(
        "INSERT INTO custom_reactions (name, image, created_by_pseudo, is_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)",
        (name.expect("validé").trim(), &data, &p.pseudo, &now),
    )?;
    let reaction = tx.opt(&format!("SELECT {REACTION_COLS} FROM custom_reactions WHERE id = ?1"), [id], |r| reaction_public(env, r))?.expect("créé");
    Ok(Out::created(json!({ "message": "Emoji créé.", "reaction": reaction })))
}

/// POST /games/{game}/reactions — joueurs ET spectateurs ; rien n'est stocké.
pub fn react(tx: &Transaction, env: &Env, game_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(tx, game_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let rid = v.req_int("reaction_id");
    v.finish()?;
    let p = player_or_404(tx, pid.expect("validé"))?;
    if !can_watch(tx, &g, p.id)? {
        return Err(AppError::forbidden("Tu ne participes pas à cette partie."));
    }
    let reaction = tx
        .opt(&format!("SELECT {REACTION_COLS} FROM custom_reactions WHERE id = ?1 AND is_enabled = 1"), [rid.expect("validé")], |r| reaction_public(env, r))?
        .ok_or_else(|| AppError::Status(axum::http::StatusCode::NOT_FOUND, "Cet emoji n'est plus disponible.".into()))?;
    if reaction["emoji"].is_null() {
        controls::require_validated(tx, env, input.1, "L'envoi d'emojis personnalisés")?;
    }
    Ok(Out::message("Réaction envoyée.").event(Event::on(game_channel(g.id), "reaction.sent", json!({ "player": brief(&p), "reaction": reaction }))))
}

// ─── Emojis : administration ────────────────────────────────────────────────

pub fn admin_reactions_index(c: &impl Q, env: &Env) -> AppResult<Out> {
    let list = c.all(&format!("SELECT {REACTION_COLS} FROM custom_reactions {BAR_ORDER}"), [], |r| reaction_admin(env, r))?;
    Ok(Out::ok(json!({ "reactions": list })))
}

/// Un seul emoji : exactement un graphème, sans lettre ni chiffre.
/// (Correction validée : l'ancien contrôle laissait passer « 🦩🦩 ».)
pub fn is_single_emoji(s: &str) -> bool {
    s.graphemes(true).count() == 1 && !s.chars().any(char::is_alphanumeric)
}

pub fn admin_reaction_store(tx: &Transaction, env: &Env, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input);
    let name = v.req_str("name", None, Some(20));
    let emoji = v.opt_str("emoji", None, Some(32));
    let data = v.opt_str("image", None, None);
    if emoji.is_none() && data.is_none() {
        v.fail("emoji", "required_without", "The emoji field is required when image is not present.".into());
        v.fail("image", "required_without", "The image field is required when emoji is not present.".into());
    }
    image_rule(&mut v, "image", data.as_ref());
    v.finish()?;

    let now = now_db();
    let name = name.expect("validé");
    let id = if let Some(e) = emoji {
        let e = e.trim().to_string();
        if !is_single_emoji(&e) {
            return Err(AppError::field("emoji", "Saisis un seul emoji."));
        }
        if tx.exists("SELECT 1 FROM custom_reactions WHERE emoji = ?1", [&e])? {
            return Err(AppError::field("emoji", "Cet emoji existe déjà."));
        }
        tx.insert("INSERT INTO custom_reactions (name, emoji, is_enabled, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)", (name.trim(), &e, &now))?
    } else {
        let data = data.expect("validé");
        if let Some(err) = reaction_image_error(&data) {
            return Err(AppError::field("image", err));
        }
        tx.insert(
            "INSERT INTO custom_reactions (name, image, created_by_pseudo, is_enabled, created_at, updated_at) VALUES (?1, ?2, 'Admin', 1, ?3, ?3)",
            (name.trim(), &data, &now),
        )?
    };
    let reaction = tx.opt(&format!("SELECT {REACTION_COLS} FROM custom_reactions WHERE id = ?1"), [id], |r| reaction_admin(env, r))?.expect("créé");
    Ok(Out::created(json!({ "message": "Emoji ajouté.", "reaction": reaction })))
}

pub fn admin_reaction_update(tx: &Transaction, env: &Env, id: i64, input: &Input) -> AppResult<Out> {
    if !tx.exists("SELECT 1 FROM custom_reactions WHERE id = ?1", [id])? {
        return Err(AppError::NotFound);
    }
    let mut v = V::new(input);
    let name = v.opt_str("name", None, Some(20));
    let enabled = v.opt_bool("is_enabled");
    v.finish()?;
    let now = now_db();
    if let Some(n) = &name {
        tx.run("UPDATE custom_reactions SET name = ?1, updated_at = ?2 WHERE id = ?3", (n, &now, id))?;
    }
    if let Some(e) = enabled {
        tx.run("UPDATE custom_reactions SET is_enabled = ?1, updated_at = ?2 WHERE id = ?3", (e, &now, id))?;
    }
    let reaction = tx.opt(&format!("SELECT {REACTION_COLS} FROM custom_reactions WHERE id = ?1"), [id], |r| reaction_admin(env, r))?.expect("existe");
    Ok(Out::ok(json!({ "message": "Emoji mis à jour.", "reaction": reaction })))
}

pub fn admin_reaction_destroy(tx: &Transaction, id: i64) -> AppResult<Out> {
    if tx.run("DELETE FROM custom_reactions WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Out::message("Emoji supprimé."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_emoji_rule() {
        assert!(is_single_emoji("🦩"));
        assert!(is_single_emoji("👨‍👩‍👧")); // séquence ZWJ = un seul graphème
        assert!(is_single_emoji("🇫🇷"));
        assert!(!is_single_emoji("🦩🦩"));
        assert!(!is_single_emoji("A"));
        assert!(!is_single_emoji("1"));
        assert!(!is_single_emoji(""));
    }
}
