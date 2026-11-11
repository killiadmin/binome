//! Salons : création, arrivée, prêt, réglages, départ.

use rand::RngExt;
use rusqlite::Transaction;
use serde_json::{json, Value};

use crate::clock::now_db;
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Input, V};
use crate::realtime::hub::Event;
use crate::services::common::*;
use crate::services::{Events, Out};

const PSEUDO_MESSAGES: &[(&str, &str)] = &[
    ("pseudo.required", "Le pseudo est requis."),
    ("pseudo.min", "Le pseudo doit faire au moins 2 caractères."),
    ("pseudo.max", "Le pseudo ne peut pas dépasser 20 caractères."),
];

fn room_channel(id: i64) -> String {
    format!("room.{id}")
}

/// Joueur par pseudo (comparaison insensible casse / accents), créé au besoin.
fn first_or_create_player(tx: &Transaction, pseudo: &str) -> AppResult<Player> {
    if let Some(p) = tx.opt("SELECT id, pseudo FROM players WHERE pseudo = ?1", [pseudo], |r| Ok(Player { id: r.get(0)?, pseudo: r.get(1)? }))? {
        return Ok(p);
    }
    let now = now_db();
    let id = tx.insert("INSERT INTO players (pseudo, created_at, updated_at) VALUES (?1, ?2, ?2)", (pseudo, &now))?;
    Ok(Player { id, pseudo: pseudo.to_string() })
}

/// Un pseudo = un joueur : refusé s'il est connecté ailleurs (présence en mémoire).
fn pseudo_in_use(tx: &Transaction, env: &Env, pseudo: &str) -> AppResult<()> {
    let Some(p) = tx.opt("SELECT id, pseudo FROM players WHERE pseudo = ?1", [pseudo], |r| Ok(Player { id: r.get(0)?, pseudo: r.get(1)? }))? else {
        return Ok(());
    };
    let rooms = tx.all(
        "SELECT r.id, r.code, (SELECT g.id FROM games g WHERE g.room_id = r.id AND g.status = 'in_progress' ORDER BY g.id LIMIT 1)
         FROM rooms r JOIN room_player rp ON rp.room_id = r.id WHERE rp.player_id = ?1 ORDER BY rp.id",
        [p.id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?)),
    )?;
    for (room_id, code, game_id) in rooms {
        let online = env.hub.is_online(&room_channel(room_id), p.id) || game_id.is_some_and(|g| env.hub.is_online(&format!("game.{g}"), p.id));
        if online {
            return Err(AppError::rule(format!(
                "Le pseudo « {} » est déjà connecté sur un autre appareil (salon {code}). Choisis un autre pseudo, ou quitte ce salon sur l'autre appareil.",
                p.pseudo
            )));
        }
    }
    Ok(())
}

fn generate_code(tx: &Transaction) -> AppResult<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    loop {
        let code: String = (0..6).map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char).collect();
        if !tx.exists("SELECT 1 FROM rooms WHERE code = ?1", [&code])? {
            return Ok(code);
        }
    }
}

fn player_json(tx: &Transaction, env: &Env, p: &Player) -> AppResult<Value> {
    let urls = avatar_urls(tx, env, std::iter::once(p.pseudo.as_str()))?;
    Ok(json!({ "id": p.id, "pseudo": p.pseudo, "avatar_url": urls.get(&crate::support::text::ci_key(&p.pseudo)) }))
}

/// POST /rooms
pub fn store(tx: &Transaction, env: &Env, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input).messages(&[
        PSEUDO_MESSAGES[0],
        PSEUDO_MESSAGES[1],
        PSEUDO_MESSAGES[2],
        ("max_players.min", "Il faut au minimum 4 joueurs."),
        ("max_players.max", "Le salon ne peut pas dépasser 12 joueurs."),
    ]);
    let pseudo = v.req_str("pseudo", Some(2), Some(20));
    let is_private = v.opt_bool("is_private");
    let max = v.opt_int("max_players");
    v.min_int("max_players", max, 4);
    v.max_int("max_players", max, 12);
    v.finish()?;
    let pseudo = pseudo.expect("validé");

    pseudo_in_use(tx, env, &pseudo)?;
    let player = first_or_create_player(tx, &pseudo)?;
    let code = generate_code(tx)?;
    let now = now_db();
    let is_private = is_private.unwrap_or(false);
    let max = max.unwrap_or(12);
    let room_id = tx.insert(
        "INSERT INTO rooms (code, is_private, max_players, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        (&code, is_private, max, player.id, &now),
    )?;
    tx.run("INSERT INTO room_player (room_id, player_id, is_ready, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?3)", (room_id, player.id, &now))?;

    Ok(Out::created(json!({
        "message": "Salon créé avec succès.",
        "room": { "id": room_id, "code": code, "is_private": is_private, "max_players": max },
        "player": player_json(tx, env, &player)?,
    })))
}

