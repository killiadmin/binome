//! Historique des parties et classement cumulé (GameHistoryService).
//!
//! Le même formatage sert à l'admin (toutes les parties) et au public (parties
//! terminées seulement) : c'est l'appelant qui décide, pas ce module.
//! Invariant public : rien d'une partie en cours ne sort par ces routes.

use serde_json::{json, Value};

use crate::clock;
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Input, V};
use crate::services::common::*;
use crate::services::score::{by_score_desc, game_stats};
use crate::services::Out;
use crate::support::text::ci_key;

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Liste des parties, la plus récente d'abord.
pub fn list_games(c: &impl Q, finished_only: bool) -> AppResult<Out> {
    let filter = if finished_only { "WHERE g.status = 'finished'" } else { "" };
    let games = c.all(
        &format!(
            "SELECT g.id, g.room_id, g.room_code, g.status, g.created_at, g.updated_at,
                    (SELECT COUNT(*) FROM binomes b WHERE b.game_id = g.id),
                    (SELECT COUNT(*) FROM rounds rd WHERE rd.game_id = g.id),
                    (SELECT COUNT(*) FROM binome_player bp JOIN binomes b ON b.id = bp.binome_id WHERE b.game_id = g.id)
             FROM games g {filter}
             ORDER BY g.created_at DESC, g.id DESC"
        ),
        [],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                // Salon supprimé depuis : la partie reste, sans salon
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
            ))
        },
    )?;
    let mut out = vec![];
    for (id, room_id, code, status, created, updated, binomes, rounds, players) in games {
        let winners: Vec<String> = c.all("SELECT player_pseudo FROM game_stats WHERE game_id = ?1 AND is_winner = 1 ORDER BY id", [id], |r| r.get(0))?;
        out.push(json!({
            "id": id,
            "room_id": room_id,
            "room_code": code,
            "status": status,
            "players_count": players,
            "binomes_count": binomes,
            // La découverte de binôme n'existe plus : toujours 0 (conservé pour le contrat)
            "binomes_discovered_count": 0,
            "rounds_count": rounds,
            "winners": winners,
            "created_at": clock::iso(created.as_deref()),
            "updated_at": clock::iso(updated.as_deref()),
        }));
    }
    Ok(Out::ok(json!({ "games": out })))
}

fn history_action(a: &Action, names: &std::collections::HashMap<i64, String>, avatars: &std::collections::HashMap<String, String>) -> Value {
    let who = |id: i64| {
        let pseudo = names.get(&id).cloned().unwrap_or_default();
        json!({ "id": id, "avatar_url": avatars.get(&ci_key(&pseudo)), "pseudo": pseudo })
    };
    let mut v = json!({
        "id": a.id,
        "round_id": a.round_id,
        "type": a.kind,
        "player": who(a.player_id),
        "target_player": a.target_player_id.map(who),
        "is_valid": a.is_valid,
        "answer": a.answer,
        "answer_corrected_from": a.answer_corrected_from,
        "created_at": clock::iso(a.created_at.as_deref()),
        "content": a.content,
    });
    let o = v.as_object_mut().expect("objet");
    if a.is_question() {
        o.insert("refused_reason".into(), if a.is_valid { Value::Null } else { json!("Mot interdit détecté.") });
    } else {
        o.insert("character_name".into(), json!(a.character_name.as_ref().or(a.content.as_ref())));
        o.insert("accusation_correct".into(), json!(a.accusation_correct));
        o.insert("accusation_confirmed".into(), json!(a.accusation_confirmed));
    }
    v
}

