//! Administration des joueurs et des parties.
//!
//! Deux sortes d'administrateurs (migration 7) : le propriétaire (jeton
//! `X-Characters-Access`, obtenu avec le mot de passe d'administration) et les
//! administrateurs qu'il désigne (jeton de profil). Seul le propriétaire désigne
//! ou retire un administrateur ; un désigné ne peut pas supprimer un administrateur.

use std::collections::{HashMap, HashSet};

use rusqlite::Transaction;
use serde_json::json;

use crate::clock;
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::Input;
use crate::http::input::V;
use crate::services::common::*;
use crate::services::profile;
use crate::services::rooms::remove_player;
use crate::services::Out;
use crate::support::text::ci_key;

/// Qui accède à l'administration (posé par le middleware `require_admin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminRole {
    /// Propriétaire : mot de passe d'administration.
    Owner,
    /// Administrateur désigné par le propriétaire (identifiant du joueur).
    Delegate(i64),
}

pub(crate) fn players_in_progress_games(c: &impl Q) -> AppResult<HashSet<i64>> {
    Ok(c
        .all(
            "SELECT bp.player_id FROM binome_player bp JOIN binomes b ON b.id = bp.binome_id JOIN games g ON g.id = b.game_id
             WHERE g.status = 'in_progress'",
            [],
            |r| r.get(0),
        )?
        .into_iter()
        .collect())
}

/// GET /admin/players
pub fn players_index(c: &impl Q, env: &Env) -> AppResult<Out> {
    let players = c.all(
        "SELECT id, pseudo, created_at, updated_at, password_hash IS NOT NULL, validated_at, admin_granted_at IS NOT NULL FROM players
         WHERE deleted_at IS NULL ORDER BY pseudo, id",
        [],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, bool>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, bool>(6)?,
            ))
        },
    )?;
    let games: HashMap<i64, i64> = c.all("SELECT player_id, COUNT(*) FROM binome_player GROUP BY player_id", [], |r| Ok((r.get(0)?, r.get(1)?)))?.into_iter().collect();
    let rooms: HashMap<i64, String> = c
        .all("SELECT rp.player_id, r.code FROM room_player rp JOIN rooms r ON r.id = rp.room_id ORDER BY rp.id", [], |r| Ok((r.get(0)?, r.get(1)?)))?
        .into_iter()
        .collect();
    let mut stats: HashMap<String, (i64, i64)> = HashMap::new();
    for (pseudo, wins, score) in c.all("SELECT player_pseudo, is_winner, score FROM game_stats", [], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
    })? {
        let e = stats.entry(ci_key(&pseudo)).or_default();
        e.0 += wins;
        e.1 += score;
    }
    let in_game = players_in_progress_games(c)?;
    let avatars = avatar_urls(c, env, players.iter().map(|p| p.1.as_str()))?;
    let list: Vec<_> = players
        .iter()
        .map(|(id, pseudo, created, updated, has_password, validated_at, granted)| {
            let is_admin = profile::is_admin_pseudo(env, pseudo);
            let role = if is_admin {
                Some("owner")
            } else if *granted && *has_password {
                Some("admin")
            } else {
                None
            };
            let (wins, score) = stats.get(&ci_key(pseudo)).copied().unwrap_or_default();
            json!({
                "id": id,
                "pseudo": pseudo,
                "avatar_url": avatars.get(&ci_key(pseudo)),
                "games_count": games.get(id).copied().unwrap_or(0),
                "wins": wins,
                "score": score,
                "room_code": rooms.get(id),
                "in_game": in_game.contains(id),
                "has_password": *has_password || is_admin,
                "is_admin": is_admin,
                "is_validated": validated_at.is_some() || role.is_some(),
                "admin_role": role,
                "validated_at": clock::iso(validated_at.as_deref()),
                "created_at": clock::iso(created.as_deref()),
                "updated_at": clock::iso(updated.as_deref()),
            })
        })
        .collect();
    Ok(Out::ok(json!({ "players": list })))
}

fn blocked_by_game(c: &impl Q, p: &Player) -> AppResult<()> {
    if players_in_progress_games(c)?.contains(&p.id) {
        return Err(AppError::rule(format!("{} est dans une partie en cours : supprime ou termine d'abord la partie.", p.pseudo)));
    }
    Ok(())
}

pub(crate) fn rooms_of(c: &impl Q, player_id: i64) -> AppResult<Vec<Room>> {
    let ids: Vec<i64> = c.all("SELECT room_id FROM room_player WHERE player_id = ?1 ORDER BY id", [player_id], |r| r.get(0))?;
    ids.into_iter().filter_map(|id| room(c, id).transpose()).collect()
}

/// DELETE /admin/players/{player}/room
pub fn remove_from_room(tx: &Transaction, env: &Env, player_id: i64) -> AppResult<Out> {
    let p = player_or_404(tx, player_id)?;
    blocked_by_game(tx, &p)?;
    let rooms = rooms_of(tx, p.id)?;
    if rooms.is_empty() {
        return Err(AppError::rule(format!("{} n'est dans aucun salon.", p.pseudo)));
    }
    let mut events = vec![];
    for r in rooms {
        remove_player(tx, env, &r, p.id, &mut events)?;
    }
    Ok(Out::message(&format!("{} a été retiré du salon.", p.pseudo)).with(events))
}

