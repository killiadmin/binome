//! Boîte à idées (migration 8, onglet Idées) : tout joueur connecté (jeton de
//! profil) poste librement, 5 idées par 24 h, filtrées des insultes ; un +1 par
//! joueur et par idée, retirable, jamais sur la sienne ; pas de commentaires.
//! L'auteur supprime son idée ; un administrateur la met en favori (en tête de
//! liste) ou la supprime.

use rusqlite::Transaction;
use serde_json::{json, Value};

use crate::clock::{self, now_db};
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Caller, Input, V};
use crate::services::common::*;
use crate::services::Out;
use crate::support::moderation::contains_insult;
use crate::support::text::ci_key;

pub const DAILY_LIMIT: i64 = 5;
const BODY_MIN: usize = 10;
const BODY_MAX: usize = 500;

/// Colonnes d'une idée ; `?1` = joueur qui regarde (0 = personne).
const SELECT: &str = "SELECT i.id, i.body, COALESCE(p.pseudo, i.author_name), i.player_id, i.pinned_at, i.created_at,
        (SELECT COUNT(*) FROM idea_votes v WHERE v.idea_id = i.id) AS votes,
        EXISTS (SELECT 1 FROM idea_votes v WHERE v.idea_id = i.id AND v.player_id = ?1)
    FROM ideas i LEFT JOIN players p ON p.id = i.player_id";
/// Favoris d'abord (le plus récemment épinglé en tête), puis les plus votées.
const ORDER: &str = "ORDER BY i.pinned_at IS NULL, i.pinned_at DESC, votes DESC, i.id DESC";

struct Row {
    id: i64,
    body: String,
    author: String,
    author_id: Option<i64>,
    pinned_at: Option<String>,
    created_at: Option<String>,
    votes: i64,
    has_voted: bool,
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        body: r.get(1)?,
        author: r.get(2)?,
        author_id: r.get(3)?,
        pinned_at: r.get(4)?,
        created_at: r.get(5)?,
        votes: r.get(6)?,
        has_voted: r.get(7)?,
    })
}

fn viewer(caller: Caller) -> i64 {
    match caller {
        Caller::Player(id) => id,
        _ => 0,
    }
}

fn to_json(c: &impl Q, env: &Env, rows: Vec<Row>, viewer: i64) -> AppResult<Vec<Value>> {
    let avatars = avatar_urls(c, env, rows.iter().map(|r| r.author.as_str()))?;
    Ok(rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "body": r.body,
                "author_name": r.author,
                "author_avatar_url": avatars.get(&ci_key(&r.author)),
                "votes": r.votes,
                "has_voted": r.has_voted,
                "is_mine": viewer != 0 && r.author_id == Some(viewer),
                "is_pinned": r.pinned_at.is_some(),
                "pinned_at": clock::iso(r.pinned_at.as_deref()),
                "created_at": clock::iso(r.created_at.as_deref()),
            })
        })
        .collect())
}

fn one(c: &impl Q, env: &Env, id: i64, viewer: i64) -> AppResult<Value> {
    let r = c.opt(&format!("{SELECT} WHERE i.id = ?2"), (viewer, id), row)?.ok_or(AppError::NotFound)?;
    Ok(to_json(c, env, vec![r], viewer)?.remove(0))
}

fn posted_today(c: &impl Q, player_id: i64) -> AppResult<i64> {
    c.one("SELECT COUNT(*) FROM ideas WHERE player_id = ?1 AND created_at >= ?2", (player_id, clock::in_seconds_db(-24 * 3600)))
}

/// GET /ideas — lecture libre ; `has_voted` / `is_mine` / `remaining_today` pour un joueur connecté.
pub fn index(c: &impl Q, env: &Env, caller: Caller) -> AppResult<Out> {
    let viewer = viewer(caller);
    let rows = c.all(&format!("{SELECT} {ORDER}"), [viewer], row)?;
    let ideas = to_json(c, env, rows, viewer)?;
    let remaining = if viewer != 0 { Some((DAILY_LIMIT - posted_today(c, viewer)?).max(0)) } else { None };
    Ok(Out::ok(json!({ "ideas": ideas, "daily_limit": DAILY_LIMIT, "remaining_today": remaining })))
}