/// Détail complet d'une partie, sans aucun masquage (l'appelant filtre).
pub fn detail(c: &impl Q, env: &Env, game_id: i64, finished_only: bool) -> AppResult<Out> {
    let g = game_or_404(c, game_id)?;
    if finished_only && !g.finished() {
        return Err(AppError::Status(axum::http::StatusCode::NOT_FOUND, "Cette partie n'est pas terminée.".into()));
    }
    let code: Option<String> = c.one("SELECT room_code FROM games WHERE id = ?1", [g.id])?;
    let seated = seats(c, g.id)?;
    let stats = game_stats(c, g.id)?;
    let actions = game_actions(c, g.id)?;
    let names = pseudos(c, actions.iter().flat_map(|a| [Some(a.player_id), a.target_player_id]).flatten())?;
    let avatars = avatar_urls(
        c,
        env,
        seated.iter().map(|s| s.pseudo.as_str()).chain(stats.iter().map(|s| s.player_pseudo.as_str())).chain(names.values().map(String::as_str)),
    )?;

    let mut binomes: Vec<(i64, Vec<&Seat>)> = vec![];
    for s in &seated {
        match binomes.iter_mut().find(|(id, _)| *id == s.binome_id) {
            Some((_, l)) => l.push(s),
            None => binomes.push((s.binome_id, vec![s])),
        }
    }
    let mut binomes_json = vec![];
    for (id, members) in &binomes {
        let (universe, cosmos) = crate::services::game::universe_names(c, members[0].universe_id)?;
        let mut players = vec![];
        for s in members {
            let ch = character(c, s.character_id)?;
            let ch_json = match ch {
                Some(ch) => json!({
                    "id": ch.id,
                    "name": ch.name,
                    "universe": crate::services::game::universe_names(c, ch.universe_id)?.0,
                    "forbidden_words": ch.forbidden_words,
                }),
                None => Value::Null,
            };
            players.push(json!({
                "id": s.player_id,
                "pseudo": s.pseudo,
                "avatar_url": avatars.get(&ci_key(&s.pseudo)),
                "is_eliminated": s.is_eliminated,
                "score": s.score,
                "character": ch_json,
            }));
        }
        binomes_json.push(json!({
            "id": id,
            "universe": universe,
            "cosmos": cosmos,
            "is_discovered": false,
            "is_orphan": members[0].is_orphan,
            "discovered_by_player_id": null,
            "players": players,
        }));
    }

    let rounds_json: Vec<Value> = rounds(c, g.id)?
        .iter()
        .map(|r| {
            let mut list: Vec<&Action> = actions.iter().filter(|a| a.round_id == r.id).collect();
            list.sort_by(|a, b| (a.created_at.as_deref(), a.id).cmp(&(b.created_at.as_deref(), b.id)));
            json!({
                "id": r.id,
                "number": r.number,
                "is_finished": r.is_finished,
                "current_player_id": r.current_player_id,
                "actions": list.iter().map(|a| history_action(a, &names, &avatars)).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(Out::ok(json!({
        "id": g.id,
        "room_id": g.room_id,
        "room_code": code,
        "status": g.status,
        "created_at": clock::iso(g.created_at.as_deref()),
        "updated_at": clock::iso(g.updated_at.as_deref()),
        "binomes": binomes_json,
        "rounds": rounds_json,
        "game_stats": by_score_desc(stats).iter().map(|s| json!({
            "player_pseudo": s.player_pseudo,
            "avatar_url": avatars.get(&ci_key(&s.player_pseudo)),
            "character_name": s.character_name,
            "eliminations": s.eliminations,
            "best_questions": s.best_questions,
            "rounds_survived": s.rounds_survived,
            "survived_full_game": s.survived_full_game,
            "score": s.score,
            "is_winner": s.is_winner,
            "is_eliminated": s.is_eliminated,
        })).collect::<Vec<_>>(),
    })))
}

/// Classement cumulé par pseudo (insensible casse / accents), rangs ex æquo.
pub fn leaderboard(c: &impl Q, env: &Env) -> AppResult<Out> {
    let rows = c.all(
        "SELECT MAX(s.player_pseudo), COUNT(*), COALESCE(SUM(s.score), 0), COALESCE(SUM(s.is_winner), 0),
                COALESCE(SUM(s.eliminations), 0), COALESCE(SUM(s.rounds_survived), 0)
         FROM game_stats s JOIN games g ON g.id = s.game_id
         WHERE g.status = 'finished'
         GROUP BY s.player_pseudo
         ORDER BY 3 DESC, 4 DESC, 1 ASC",
        [],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?)),
    )?;
    let avatars = avatar_urls(c, env, rows.iter().map(|r| r.0.as_str()))?;
    let (mut rank, mut previous) = (0, None);
    let board: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(i, (pseudo, played, total, wins, elims, rounds))| {
            if previous != Some(*total) {
                rank = i + 1;
                previous = Some(*total);
            }
            json!({
                "pseudo": pseudo,
                "avatar_url": avatars.get(&ci_key(pseudo)),
                "games_played": played,
                "total_score": total,
                "wins": wins,
                "eliminations": elims,
                "rounds_survived": rounds,
                "average_score": if *played > 0 { round1(*total as f64 / *played as f64) } else { 0.0 },
                "rank": rank,
            })
        })
        .collect();
    Ok(Out::ok(json!({ "leaderboard": board })))
}

/// GET /history/players?pseudo= — totaux + une ligne par partie terminée.
pub fn player_breakdown(c: &impl Q, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input);
    let pseudo = v.req_str("pseudo", None, Some(20));
    v.finish()?;
    let rows = c.all(
        "SELECT s.game_id, g.room_code, g.created_at, s.character_name, s.score, s.eliminations, s.rounds_survived, s.is_winner,
                s.is_eliminated, s.player_pseudo
         FROM game_stats s JOIN games g ON g.id = s.game_id
         WHERE s.player_pseudo = ?1 AND g.status = 'finished'
         ORDER BY g.created_at DESC, s.id DESC",
        [pseudo.expect("validé")],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, bool>(7)?,
                r.get::<_, bool>(8)?,
                r.get::<_, String>(9)?,
            ))
        },
    )?;
    if rows.is_empty() {
        return Err(AppError::Status(axum::http::StatusCode::NOT_FOUND, "Aucune partie pour ce joueur.".into()));
    }
    let total: i64 = rows.iter().map(|r| r.4).sum();
    let n = rows.len() as i64;
    Ok(Out::ok(json!({
        "pseudo": rows[0].9,
        "totals": {
            "games_played": n,
            "total_score": total,
            "wins": rows.iter().filter(|r| r.7).count(),
            "eliminations": rows.iter().map(|r| r.5).sum::<i64>(),
            "rounds_survived": rows.iter().map(|r| r.6).sum::<i64>(),
            "average_score": round1(total as f64 / n as f64),
        },
        "games": rows.iter().map(|r| json!({
            "game_id": r.0,
            "room_code": r.1,
            "played_at": clock::iso(r.2.as_deref()),
            "character_name": r.3,
            "score": r.4,
            "eliminations": r.5,
            "rounds_survived": r.6,
            "is_winner": r.7,
            "is_eliminated": r.8,
        })).collect::<Vec<_>>(),
    })))
}