/// POST /rooms/join
pub fn join(tx: &Transaction, env: &Env, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input).messages(&[
        PSEUDO_MESSAGES[0],
        PSEUDO_MESSAGES[1],
        PSEUDO_MESSAGES[2],
        ("code.required", "Le code du salon est requis."),
        ("code.size", "Le code doit faire exactement 6 caractères."),
        ("code.exists", "Ce salon n'existe pas."),
    ]);
    let pseudo = v.req_str("pseudo", Some(2), Some(20));
    let code = v.req_str("code", None, None);
    v.size_str("code", code.as_ref(), 6);
    let room = match (&code, v.ok() || !input.0.contains_key("code")) {
        (Some(c), _) if c.chars().count() == 6 => room_by_code(tx, c)?,
        _ => None,
    };
    if code.as_ref().is_some_and(|c| c.chars().count() == 6) && room.is_none() {
        v.fail("code", "exists", "The selected code is invalid.".into());
    }
    v.finish()?;
    let (pseudo, room) = (pseudo.expect("validé"), room.expect("validé"));

    if room.is_locked {
        return Err(AppError::forbidden("Ce salon est verrouillé."));
    }
    let count: i64 = tx.one("SELECT COUNT(*) FROM room_player WHERE room_id = ?1", [room.id])?;
    if count >= room.max_players {
        return Err(AppError::rule("Ce salon est complet."));
    }
    pseudo_in_use(tx, env, &pseudo)?;
    let player = first_or_create_player(tx, &pseudo)?;
    if tx.exists("SELECT 1 FROM room_player WHERE room_id = ?1 AND player_id = ?2", [room.id, player.id])? {
        return Err(AppError::rule("Tu es déjà dans ce salon."));
    }
    let now = now_db();
    tx.run("INSERT INTO room_player (room_id, player_id, is_ready, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?3)", (room.id, player.id, &now))?;

    let players = players_payload(tx, env, room.id)?;
    let joined = Event::on(
        room_channel(room.id),
        "player.joined",
        json!({ "player": { "id": player.id, "pseudo": player.pseudo, "is_ready": false }, "players": players }),
    );

    Ok(Out::ok(json!({
        "message": "Tu as rejoint le salon.",
        "room": {
            "id": room.id,
            "code": room.code,
            "game_mode": room.game_mode,
            "cosmos_id": room.cosmos_id,
            "cosmos_name": cosmos_name(tx, room.cosmos_id)?,
            "reveal_to_eliminated": room.reveal_to_eliminated,
            "best_question_vote": room.best_question_vote,
            "current_game_id": current_game_id(tx, room.id)?,
            "players": players,
        },
        "player": player_json(tx, env, &player)?,
    }))
    .event(joined))
}

/// GET /rooms/{room}
pub fn show(c: &impl Q, env: &Env, room_id: i64) -> AppResult<Out> {
    let room = room_or_404(c, room_id)?;
    Ok(Out::ok(json!({
        "room": {
            "id": room.id,
            "code": room.code,
            "is_locked": room.is_locked,
            "max_players": room.max_players,
            "game_mode": room.game_mode,
            "cosmos_id": room.cosmos_id,
            "cosmos_name": cosmos_name(c, room.cosmos_id)?,
            "reveal_to_eliminated": room.reveal_to_eliminated,
            "best_question_vote": room.best_question_vote,
            "created_by": room.created_by,
            "current_game_id": current_game_id(c, room.id)?,
            "players": players_payload(c, env, room.id)?,
        }
    })))
}

fn is_member(c: &impl Q, room_id: i64, player_id: i64) -> AppResult<bool> {
    c.exists("SELECT 1 FROM room_player WHERE room_id = ?1 AND player_id = ?2", [room_id, player_id])
}

