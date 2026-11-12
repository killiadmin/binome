//! Lancement d'une partie, tirage des binômes et personnages, vote des
//! personnages, état public de la partie, `/me` et vue omnisciente.

use std::collections::BTreeMap;

use rand::seq::SliceRandom;
use rusqlite::Transaction;
use serde_json::{json, Value};

use crate::clock::{self, now_db};
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Input, V};
use crate::realtime::hub::Event;
use crate::services::common::*;
use crate::services::play::create_round;
use crate::services::{Events, Out};

pub fn game_channel(id: i64) -> String {
    format!("game.{id}")
}

/// POST /rooms/{room}/start
pub fn start(tx: &Transaction, _env: &Env, room_id: i64, input: &Input) -> AppResult<Out> {
    let room = room_or_404(tx, room_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    v.finish()?;
    if room.created_by != pid.expect("validé") {
        return Err(AppError::forbidden("Seul le créateur du salon peut lancer la partie."));
    }
    if current_game_id(tx, room.id)?.is_some() {
        return Err(AppError::rule("Une partie est déjà en cours dans ce salon."));
    }

    let players = room_players(tx, room.id)?;
    if players.len() < 4 {
        return Err(AppError::unprocessable("Il faut au minimum 4 joueurs pour démarrer."));
    }
    if !players.iter().all(|p| p.2) {
        return Err(AppError::unprocessable("Tous les joueurs doivent être prêts."));
    }

    let now = now_db();
    let game_id = tx.insert(
        "INSERT INTO games (room_id, room_code, status, draw_number, created_at, updated_at) VALUES (?1, ?2, 'in_progress', 1, ?3, ?3)",
        (room.id, &room.code, &now),
    )?;
    let ids: Vec<i64> = players.iter().map(|p| p.0).collect();
    assign_binomes(tx, game_id, &ids, &room, &[])?;

    let mut seated = seats(tx, game_id)?;
    seated.sort_by_key(|s| s.player_id);
    let started = Event::new(
        vec![format!("room.{}", room.id), game_channel(game_id)],
        "game.started",
        json!({
            "game_id": game_id,
            "status": "in_progress",
            "players": seated.iter().map(|s| json!({ "id": s.player_id, "pseudo": s.pseudo })).collect::<Vec<_>>(),
            "has_orphan": seated.iter().any(|s| s.is_orphan),
            "draw_number": 1,
        }),
    );
    Ok(Out::created(json!({ "message": "La partie a démarré.", "game_id": game_id, "status": "in_progress" })).event(started))
}

/// Forme les binômes (mélange, paires, orphelin si impair) et distribue les personnages.
/// Les personnages refusés sont écartés tant que le catalogue le permet.
fn assign_binomes(tx: &Transaction, game_id: i64, players: &[i64], room: &Room, excluded: &[i64]) -> AppResult<()> {
    let mut shuffled = players.to_vec();
    shuffled.shuffle(&mut rand::rng());
    let teams: Vec<Vec<i64>> = shuffled.chunks(2).map(<[i64]>::to_vec).collect();

    let draw = match (!excluded.is_empty()).then(|| draw_characters(tx, &teams, room, excluded)) {
        Some(Ok(d)) => d,
        // Pas assez de personnages sans les refusés : tirage sur tout le catalogue
        _ => draw_characters(tx, &teams, room, &[])?,
    };

    let now = now_db();
    for (members, (universe_id, characters)) in teams.iter().zip(draw) {
        let binome_id = tx.insert(
            "INSERT INTO binomes (game_id, universe_id, is_orphan, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            (game_id, universe_id, members.len() == 1, &now),
        )?;
        for (player, character) in members.iter().zip(characters) {
            tx.run(
                "INSERT INTO binome_player (binome_id, player_id, character_id, score, created_at, updated_at) VALUES (?1, ?2, ?3, 0, ?4, ?4)",
                (binome_id, player, character, &now),
            )?;
        }
    }
    Ok(())
}

/// Univers + personnages de chaque équipe, sans rien écrire.
fn draw_characters(tx: &Transaction, teams: &[Vec<i64>], room: &Room, excluded: &[i64]) -> AppResult<Vec<(i64, Vec<i64>)>> {
    let universes = resolve_universes(tx, teams.len(), room, excluded)?;
    let mut rng = rand::rng();
    let mut out = vec![];
    for (members, (universe_id, universe_name)) in teams.iter().zip(universes) {
        let mut groups: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for (id, level) in tx.all(
            "SELECT id, level_affectation FROM characters WHERE verif_manual = 1 AND hidden = 0 AND universe_id = ?1",
            [universe_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )? {
            if !excluded.contains(&id) {
                groups.entry(level).or_default().push(id);
            }
        }
        let mut valid: Vec<Vec<i64>> = groups.into_values().filter(|g| g.len() >= 2).collect();
        if valid.is_empty() {
            return Err(AppError::unprocessable(format!(
                "L'univers {universe_name} n'a pas de binome de personnages valide (niveau d'affectation)."
            )));
        }
        valid.shuffle(&mut rng);
        let mut group = valid.swap_remove(0);
        group.shuffle(&mut rng);
        group.truncate(members.len());
        out.push((universe_id, group));
    }
    Ok(out)
}

/// `teams` univers distincts jouables (≥ 2 personnages jouables au même niveau),
/// restreints au cosmos du salon en mode `cosmos`.
fn resolve_universes(tx: &Transaction, teams: usize, room: &Room, excluded: &[i64]) -> AppResult<Vec<(i64, String)>> {
    let excluded_json = serde_json::to_string(excluded).expect("json");
    let cosmos = (room.game_mode == "cosmos").then_some(room.cosmos_id).flatten();
    let cosmos_filter = if room.game_mode == "cosmos" { "AND u.cosmos_id IS ?2" } else { "AND ?2 IS ?2" };
    let picked = tx.all(
        &format!(
            "SELECT u.id, u.name FROM universes u
             WHERE u.id IN (
                 SELECT universe_id FROM characters
                 WHERE verif_manual = 1 AND hidden = 0 AND id NOT IN (SELECT value FROM json_each(?1))
                 GROUP BY universe_id, level_affectation HAVING COUNT(*) >= 2
             ) {cosmos_filter}
             ORDER BY RANDOM() LIMIT ?3"
        ),
        (&excluded_json, cosmos, teams as i64),
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )?;
    if picked.len() < teams {
        if room.game_mode == "cosmos" {
            let name = cosmos_name(tx, room.cosmos_id)?.unwrap_or_default();
            return Err(AppError::unprocessable(format!(
                "Le cosmos « {name} » n'a que {} univers jouable(s) pour {teams} binôme(s). Choisis un autre cosmos ou ajoute des personnages.",
                picked.len()
            )));
        }
        return Err(AppError::unprocessable("Pas assez d'univers avec des personnages validés pour cette partie."));
    }
    Ok(picked)
}

/// Votes en cours, par id de joueur croissant : (acceptés, changements, personnages à changer, total).
pub fn character_votes(c: &impl Q, game_id: i64) -> AppResult<(Vec<i64>, Vec<i64>, Vec<i64>, usize)> {
    let rows = c.all(
        "SELECT bp.player_id, bp.character_id, bp.character_vote FROM binome_player bp JOIN binomes b ON b.id = bp.binome_id
         WHERE b.game_id = ?1 ORDER BY bp.player_id",
        [game_id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?)),
    )?;
    let accepted = rows.iter().filter(|r| r.2.as_deref() == Some("accepted")).map(|r| r.0).collect();
    let change: Vec<&(i64, i64, Option<String>)> = rows.iter().filter(|r| r.2.as_deref() == Some("change")).collect();
    Ok((accepted, change.iter().map(|r| r.0).collect(), change.iter().map(|r| r.1).collect(), rows.len()))
}

/// POST /games/{game}/character-vote
pub fn vote_character(tx: &Transaction, _env: &Env, game_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(tx, game_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let character_id = v.req_int("character_id");
    let accepted = v.req_bool("accepted");
    v.finish()?;
    let (pid, character_id, accepted) = (pid.expect("validé"), character_id.expect("validé"), accepted.expect("validé"));

    if !g.voting() {
        return Err(AppError::rule("Le choix des personnages est terminé."));
    }
    let seat = seats(tx, g.id)?.into_iter().find(|s| s.player_id == pid).ok_or_else(|| AppError::rule("Tu ne joues pas dans cette partie."))?;
    if seat.character_id != character_id {
        return Err(AppError::rule("Les personnages ont été tirés à nouveau : regarde ton nouveau personnage."));
    }
    if seat.character_vote.is_some() {
        return Err(AppError::rule("Tu as déjà voté pour ce tirage."));
    }
    if !accepted && g.character_changes_left(pid) == 0 {
        return Err(AppError::rule(format!(
            "Tu as déjà changé {MAX_CHARACTER_CHANGES} fois de personnage : tu joues avec celui-ci."
        )));
    }
    tx.run(
        "UPDATE binome_player SET character_vote = ?1, updated_at = ?2 WHERE binome_id = ?3 AND player_id = ?4",
        (if accepted { "accepted" } else { "change" }, now_db(), seat.binome_id, pid),
    )?;

    let (acc, change, change_chars, total) = character_votes(tx, g.id)?;
    let mut events: Events = vec![];
    let vote_cast = |accepted: Vec<i64>, change: Vec<i64>, draw: i64| {
        Event::on(
            game_channel(g.id),
            "character.vote.cast",
            json!({ "game_id": g.id, "draw_number": draw, "accepted_player_ids": accepted, "change_player_ids": change }),
        )
    };

    if acc.len() + change.len() < total {
        events.push(vote_cast(acc, change, g.draw_number));
    } else if !change.is_empty() {
        let changes = redraw(tx, &g, &change, &change_chars)?;
        events.push(Event::on(
            game_channel(g.id),
            "characters.redrawn",
            json!({
                "game_id": g.id,
                "draw_number": g.draw_number + 1,
                "change_count": change.len(),
                "change_player_ids": change,
                "character_changes_left": changes_left_list(tx, &changes)?,
            }),
        ));
    } else {
        tx.run("UPDATE games SET characters_confirmed_at = ?1, updated_at = ?1 WHERE id = ?2", (now_db(), g.id))?;
        create_round(tx, g.id, 1, &mut events)?;
        events.push(vote_cast(acc, vec![], g.draw_number));
    }

    Ok(Out::message(if accepted { "Personnage accepté." } else { "Demande de changement enregistrée." }).with(events))
}

/// Nouveau tirage complet avec les mêmes joueurs (pas les spectateurs arrivés depuis).
/// Chaque demandeur consomme une de ses demandes de changement. Renvoie la partie à jour.
fn redraw(tx: &Transaction, g: &Game, askers: &[i64], rejected_now: &[i64]) -> AppResult<Game> {
    let players: Vec<i64> = seats(tx, g.id)?.iter().map(|s| s.player_id).collect();
    let mut rejected = g.rejected_character_ids.clone();
    for id in rejected_now {
        if !rejected.contains(id) {
            rejected.push(*id);
        }
    }
    let mut changes = g.character_changes.clone();
    for id in askers {
        *changes.entry(*id).or_default() += 1;
    }
    tx.run("DELETE FROM binomes WHERE game_id = ?1", [g.id])?;
    tx.run(
        "UPDATE games SET draw_number = draw_number + 1, rejected_character_ids = ?1, character_changes = ?2, updated_at = ?3 WHERE id = ?4",
        (serde_json::to_string(&rejected).expect("json"), serde_json::to_string(&changes).expect("json"), now_db(), g.id),
    )?;
    // Une partie en cours a toujours son salon (ses joueurs ne peuvent pas le quitter)
    let room = game_room(tx, g)?.ok_or_else(|| AppError::rule("Le salon de cette partie n'existe plus."))?;
    assign_binomes(tx, g.id, &players, &room, &rejected)?;
    game_or_404(tx, g.id)
}

/// Demandes de changement restantes de chaque joueur assis, par id croissant.
fn changes_left_list(tx: &Transaction, g: &Game) -> AppResult<Vec<Value>> {
    let mut ids: Vec<i64> = seats(tx, g.id)?.iter().map(|s| s.player_id).collect();
    ids.sort_unstable();
    Ok(ids.into_iter().map(|id| json!({ "player_id": id, "left": g.character_changes_left(id) })).collect())
}

/// Action telle que la montre le journal public de la partie.
pub fn public_action(a: &Action, round_number: i64, names: &std::collections::HashMap<i64, String>) -> Value {
    let who = |id: i64| json!({ "id": id, "pseudo": names.get(&id) });
    let target = a.target_player_id.map(who).unwrap_or(Value::Null);
    let mut v = json!({
        "action_id": a.id,
        "id": a.id,
        "round_id": a.round_id,
        "round_number": round_number,
        "player": who(a.player_id),
        "type": a.kind,
        "is_valid": a.is_valid,
        "answer": a.answer,
        "answer_corrected_from": a.answer_corrected_from,
        "created_at": clock::iso(a.created_at.as_deref()),
        "target_player": target,
    });
    let o = v.as_object_mut().expect("objet");
    if a.is_question() {
        o.insert("question".into(), if a.is_valid { json!(a.content) } else { Value::Null });
        o.insert("refused_reason".into(), if a.is_valid { Value::Null } else { json!("Mot interdit détecté.") });
        o.insert("card_draw".into(), a.public_card_draw());
    } else {
        o.insert("accusation_correct".into(), json!(a.accusation_correct));
        o.insert("accusation_confirmed".into(), json!(a.accusation_confirmed));
        o.insert("character_name".into(), json!(a.character_name.as_ref().or(a.content.as_ref())));
    }
    v
}

/// GET /games/{game} — état public : jamais de personnage ni de binôme non découvert.
pub fn show(c: &impl Q, env: &Env, game_id: i64) -> AppResult<Out> {
    let g = game_or_404(c, game_id)?;
    let room = game_room(c, &g)?;
    let mut seated = seats(c, g.id)?;
    seated.sort_by_key(|s| s.player_id);
    let avatars = avatar_urls(c, env, seated.iter().map(|s| s.pseudo.as_str()))?;
    let all_rounds = rounds(c, g.id)?;
    let current = all_rounds.iter().filter(|r| !r.is_finished).max_by_key(|r| r.number);
    let actions = game_actions(c, g.id)?;
    let names = pseudos(c, actions.iter().flat_map(|a| [Some(a.player_id), a.target_player_id]).flatten())?;
    let number_of = |round_id: i64| all_rounds.iter().find(|r| r.id == round_id).map_or(0, |r| r.number);
    let mut journal: Vec<(i64, i64, Value)> = actions.iter().map(|a| (number_of(a.round_id), a.id, public_action(a, number_of(a.round_id), &names))).collect();
    journal.sort_by_key(|(n, id, _)| (*n, *id));

    let voting = g.voting();
    let (accepted, change, _, _) = if voting { character_votes(c, g.id)? } else { (vec![], vec![], vec![], 0) };
    let question_votes = c.all(
        "SELECT round_id, action_id, voter_id FROM question_votes WHERE round_id IN (SELECT id FROM rounds WHERE game_id = ?1) ORDER BY id",
        [g.id],
        |r| Ok(json!({ "round_id": r.get::<_, i64>(0)?, "action_id": r.get::<_, i64>(1)?, "voter_id": r.get::<_, i64>(2)? })),
    )?;

    Ok(Out::ok(json!({
        "game_id": g.id,
        "status": g.status,
        "players": seated.iter().map(|s| json!({
            "id": s.player_id,
            "pseudo": s.pseudo,
            "is_eliminated": s.is_eliminated,
            "is_excluded": s.is_excluded,
            "avatar_url": avatars.get(&crate::support::text::ci_key(&s.pseudo)),
            "character_changes_left": g.character_changes_left(s.player_id),
        })).collect::<Vec<_>>(),
        "max_character_changes": MAX_CHARACTER_CHANGES,
        "has_orphan": seated.iter().any(|s| s.is_orphan),
        "host_id": room.as_ref().map(|r| r.created_by),
        "characters_confirmed": !voting,
        "draw_number": g.draw_number,
        "settings": {
            "reveal_to_eliminated": room.as_ref().is_some_and(|r| r.reveal_to_eliminated),
            "best_question_vote": room.as_ref().is_some_and(|r| r.best_question_vote),
        },
        "question_votes": question_votes,
        "accepted_player_ids": accepted,
        "change_player_ids": change,
        // Aucun binôme n'est jamais « découvert » (la découverte a été retirée du jeu) :
        // liste toujours vide, conservée pour le contrat.
        "binomes": [],
        "current_round": current.map(|r| json!({ "id": r.id, "number": r.number, "current_player_id": r.current_player_id })),
        "actions": journal.into_iter().map(|(_, _, v)| v).collect::<Vec<_>>(),
    })))
}

/// Univers d'un personnage : (nom de l'univers, nom du cosmos).
pub fn universe_names(c: &impl Q, universe_id: i64) -> AppResult<(String, Option<String>)> {
    Ok(c.opt(
        "SELECT u.name, co.name FROM universes u LEFT JOIN cosmos co ON co.id = u.cosmos_id WHERE u.id = ?1",
        [universe_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?
    .unwrap_or_default())
}

/// GET /games/{game}/me
pub fn me(c: &impl Q, env: &Env, game_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(c, game_id)?;
    let mut v = V::new(input);
    let pid = v.player(c, "player_id")?;
    v.finish()?;
    let seat = seats(c, g.id)?.into_iter().find(|s| Some(s.player_id) == pid);
    let Some(ch) = seat.map(|s| s.character_id).map(|id| character(c, id)).transpose()?.flatten() else {
        return Err(AppError::Status(axum::http::StatusCode::NOT_FOUND, "Aucun personnage trouvé pour ce joueur dans cette partie.".into()));
    };
    let (universe, cosmos) = universe_names(c, ch.universe_id)?;
    Ok(Out::ok(json!({
        "character": {
            "id": ch.id,
            "name": ch.name,
            "image": character_image_url(env, ch.id, ch.has_image, ch.updated_at.as_deref()),
            "universe": universe,
            "cosmos": cosmos,
            "forbidden_words": ch.forbidden_words,
        }
    })))
}

/// GET /games/{game}/reveal — éliminés uniquement, si l'hôte l'a activé.
pub fn reveal(c: &impl Q, env: &Env, game_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(c, game_id)?;
    let mut v = V::new(input);
    let pid = v.player(c, "player_id")?;
    v.finish()?;
    let room = game_room(c, &g)?;
    if !room.is_some_and(|r| r.reveal_to_eliminated) {
        return Err(AppError::forbidden("La vue omnisciente est désactivée pour cette partie."));
    }
    let seated = seats(c, g.id)?;
    let me = seated.iter().find(|s| Some(s.player_id) == pid);
    if me.is_none_or(|m| !m.is_eliminated && !g.finished()) {
        return Err(AppError::forbidden("Réservé aux joueurs éliminés."));
    }

    let mut binomes: Vec<(i64, Vec<&Seat>)> = vec![];
    for s in &seated {
        match binomes.iter_mut().find(|(id, _)| *id == s.binome_id) {
            Some((_, list)) => list.push(s),
            None => binomes.push((s.binome_id, vec![s])),
        }
    }
    let mut out = vec![];
    for (id, mut members) in binomes {
        members.sort_by_key(|s| s.player_id);
        let (universe, cosmos) = universe_names(c, members[0].universe_id)?;
        let mut players = vec![];
        for s in &members {
            let ch = character(c, s.character_id)?;
            players.push(json!({
                "id": s.player_id,
                "pseudo": s.pseudo,
                "character": ch.as_ref().map(|c| c.name.clone()),
                "image": ch.as_ref().map_or(Value::Null, |c| character_image_url(env, c.id, c.has_image, c.updated_at.as_deref())),
                "forbidden_words": ch.as_ref().map(|c| c.forbidden_words.clone()).unwrap_or_default(),
                "is_eliminated": s.is_eliminated,
            }));
        }
        out.push(json!({ "id": id, "universe": universe, "cosmos": cosmos, "is_orphan": members[0].is_orphan, "players": players }));
    }
    Ok(Out::ok(json!({ "binomes": out })))
}