/// `?with_games=1` (ou true / on / yes) : supprimer aussi les parties du joueur.
pub fn with_games(input: &Input) -> bool {
    input.get("with_games").is_some_and(|v| matches!(v.as_str().unwrap_or(&v.to_string()), "1" | "true" | "on" | "yes"))
}

/// DELETE /admin/players/{player}[?with_games=1]
///
/// Par défaut, le joueur est effacé comme s'il le demandait lui-même
/// (`profile::anonymize`) : ses parties restent dans l'historique des autres sous
/// « Joueur supprimé ». Avec `?with_games=1` (et le mot de passe de suppression),
/// ses parties sont supprimées pour tous leurs joueurs, puis le joueur.
pub fn destroy_player(tx: &Transaction, env: &Env, role: AdminRole, player_id: i64, input: &Input) -> AppResult<Out> {
    let p = player_or_404(tx, player_id)?;
    let target = profile::by_id(tx, p.id)?.ok_or(AppError::NotFound)?;
    if target.is_admin(env) {
        return Err(AppError::forbidden("Le profil propriétaire ne peut pas être supprimé."));
    }
    if matches!(role, AdminRole::Delegate(_)) && target.admin_role(env).is_some() {
        return Err(AppError::forbidden("Seul le propriétaire peut supprimer un administrateur."));
    }
    blocked_by_game(tx, &p)?;
    let mut events = vec![];
    if !with_games(input) {
        if target.deleted {
            return Err(AppError::rule(format!("{} est déjà anonymisé.", p.pseudo)));
        }
        let msg = if profile::anonymize(tx, env, &target, &mut events)? {
            format!("{} a été supprimé.", p.pseudo)
        } else {
            format!("{} a été anonymisé : ses parties restent dans l'historique sous « Joueur supprimé ».", p.pseudo)
        };
        return Ok(Out::message(&msg).with(events));
    }
    let game_ids: Vec<i64> = tx.all(
        "SELECT b.game_id FROM binome_player bp JOIN binomes b ON b.id = bp.binome_id WHERE bp.player_id = ?1
         UNION SELECT r.game_id FROM actions a JOIN rounds r ON r.id = a.round_id WHERE a.player_id = ?1 OR a.target_player_id = ?1
         UNION SELECT game_id FROM rounds WHERE current_player_id = ?1",
        [p.id],
        |r| r.get(0),
    )?;
    for g in &game_ids {
        tx.run("DELETE FROM games WHERE id = ?1", [g])?;
    }
    // Ses parties parties, plus rien ne retient la ligne : anonymize la supprime
    // (salons, photo, emojis, messages, votes, statistiques restantes)
    profile::anonymize(tx, env, &target, &mut events)?;
    let msg = if game_ids.is_empty() {
        format!("{} a été supprimé.", p.pseudo)
    } else {
        format!("{} et ses {} partie(s) ont été supprimés.", p.pseudo, game_ids.len())
    };
    Ok(Out::message(&msg).with(events))
}

/// PATCH /admin/players/{player}/admin — désigne ou retire un administrateur
/// (propriétaire seulement). Le profil désigné doit avoir un mot de passe.
pub fn set_admin(tx: &Transaction, env: &Env, role: AdminRole, player_id: i64, input: &Input) -> AppResult<Out> {
    if role != AdminRole::Owner {
        return Err(AppError::forbidden("Seul le propriétaire peut désigner ou retirer un administrateur."));
    }
    let p = profile::by_id(tx, player_id)?.ok_or(AppError::NotFound)?;
    let mut v = V::new(input);
    let admin = v.req_bool("admin");
    v.finish()?;
    let admin = admin.expect("validé");
    if p.is_admin(env) {
        return Err(AppError::rule("Le profil propriétaire est toujours administrateur."));
    }
    if admin && p.password_hash.is_none() {
        return Err(AppError::rule(format!(
            "Le profil de {} est public : il doit d'abord poser un mot de passe (page Profil) pour devenir administrateur.",
            p.pseudo
        )));
    }
    let stamp = admin.then(clock::now_db);
    tx.run("UPDATE players SET admin_granted_at = ?1 WHERE id = ?2", (&stamp, p.id))?;
    let p = profile::by_id(tx, p.id)?.expect("profil");
    let msg = format!("{} {}.", p.pseudo, if admin { "est administrateur" } else { "n'est plus administrateur" });
    Ok(Out::ok(json!({ "message": msg, "player": { "id": p.id, "admin_role": p.admin_role(env), "is_validated": p.is_validated(env) } })))
}

/// DELETE /admin/games/{game} — cascade : binômes, rounds, actions, stats.
pub fn destroy_game(tx: &Transaction, game_id: i64) -> AppResult<Out> {
    game_or_404(tx, game_id)?;
    match tx.run("DELETE FROM games WHERE id = ?1", [game_id]) {
        Ok(_) => Ok(Out::message("Partie supprimée.")),
        Err(_) => Err(AppError::rule("Impossible de supprimer cette partie.")),
    }
}