/// PATCH /rooms/{room}/ready — bascule
pub fn ready(tx: &Transaction, env: &Env, room_id: i64, input: &Input) -> AppResult<Out> {
    let room = room_or_404(tx, room_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    v.finish()?;
    let pid = pid.expect("validé");
    if !is_member(tx, room.id, pid)? {
        return Err(AppError::forbidden("Ce joueur n'est pas dans ce salon."));
    }
    let current: bool = tx.one("SELECT is_ready FROM room_player WHERE room_id = ?1 AND player_id = ?2", [room.id, pid])?;
    let ready = !current;
    tx.run("UPDATE room_player SET is_ready = ?1, updated_at = ?2 WHERE room_id = ?3 AND player_id = ?4", (ready, now_db(), room.id, pid))?;

    let players = players_payload(tx, env, room.id)?;
    let list = room_players(tx, room.id)?;
    let all_ready = list.iter().all(|p| p.2);
    let event = Event::on(room_channel(room.id), "player.ready", json!({ "player_id": pid, "players": players, "can_start": all_ready }));

    Ok(Out::ok(json!({
        "message": if ready { "Tu es prêt !" } else { "Tu n'es plus prêt." },
        "is_ready": ready,
        "all_ready": all_ready,
        "can_start": all_ready && list.len() >= 4,
        "players": players,
    }))
    .event(event))
}

/// PATCH /rooms/{room}/settings — hôte
pub fn settings(tx: &Transaction, room_id: i64, input: &Input) -> AppResult<Out> {
    let room = room_or_404(tx, room_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let mode = v.req_str("game_mode", None, None);
    v.in_list("game_mode", mode.as_ref(), &["random", "cosmos"]);
    let cosmos_id = if mode.as_deref() == Some("cosmos") && input.get("cosmos_id").is_none() {
        v.fail("cosmos_id", "required_if", "The cosmos id field is required when game mode is cosmos.".into());
        None
    } else {
        let id = v.opt_int("cosmos_id");
        v.exists(tx, "cosmos_id", id, "SELECT 1 FROM cosmos WHERE id = ?1")?
    };
    let reveal = v.opt_bool("reveal_to_eliminated");
    let best = v.opt_bool("best_question_vote");
    v.finish()?;
    let (pid, mode) = (pid.expect("validé"), mode.expect("validé"));

    if room.created_by != pid {
        return Err(AppError::forbidden("Seul le créateur du salon peut configurer la partie."));
    }
    let cosmos_id = if mode == "cosmos" { cosmos_id } else { None };
    let reveal = reveal.unwrap_or(room.reveal_to_eliminated);
    let best = best.unwrap_or(room.best_question_vote);
    tx.run(
        "UPDATE rooms SET game_mode = ?1, cosmos_id = ?2, reveal_to_eliminated = ?3, best_question_vote = ?4, updated_at = ?5 WHERE id = ?6",
        (&mode, cosmos_id, reveal, best, now_db(), room.id),
    )?;
    let body = json!({
        "game_mode": mode,
        "cosmos_id": cosmos_id,
        "cosmos_name": cosmos_name(tx, cosmos_id)?,
        "reveal_to_eliminated": reveal,
        "best_question_vote": best,
    });
    let mut response = json!({ "message": "Configuration mise à jour." });
    response.as_object_mut().expect("objet").extend(body.as_object().expect("objet").clone());
    Ok(Out::ok(response).event(Event::on(room_channel(room.id), "room.settings.updated", body)))
}

/// Retire un joueur du salon (`Room::removePlayer`) : salon supprimé s'il se
/// vide (ses parties partent avec lui, cascade identique à l'ancien schéma),
/// sinon l'hôte passe au plus ancien joueur restant, puis `player.left`.
pub fn remove_player(tx: &Transaction, env: &Env, room: &Room, player_id: i64, events: &mut Events) -> AppResult<()> {
    tx.run("DELETE FROM room_player WHERE room_id = ?1 AND player_id = ?2", [room.id, player_id])?;
    let left: i64 = tx.one("SELECT COUNT(*) FROM room_player WHERE room_id = ?1", [room.id])?;
    if left == 0 {
        tx.run("DELETE FROM rooms WHERE id = ?1", [room.id])?;
        return Ok(());
    }
    let mut new_host = None;
    if room.created_by == player_id {
        let id: i64 = tx.one("SELECT player_id FROM room_player WHERE room_id = ?1 ORDER BY id LIMIT 1", [room.id])?;
        tx.run("UPDATE rooms SET created_by = ?1, updated_at = ?2 WHERE id = ?3", (id, now_db(), room.id))?;
        new_host = Some(id);
    }
    events.push(Event::on(
        room_channel(room.id),
        "player.left",
        json!({ "players": players_payload(tx, env, room.id)?, "new_host_id": new_host }),
    ));
    Ok(())
}

/// Le joueur a-t-il déjà joué (ses parties pointent sur lui) ?
pub fn has_played(c: &impl Q, player_id: i64) -> AppResult<bool> {
    Ok(c.exists("SELECT 1 FROM binome_player WHERE player_id = ?1", [player_id])?
        || c.exists("SELECT 1 FROM actions WHERE player_id = ?1", [player_id])?)
}

/// DELETE /rooms/{room}/leave
pub fn leave(tx: &Transaction, env: &Env, room_id: i64, input: &Input) -> AppResult<Out> {
    let room = room_or_404(tx, room_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    v.finish()?;
    let pid = pid.expect("validé");
    if !is_member(tx, room.id, pid)? {
        return Err(AppError::forbidden("Ce joueur n'est pas dans ce salon."));
    }
    if let Some(game_id) = current_game_id(tx, room.id)? {
        if has_seat(tx, game_id, pid)? {
            return Err(AppError::rule(
                "Une partie est en cours : tu ne peux pas quitter le salon avant la fin. Si tu dois partir, l'hôte peut t'exclure.",
            ));
        }
    }
    let count: i64 = tx.one("SELECT COUNT(*) FROM room_player WHERE room_id = ?1", [room.id])?;
    let mut events = vec![];
    remove_player(tx, env, &room, pid, &mut events)?;
    if !has_played(tx, pid)? {
        tx.run("DELETE FROM players WHERE id = ?1", [pid])?;
    }
    let msg = if count == 1 { "Salon supprimé car vide." } else { "Tu as quitté le salon." };
    Ok(Out::message(msg).with(events))
}
