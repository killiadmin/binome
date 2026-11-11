//! Administration des joueurs et des parties.

use std::collections::{HashMap, HashSet};

use rusqlite::Transaction;
use serde_json::json;

use crate::clock;
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::Input;
use crate::services::common::*;
use crate::services::rooms::remove_player;
use crate::services::Out;
use crate::support::text::ci_key;

fn players_in_progress_games(c: &impl Q) -> AppResult<HashSet<i64>> {
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
    let players = c.all("SELECT id, pseudo, created_at, updated_at FROM players ORDER BY pseudo, id", [], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?))
    })?;
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
        .map(|(id, pseudo, created, updated)| {
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

fn rooms_of(c: &impl Q, player_id: i64) -> AppResult<Vec<Room>> {
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

/// DELETE /admin/players/{player}[?with_games=1]
pub fn destroy_player(tx: &Transaction, env: &Env, player_id: i64, input: &Input) -> AppResult<Out> {
    let p = player_or_404(tx, player_id)?;
    blocked_by_game(tx, &p)?;
    let with_games = input
        .get("with_games")
        .is_some_and(|v| matches!(v.as_str().unwrap_or(&v.to_string()), "1" | "true" | "on" | "yes"));
    let game_ids: Vec<i64> = tx.all(
        "SELECT b.game_id FROM binome_player bp JOIN binomes b ON b.id = bp.binome_id WHERE bp.player_id = ?1
         UNION SELECT r.game_id FROM actions a JOIN rounds r ON r.id = a.round_id WHERE a.player_id = ?1 OR a.target_player_id = ?1
         UNION SELECT game_id FROM rounds WHERE current_player_id = ?1",
        [p.id],
        |r| r.get(0),
    )?;
    if !game_ids.is_empty() && !with_games {
        let n = game_ids.len();
        return Ok(Out::ok(json!({
            "message": format!("{} a joué {n} partie(s) : supprime-les aussi pour pouvoir supprimer le joueur.", p.pseudo),
            "games_count": n,
        }))
        .status(axum::http::StatusCode::CONFLICT));
    }
    let mut events = vec![];
    for r in rooms_of(tx, p.id)? {
        remove_player(tx, env, &r, p.id, &mut events)?;
    }
    for g in &game_ids {
        tx.run("DELETE FROM games WHERE id = ?1", [g])?;
    }
    tx.run("DELETE FROM player_avatars WHERE pseudo = ?1", [&p.pseudo])?;
    tx.run("DELETE FROM players WHERE id = ?1", [p.id])?;
    let msg = if game_ids.is_empty() {
        format!("{} a été supprimé.", p.pseudo)
    } else {
        format!("{} et ses {} partie(s) ont été supprimés.", p.pseudo, game_ids.len())
    };
    Ok(Out::message(&msg).with(events))
}

/// DELETE /admin/games/{game} — cascade : binômes, rounds, actions, stats.
pub fn destroy_game(tx: &Transaction, game_id: i64) -> AppResult<Out> {
    game_or_404(tx, game_id)?;
    match tx.run("DELETE FROM games WHERE id = ?1", [game_id]) {
        Ok(_) => Ok(Out::message("Partie supprimée.")),
        Err(_) => Err(AppError::rule("Impossible de supprimer cette partie.")),
    }
}