/// POST /ideas — `{player_id, body}`.
pub fn store(tx: &Transaction, env: &Env, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input).messages(&[
        ("body.required", "Écris ton idée avant de l'envoyer."),
        ("body.min", "Ton idée doit faire au moins 10 caractères."),
        ("body.max", "Ton idée ne peut pas dépasser 500 caractères."),
    ]);
    let pid = v.player(tx, "player_id")?;
    let body = v.req_str("body", Some(BODY_MIN), Some(BODY_MAX));
    v.finish()?;
    let p = player_or_404(tx, pid.expect("validé"))?;
    let body = body.expect("validé");
    if contains_insult(&body) {
        tracing::info!("idée refusée (terme injurieux) pour le joueur {}", p.id);
        return Err(AppError::field("body", "Ton idée contient un terme injurieux : reformule-la poliment."));
    }
    if posted_today(tx, p.id)? >= DAILY_LIMIT {
        return Err(AppError::field("body", format!("Tu as déjà posté {DAILY_LIMIT} idées ces dernières 24 heures : reviens plus tard.")));
    }
    let now = now_db();
    let id = tx.insert(
        "INSERT INTO ideas (player_id, author_name, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        (p.id, &p.pseudo, &body, &now),
    )?;
    Ok(Out::created(json!({ "message": "Idée publiée, merci !", "idea": one(tx, env, id, p.id)? })))
}

fn author_of(c: &impl Q, id: i64) -> AppResult<Option<i64>> {
    c.opt("SELECT player_id FROM ideas WHERE id = ?1", [id], |r| r.get::<_, Option<i64>>(0))?.ok_or(AppError::NotFound)
}

/// PUT /ideas/{idea}/vote (`on`) et DELETE /ideas/{idea}/vote — sans effet si déjà dans cet état.
pub fn vote(tx: &Transaction, env: &Env, id: i64, input: &Input, on: bool) -> AppResult<Out> {
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    v.finish()?;
    let pid = pid.expect("validé");
    let author = author_of(tx, id)?;
    if on {
        if author == Some(pid) {
            return Err(AppError::rule("Tu ne peux pas voter pour ta propre idée."));
        }
        tx.run("INSERT OR IGNORE INTO idea_votes (idea_id, player_id, created_at) VALUES (?1, ?2, ?3)", (id, pid, now_db()))?;
    } else {
        tx.run("DELETE FROM idea_votes WHERE idea_id = ?1 AND player_id = ?2", (id, pid))?;
    }
    let msg = if on { "Vote enregistré." } else { "Vote retiré." };
    Ok(Out::ok(json!({ "message": msg, "idea": one(tx, env, id, pid)? })))
}

/// DELETE /ideas/{idea} — par son auteur seulement.
pub fn destroy_own(tx: &Transaction, id: i64, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    v.finish()?;
    if author_of(tx, id)? != pid {
        return Err(AppError::forbidden("Tu ne peux supprimer que tes propres idées."));
    }
    tx.run("DELETE FROM ideas WHERE id = ?1", [id])?;
    Ok(Out::message("Idée supprimée."))
}

// ─── Administration ─────────────────────────────────────────────────────────

/// PATCH /admin/ideas/{idea} — `{is_pinned}` : favori en tête de liste.
pub fn admin_update(tx: &Transaction, env: &Env, id: i64, input: &Input) -> AppResult<Out> {
    author_of(tx, id)?;
    let mut v = V::new(input);
    let pinned = v.req_bool("is_pinned");
    v.finish()?;
    let now = now_db();
    // Déjà en favori : on garde sa date (sa place parmi les favoris)
    let pinned_at = if pinned.expect("validé") { "COALESCE(pinned_at, ?1)" } else { "NULL" };
    tx.run(&format!("UPDATE ideas SET pinned_at = {pinned_at}, updated_at = ?1 WHERE id = ?2"), (&now, id))?;
    let msg = if pinned == Some(true) { "Idée mise en favori." } else { "Idée retirée des favoris." };
    Ok(Out::ok(json!({ "message": msg, "idea": one(tx, env, id, viewer(input.1))? })))
}

/// DELETE /admin/ideas/{idea}
pub fn admin_destroy(tx: &Transaction, id: i64) -> AppResult<Out> {
    if tx.run("DELETE FROM ideas WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Out::message("Idée supprimée."))
}
