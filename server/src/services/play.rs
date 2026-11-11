//! Déroulement d'une partie (ports d'ActionService, RoundService, BinomeService).
//!
//! Toutes les transitions sensibles sont des mises à jour conditionnelles
//! (`answer IS NULL`, `card_draw_status`, `current_player_id = from`…) : réponse,
//! passage de tour et expiration simultanés → un seul l'emporte, et le tour
//! n'avance jamais deux fois. Les écritures étant de toute façon sérialisées
//! (transactions IMMEDIATE), c'est une double sécurité.

use rand::seq::IndexedRandom;
use rusqlite::Transaction;
use serde_json::{json, Value};

use crate::clock::{self, now_db};
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Input, V};
use crate::realtime::hub::Event;
use crate::services::common::*;
use crate::services::game::{game_channel, public_action};
use crate::services::score;
use crate::services::{Events, Followup, Out};
use crate::support::text::squish_ascii;

/// Délai pour tirer la carte ET décider de la montrer (ActionService::CARD_DRAW_SECONDS).
pub const CARD_DRAW_SECONDS: i64 = 30;
/// Durée d'affichage du mot montré (CardWordShared::REVEAL_SECONDS).
pub const REVEAL_SECONDS: i64 = 5;

// ─── Modèles « à la Eloquent » (réponses brutes de quelques routes) ──────────

fn player_model(c: &impl Q, id: i64) -> AppResult<Value> {
    Ok(c.opt("SELECT id, pseudo, created_at, updated_at FROM players WHERE id = ?1", [id], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "pseudo": r.get::<_, String>(1)?,
            "created_at": clock::eloquent(r.get::<_, Option<String>>(2)?.as_deref()),
            "updated_at": clock::eloquent(r.get::<_, Option<String>>(3)?.as_deref()),
        }))
    })?
    .unwrap_or(Value::Null))
}

fn round_model(c: &impl Q, id: i64) -> AppResult<Value> {
    Ok(c.opt("SELECT id, game_id, number, current_player_id, is_finished, created_at, updated_at FROM rounds WHERE id = ?1", [id], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "game_id": r.get::<_, i64>(1)?,
            "number": r.get::<_, i64>(2)?,
            "current_player_id": r.get::<_, i64>(3)?,
            "is_finished": r.get::<_, i64>(4)?,
            "created_at": clock::eloquent(r.get::<_, Option<String>>(5)?.as_deref()),
            "updated_at": clock::eloquent(r.get::<_, Option<String>>(6)?.as_deref()),
        }))
    })?
    .unwrap_or(Value::Null))
}

/// Action sérialisée comme un modèle Eloquent. `fresh` : modèle juste créé
/// (seuls les attributs fournis à la création), sinon toutes les colonnes.
/// Le mot tiré (`drawn_word`) n'y figure jamais.
fn action_model(c: &impl Q, a: &Action, fresh: bool, round: Value) -> AppResult<Value> {
    let mut v = json!({
        "id": a.id,
        "round_id": a.round_id,
        "player_id": a.player_id,
        "target_player_id": a.target_player_id,
        "type": a.kind,
        "content": a.content,
        "is_valid": a.is_valid,
        "accusation_correct": a.accusation_correct,
        "created_at": clock::eloquent(a.created_at.as_deref()),
        "updated_at": clock::eloquent(a.updated_at.as_deref()),
        "player": player_model(c, a.player_id)?,
        "target_player": match a.target_player_id { Some(t) => player_model(c, t)?, None => Value::Null },
        "round": round,
    });
    let o = v.as_object_mut().expect("objet");
    if !fresh || a.character_name.is_some() {
        o.insert("character_name".into(), json!(a.character_name));
    }
    if !fresh {
        o.insert("accusation_confirmed".into(), json!(a.accusation_confirmed));
        o.insert("answer".into(), json!(a.answer));
        o.insert("answer_corrected_from".into(), json!(a.answer_corrected_from));
        o.insert("answer_corrected_at".into(), json!(clock::eloquent(a.answer_corrected_at.as_deref())));
        o.insert("card_draw_status".into(), json!(a.card_draw_status));
        o.insert("card_count".into(), json!(a.card_count));
        o.insert("card_draw_deadline".into(), json!(clock::eloquent(a.card_draw_deadline.as_deref())));
    }
    Ok(v)
}

// ─── Chargements et contrôles ───────────────────────────────────────────────

/// `/games/{game}/rounds/{round}[/actions/{action}]` : le round doit appartenir
/// à la partie, l'action au round (scopeBindings) — sinon 404.
pub fn scoped(c: &impl Q, game_id: i64, round_id: i64, action_id: Option<i64>) -> AppResult<(Game, Round, Option<Action>)> {
    let g = game_or_404(c, game_id)?;
    let r = round(c, round_id)?.filter(|r| r.game_id == g.id).ok_or(AppError::NotFound)?;
    let a = match action_id {
        Some(id) => Some(action(c, id)?.filter(|a| a.round_id == r.id).ok_or(AppError::NotFound)?),
        None => None,
    };
    Ok((g, r, a))
}

fn alive_ordered(c: &impl Q, game_id: i64) -> AppResult<Vec<Seat>> {
    let mut s: Vec<Seat> = seats(c, game_id)?.into_iter().filter(|s| !s.is_eliminated).collect();
    s.sort_by_key(|s| s.player_id);
    Ok(s)
}

fn validate_turn(g: &Game, r: &Round, player_id: i64) -> AppResult<()> {
    if !g.in_progress() {
        return Err(AppError::rule("La partie est terminée."));
    }
    if r.current_player_id != player_id {
        return Err(AppError::rule("Ce n'est pas ton tour de jouer."));
    }
    if r.is_finished {
        return Err(AppError::rule("Ce round est déjà terminé."));
    }
    Ok(())
}

fn validate_not_played(c: &impl Q, r: &Round, player_id: i64) -> AppResult<()> {
    if c.exists("SELECT 1 FROM actions WHERE round_id = ?1 AND player_id = ?2 AND is_valid = 1", [r.id, player_id])? {
        return Err(AppError::rule("Tu as déjà joué ce round."));
    }
    Ok(())
}

fn validate_target(c: &impl Q, g: &Game, player: &Player, target: &Player) -> AppResult<()> {
    if target.id == player.id {
        return Err(AppError::rule("Tu ne peux pas te cibler toi-même."));
    }
    match seats(c, g.id)?.into_iter().find(|s| s.player_id == target.id) {
        None => Err(AppError::rule(format!("{} ne joue pas dans cette partie.", target.pseudo))),
        Some(s) if s.is_eliminated => Err(AppError::rule(format!("{} est déjà éliminé.", target.pseudo))),
        Some(_) => Ok(()),
    }
}

/// Personnage du joueur dans la partie.
fn character_in_game(c: &impl Q, game_id: i64, player_id: i64) -> AppResult<Option<Character>> {
    match seats(c, game_id)?.into_iter().find(|s| s.player_id == player_id) {
        Some(s) => character(c, s.character_id),
        None => Ok(None),
    }
}

// ─── Rounds ─────────────────────────────────────────────────────────────────

fn round_started(c: &impl Q, r: &Round, is_new: bool) -> AppResult<Event> {
    let p = player_or_404(c, r.current_player_id)?;
    Ok(Event::on(
        game_channel(r.game_id),
        "round.started",
        json!({ "round_id": r.id, "number": r.number, "is_new_round": is_new, "current_player": brief(&p) }),
    ))
}

/// Nouveau round : le premier joueur actif (ordre des ids) commence.
pub fn create_round(tx: &Transaction, game_id: i64, number: i64, events: &mut Events) -> AppResult<Round> {
    let first = alive_ordered(tx, game_id)?.into_iter().next().ok_or_else(|| AppError::Internal("aucun joueur actif".into()))?;
    let now = now_db();
    let id = tx.insert(
        "INSERT INTO rounds (game_id, number, current_player_id, is_finished, created_at, updated_at) VALUES (?1, ?2, ?3, 0, ?4, ?4)",
        (game_id, number, first.player_id, &now),
    )?;
    let r = Round { id, game_id, number, current_player_id: first.player_id, is_finished: false };
    events.push(round_started(tx, &r, true)?);
    Ok(r)
}

/// Joueur suivant ; `true` si le round vient de se terminer. Conditionnel : rien
/// ne bouge si le tour a déjà quitté `from`.
fn next_turn(tx: &Transaction, r: &Round, from: Option<i64>, events: &mut Events) -> AppResult<bool> {
    let from = from.unwrap_or(r.current_player_id);
    let next = alive_ordered(tx, r.game_id)?.into_iter().find(|s| s.player_id > from);
    let now = now_db();
    let moved = match &next {
        Some(n) => tx.run(
            "UPDATE rounds SET current_player_id = ?1, updated_at = ?2 WHERE id = ?3 AND is_finished = 0 AND current_player_id = ?4",
            (n.player_id, &now, r.id, from),
        )?,
        None => tx.run("UPDATE rounds SET is_finished = 1, updated_at = ?1 WHERE id = ?2 AND is_finished = 0 AND current_player_id = ?3", (&now, r.id, from))?,
    };
    if moved == 0 || next.is_none() {
        return Ok(moved > 0);
    }
    let fresh = round(tx, r.id)?.expect("round existant");
    events.push(round_started(tx, &fresh, false)?);
    Ok(false)
}

/// Fin de tour : joueur suivant, ou nouveau round. Rien après la fin de partie.
fn advance_round(tx: &Transaction, r: &Round, from: Option<i64>, events: &mut Events) -> AppResult<()> {
    let g = game_or_404(tx, r.game_id)?;
    if !g.in_progress() {
        return Ok(());
    }
    if next_turn(tx, r, from, events)? {
        create_round(tx, g.id, r.number + 1, events)?;
    }
    Ok(())
}

// ─── Fin de partie ──────────────────────────────────────────────────────────

/// Gagnants si la partie est finie (BinomeService::checkGameOver) :
/// une seule équipe intacte et aucun rescapé, ou aucune équipe intacte et un seul rescapé.
fn check_game_over(c: &impl Q, game_id: i64) -> AppResult<Option<Vec<Seat>>> {
    Ok(winners(seats(c, game_id)?))
}

/// Règle de fin de partie, sur les places de la partie (fonction pure).
pub fn winners(seated: Vec<Seat>) -> Option<Vec<Seat>> {
    let mut binomes: Vec<Vec<Seat>> = vec![];
    for s in seated {
        match binomes.iter_mut().find(|b| b[0].binome_id == s.binome_id) {
            Some(b) => b.push(s),
            None => binomes.push(vec![s]),
        }
    }
    let active: Vec<Seat> = binomes.iter().flatten().filter(|s| !s.is_eliminated).cloned().collect();
    if active.len() <= 1 {
        return Some(active);
    }
    let intact: Vec<&Vec<Seat>> = binomes.iter().filter(|b| b.iter().all(|s| !s.is_eliminated)).collect();
    let survivors: Vec<Seat> = binomes
        .iter()
        .filter(|b| !b.iter().all(|s| !s.is_eliminated))
        .flatten()
        .filter(|s| !s.is_eliminated)
        .cloned()
        .collect();
    if intact.len() == 1 && survivors.is_empty() {
        return Some(intact[0].clone());
    }
    if intact.is_empty() && survivors.len() == 1 {
        return Some(survivors);
    }
    None
}

fn end_game(tx: &Transaction, game_id: i64, winners: &[Seat], events: &mut Events) -> AppResult<()> {
    let now = now_db();
    tx.run("UPDATE games SET status = 'finished', updated_at = ?1 WHERE id = ?2", (&now, game_id))?;
    tx.run("UPDATE rounds SET is_finished = 1, updated_at = ?1 WHERE game_id = ?2 AND is_finished = 0", (&now, game_id))?;
    let winner_ids: Vec<i64> = winners.iter().map(|w| w.player_id).collect();
    let mut stats = score::compute_and_store(tx, game_id, &winner_ids)?;
    stats.sort_by(|a, b| b.score.cmp(&a.score));

    let seated = seats(tx, game_id)?;
    let mut all_binomes: Vec<(i64, Vec<&Seat>)> = vec![];
    for s in &seated {
        match all_binomes.iter_mut().find(|(id, _)| *id == s.binome_id) {
            Some((_, l)) => l.push(s),
            None => all_binomes.push((s.binome_id, vec![s])),
        }
    }
    let mut binomes_json = vec![];
    for (id, members) in all_binomes {
        let universe: String = tx.one("SELECT name FROM universes WHERE id = ?1", [members[0].universe_id])?;
        let mut players = vec![];
        for s in &members {
            players.push(json!({ "pseudo": s.pseudo, "character": character(tx, s.character_id)?.map(|c| c.name) }));
        }
        binomes_json.push(json!({ "id": id, "universe": universe, "is_orphan": members[0].is_orphan, "players": players }));
    }

    events.push(Event::on(
        game_channel(game_id),
        "game.ended",
        json!({
            "game_id": game_id,
            "winners": winners.iter().map(|w| json!({ "id": w.player_id, "pseudo": w.pseudo })).collect::<Vec<_>>(),
            "stats": stats.iter().map(score::Stat::event_json).collect::<Vec<_>>(),
            "all_binomes": binomes_json,
        }),
    ));
    Ok(())
}

fn eliminate(tx: &Transaction, game_id: i64, target: &Player, by: &Player, events: &mut Events) -> AppResult<()> {
    let seat = seats(tx, game_id)?
        .into_iter()
        .find(|s| s.player_id == target.id)
        .ok_or_else(|| AppError::rule("Aucun binome trouvé pour ce joueur dans cette partie."))?;
    tx.run("UPDATE binome_player SET is_eliminated = 1, updated_at = ?1 WHERE binome_id = ?2 AND player_id = ?3", (now_db(), seat.binome_id, target.id))?;
    events.push(Event::on(
        game_channel(game_id),
        "player.eliminated",
        json!({ "eliminated_player": brief(target), "eliminated_by": brief(by) }),
    ));
    if let Some(winners) = check_game_over(tx, game_id)? {
        end_game(tx, game_id, &winners, events)?;
    }
    Ok(())
}

// ─── Question / accusation ──────────────────────────────────────────────────

fn store_action(tx: &Transaction, r: &Round, player: i64, kind: &str, content: &str, target: i64) -> AppResult<Action> {
    let now = now_db();
    let id = tx.insert(
        "INSERT INTO actions (round_id, player_id, type, content, is_valid, target_player_id, accusation_correct, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, NULL, ?6, ?6)",
        (r.id, player, kind, content, target, &now),
    )?;
    Ok(action(tx, id)?.expect("action créée"))
}

fn action_played(c: &impl Q, a: &Action, round_number: i64) -> AppResult<Event> {
    let names = pseudos(c, [Some(a.player_id), a.target_player_id].into_iter().flatten())?;
    let full = public_action(a, round_number, &names);
    let pick = |keys: &[&str]| -> Value { keys.iter().map(|k| (k.to_string(), full[*k].clone())).collect::<serde_json::Map<_, _>>().into() };
    let mut data = pick(&["action_id", "round_id", "round_number", "player", "type", "is_valid", "target_player"]);
    let o = data.as_object_mut().expect("objet");
    if a.is_question() {
        o.insert("question".into(), full["question"].clone());
        o.insert("refused_reason".into(), full["refused_reason"].clone());
    } else {
        o.insert("character_name".into(), json!(a.character_name));
        o.insert("accusation_confirmed".into(), Value::Null);
        o.insert("accusation_correct".into(), Value::Null);
    }
    let game_id: i64 = c.one("SELECT game_id FROM rounds WHERE id = ?1", [a.round_id])?;
    Ok(Event::on(game_channel(game_id), "action.played", data))
}

struct Play {
    g: Game,
    r: Round,
    player: Player,
    target: Player,
}

fn begin_play(tx: &Transaction, g: Game, r: Round, player_id: i64, target_id: i64) -> AppResult<Play> {
    let player = player_or_404(tx, player_id)?;
    let target = player_or_404(tx, target_id)?;
    // Relecture du round dans la transaction (équivalent du lockForUpdate)
    let r = round(tx, r.id)?.ok_or(AppError::NotFound)?;
    validate_turn(&g, &r, player.id)?;
    validate_not_played(tx, &r, player.id)?;
    validate_target(tx, &g, &player, &target)?;
    Ok(Play { g, r, player, target })
}

/// POST /games/{game}/rounds/{round}/question
pub fn question(tx: &Transaction, game_id: i64, round_id: i64, input: &Input) -> AppResult<Out> {
    let (g, r, _) = scoped(tx, game_id, round_id, None)?;
    let mut v = V::new(input).messages(&[
        ("player_id.required", "Le joueur est requis."),
        ("player_id.exists", "Ce joueur n'existe pas."),
        ("question.required", "La question est requise."),
        ("question.min", "La question doit faire au moins 5 caractères."),
        ("question.max", "La question ne peut pas dépasser 500 caractères."),
    ]);
    let pid = v.player(tx, "player_id")?;
    let tid = v.player(tx, "target_player_id")?;
    let text = v.req_str("question", Some(5), Some(500));
    v.finish()?;

    // Les mots interdits ne sont volontairement PAS contrôlés : c'est la cible
    // qui juge, et le questionneur ne doit jamais apprendre qu'il a fauté.
    let p = begin_play(tx, g, r, pid.expect("validé"), tid.expect("validé"))?;
    let a = store_action(tx, &p.r, p.player.id, "question", &text.expect("validé"), p.target.id)?;
    let event = action_played(tx, &a, p.r.number)?;
    let round_json = round_model(tx, a.round_id)?;
    let _ = p.g;
    Ok(Out::ok(json!({
        "action": action_model(tx, &a, true, round_json)?,
        "is_valid": true,
        "message": "Question posée avec succès.",
    }))
    .event(event))
}

/// POST /games/{game}/rounds/{round}/accusation — en attente de confirmation par l'accusé.
pub fn accusation(tx: &Transaction, game_id: i64, round_id: i64, input: &Input) -> AppResult<Out> {
    let (g, r, _) = scoped(tx, game_id, round_id, None)?;
    let mut v = V::new(input).messages(&[
        ("player_id.required", "Le joueur est requis."),
        ("player_id.exists", "Ce joueur n'existe pas."),
        ("target_player_id.required", "La cible est requise."),
        ("target_player_id.exists", "Ce joueur cible n'existe pas."),
        ("target_player_id.different", "Tu ne peux pas t'accuser toi-même."),
        ("character_name.required", "Le nom du personnage est requis."),
        ("character_name.max", "Le nom du personnage est trop long."),
    ]);
    let pid = v.player(tx, "player_id")?;
    let tid = v.player(tx, "target_player_id")?;
    if pid.is_some() && pid == tid {
        v.fail("target_player_id", "different", "The target player id field and player id must be different.".into());
    }
    let name = v.req_str("character_name", None, Some(100));
    v.finish()?;
    let name = name.expect("validé");

    let p = begin_play(tx, g, r, pid.expect("validé"), tid.expect("validé"))?;
    let a = store_action(tx, &p.r, p.player.id, "accusation", &name, p.target.id)?;
    tx.run("UPDATE actions SET character_name = ?1 WHERE id = ?2", (&name, a.id))?;
    let a = action(tx, a.id)?.expect("action");
    let event = action_played(tx, &a, p.r.number)?;
    let round_json = round_model(tx, a.round_id)?;
    Ok(Out::ok(json!({
        "action": action_model(tx, &a, true, round_json)?,
        "message": "Accusation envoyée — en attente de confirmation.",
    }))
    .event(event))
}

// ─── Réponse et tirage de carte ─────────────────────────────────────────────

fn asker_target(c: &impl Q, a: &Action) -> AppResult<(Player, Player)> {
    Ok((player_or_404(c, a.player_id)?, player_or_404(c, a.target_player_id.unwrap_or_default())?))
}

/// POST …/actions/{action}/answer
pub fn answer(tx: &Transaction, game_id: i64, round_id: i64, action_id: i64, input: &Input) -> AppResult<Out> {
    let (_, r, a) = scoped(tx, game_id, round_id, Some(action_id))?;
    let a = a.expect("action");
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let value = v.req_str("answer", None, None);
    v.in_list("answer", value.as_ref(), &["yes", "no", "dont_know"]);
    v.finish()?;
    let (pid, value) = (pid.expect("validé"), value.expect("validé"));

    if a.target_player_id != Some(pid) {
        return Err(AppError::rule("Tu n'es pas le joueur ciblé par cette question."));
    }
    if !a.is_question() {
        return Err(AppError::rule("Cette action n'est pas une question."));
    }
    // « Je ne sais pas » : le tour attend que le questionneur tire une carte
    let card_count = if value == "dont_know" {
        character_in_game(tx, r.game_id, pid)?.map_or(0, |c| c.forbidden_words.len() as i64)
    } else {
        0
    };
    let now = now_db();
    let deadline = clock::in_seconds_db(CARD_DRAW_SECONDS);
    let updated = if card_count > 0 {
        tx.run(
            "UPDATE actions SET answer = ?1, card_draw_status = 'pending', card_count = ?2, card_draw_deadline = ?3, updated_at = ?4
             WHERE id = ?5 AND answer IS NULL",
            (&value, card_count, &deadline, &now, a.id),
        )?
    } else {
        tx.run("UPDATE actions SET answer = ?1, updated_at = ?2 WHERE id = ?3 AND answer IS NULL", (&value, &now, a.id))?
    };
    if updated == 0 {
        return Err(AppError::rule("Cette question a déjà reçu une réponse."));
    }
    let a = action(tx, a.id)?.expect("action");
    let (asker, target) = asker_target(tx, &a)?;
    let mut events = vec![Event::on(
        game_channel(r.game_id),
        "answer.given",
        json!({ "action_id": a.id, "round_id": a.round_id, "answer": a.answer, "card_draw": a.public_card_draw(), "answerer": brief(&target), "asker": brief(&asker) }),
    )];
    let round_json = round_model(tx, a.round_id)?;
    let body = json!({ "action": action_model(tx, &a, false, round_json)?, "message": "Réponse enregistrée." });

    let mut out = Out::ok(body);
    if card_count > 0 {
        out.followups.push(Followup::ExpireCardDraw { action_id: a.id, deadline });
    } else {
        let r = round(tx, r.id)?.expect("round");
        advance_round(tx, &r, Some(a.player_id), &mut events)?;
    }
    Ok(out.with(events))
}

fn assert_card_draw_owner(c: &impl Q, a: &Action, player_id: i64) -> AppResult<()> {
    if a.player_id != player_id {
        let asker = player_or_404(c, a.player_id)?;
        return Err(AppError::rule(format!("Ce tirage revient à {}.", asker.pseudo)));
    }
    match a.card_draw_status.as_deref() {
        None => return Err(AppError::rule("Aucun tirage n'est prévu pour cette question.")),
        Some("shared" | "kept" | "expired") => return Err(AppError::rule("Ce tirage est déjà terminé.")),
        _ => {}
    }
    let game_id: i64 = c.one("SELECT game_id FROM rounds WHERE id = ?1", [a.round_id])?;
    if !game_or_404(c, game_id)?.in_progress() {
        return Err(AppError::rule("La partie est terminée."));
    }
    Ok(())
}

fn finish_card_draw(tx: &Transaction, a: &Action, events: &mut Events) -> AppResult<()> {
    let r = round(tx, a.round_id)?.expect("round");
    let asker = player_or_404(tx, a.player_id)?;
    events.push(Event::on(game_channel(r.game_id), "card.draw.finished", json!({ "action_id": a.id, "asker": brief(&asker) })));
    advance_round(tx, &r, Some(a.player_id), events)
}

/// Clôt un tirage dont le délai est dépassé (minuterie, ou l'hôte qui passe le
/// tour). Sans effet si le tirage est déjà clos.
pub fn expire_card_draw(tx: &Transaction, action_id: i64, events: &mut Events) -> AppResult<()> {
    let updated = tx.run(
        "UPDATE actions SET card_draw_status = 'expired', updated_at = ?1 WHERE id = ?2 AND card_draw_status IN ('pending', 'drawn')",
        (now_db(), action_id),
    )?;
    if updated == 0 {
        return Ok(());
    }
    let a = action(tx, action_id)?.expect("action");
    finish_card_draw(tx, &a, events)
}

/// POST …/actions/{action}/draw — le mot n'est renvoyé qu'ici, au seul questionneur.
pub fn draw(tx: &Transaction, game_id: i64, round_id: i64, action_id: i64, input: &Input) -> AppResult<Out> {
    let (g, _, a) = scoped(tx, game_id, round_id, Some(action_id))?;
    let a = a.expect("action");
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let card = v.req_int("card");
    v.min_int("card", card, 0);
    v.finish()?;
    let (pid, card) = (pid.expect("validé"), card.expect("validé"));

    assert_card_draw_owner(tx, &a, pid)?;
    if a.card_draw_status.as_deref() != Some("pending") {
        return Err(AppError::rule("Tu as déjà tiré ta carte."));
    }
    if card < 0 || card >= a.card_count.unwrap_or(0) {
        return Err(AppError::rule("Cette carte n'existe pas."));
    }
    if a.card_draw_deadline.as_deref().and_then(clock::parse).is_some_and(|d| d < clock::now()) {
        // Délai dépassé : le tirage est clos (et validé) puis la demande refusée
        let mut events = vec![];
        expire_card_draw(tx, a.id, &mut events)?;
        return Ok(Out::message("Temps écoulé : la carte est perdue.").status(axum::http::StatusCode::CONFLICT).with(events));
    }
    let target = player_or_404(tx, a.target_player_id.unwrap_or_default())?;
    let words = character_in_game(tx, g.id, target.id)?.map(|c| c.forbidden_words).unwrap_or_default();
    let Some(word) = words.choose(&mut rand::rng()).cloned() else {
        return Err(AppError::rule(format!("{} n'a aucun mot interdit.", target.pseudo)));
    };
    let updated = tx.run(
        "UPDATE actions SET card_draw_status = 'drawn', drawn_word = ?1, updated_at = ?2 WHERE id = ?3 AND card_draw_status = 'pending'",
        (&word, now_db(), a.id),
    )?;
    if updated == 0 {
        return Err(AppError::rule("Tu as déjà tiré ta carte."));
    }
    let asker = player_or_404(tx, pid)?;
    Ok(Out::ok(json!({ "word": word, "reveal_seconds": REVEAL_SECONDS }))
        .event(Event::on(game_channel(g.id), "card.drawn", json!({ "action_id": a.id, "card": card, "asker": brief(&asker) }))))
}

/// POST …/actions/{action}/draw/decision — montrer à tous sauf la cible, ou garder.
pub fn decide(tx: &Transaction, game_id: i64, round_id: i64, action_id: i64, input: &Input) -> AppResult<Out> {
    let (g, _, a) = scoped(tx, game_id, round_id, Some(action_id))?;
    let a = a.expect("action");
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let share = v.req_bool("share");
    v.finish()?;
    let (pid, share) = (pid.expect("validé"), share.expect("validé"));

    assert_card_draw_owner(tx, &a, pid)?;
    if a.card_draw_status.as_deref() == Some("pending") {
        return Err(AppError::rule("Tire d'abord une carte."));
    }
    let updated = tx.run(
        "UPDATE actions SET card_draw_status = ?1, updated_at = ?2 WHERE id = ?3 AND card_draw_status = 'drawn'",
        (if share { "shared" } else { "kept" }, now_db(), a.id),
    )?;
    if updated == 0 {
        return Err(AppError::rule("Ce tirage est déjà terminé."));
    }
    let a = action(tx, a.id)?.expect("action");
    let mut events = vec![];
    if share {
        let target_id = a.target_player_id.unwrap_or_default();
        let mut recipients: Vec<i64> = seats(tx, g.id)?.iter().map(|s| s.player_id).collect();
        let spectators = match g.room_id { Some(room_id) => room_players(tx, room_id)?, None => vec![] };
        for (id, _, _) in spectators {
            if !recipients.contains(&id) {
                recipients.push(id);
            }
        }
        recipients.retain(|id| *id != a.player_id && *id != target_id);
        if !recipients.is_empty() {
            let (asker, target) = asker_target(tx, &a)?;
            events.push(Event::new(
                recipients.iter().map(|id| format!("player.{id}")).collect(),
                "card.word.shared",
                json!({ "action_id": a.id, "word": a.drawn_word, "reveal_seconds": REVEAL_SECONDS, "asker": brief(&asker), "target": brief(&target) }),
            ));
        }
    }
    finish_card_draw(tx, &a, &mut events)?;
    Ok(Out::message(if share { "Mot montré aux autres joueurs." } else { "Mot gardé pour toi." }).with(events))
}

// ─── Confirmation d'accusation ──────────────────────────────────────────────

/// Tranche une accusation (par l'accusé, ou par le serveur s'il est hors ligne).
fn resolve_accusation(tx: &Transaction, a: &Action, correct: bool, events: &mut Events) -> AppResult<Action> {
    let updated = tx.run(
        "UPDATE actions SET accusation_confirmed = ?1, accusation_correct = ?1, updated_at = ?2 WHERE id = ?3 AND accusation_confirmed IS NULL",
        (correct, now_db(), a.id),
    )?;
    if updated == 0 {
        return Err(AppError::rule("Cette accusation a déjà été confirmée."));
    }
    let a = action(tx, a.id)?.expect("action");
    let r = round(tx, a.round_id)?.expect("round");
    let (accuser, accused) = asker_target(tx, &a)?;
    events.push(Event::on(
        game_channel(r.game_id),
        "accusation.confirmed",
        json!({
            "action_id": a.id,
            "round_id": a.round_id,
            "accusation_confirmed": a.accusation_confirmed,
            "accusation_correct": a.accusation_correct,
            "character_name": a.character_name,
            "accuser": brief(&accuser),
            "accused": brief(&accused),
        }),
    ));
    if correct {
        eliminate(tx, r.game_id, &accused, &accuser, events)?;
    }
    let r = round(tx, r.id)?.expect("round");
    advance_round(tx, &r, Some(a.player_id), events)?;
    Ok(a)
}

/// POST …/actions/{action}/confirm
pub fn confirm(tx: &Transaction, game_id: i64, round_id: i64, action_id: i64, input: &Input) -> AppResult<Out> {
    let (_, _, a) = scoped(tx, game_id, round_id, Some(action_id))?;
    let a = a.expect("action");
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let confirmed = v.req_bool("confirmed");
    v.finish()?;
    let (pid, confirmed) = (pid.expect("validé"), confirmed.expect("validé"));

    if a.target_player_id != Some(pid) {
        return Err(AppError::rule("Tu n'es pas le joueur accusé."));
    }
    if a.is_question() {
        return Err(AppError::rule("Cette action n'est pas une accusation."));
    }
    let mut events = vec![];
    let a = resolve_accusation(tx, &a, confirmed, &mut events)?;
    let round_json = round_model(tx, a.round_id)?;
    Ok(Out::ok(json!({
        "action": action_model(tx, &a, false, round_json)?,
        "message": if confirmed { "Accusation confirmée." } else { "Accusation niée." },
    }))
    .with(events))
}

// ─── Joueur déconnecté : l'hôte décide ──────────────────────────────────────

fn assert_host_of_running_game(c: &impl Q, g: &Game, player_id: i64, not_host: &str) -> AppResult<Room> {
    if !g.in_progress() {
        return Err(AppError::rule("La partie n'est pas en cours."));
    }
    if g.voting() {
        return Err(AppError::rule("Les joueurs choisissent encore leurs personnages."));
    }
    let room = game_room(c, g)?;
    match room {
        Some(r) if r.created_by == player_id => Ok(r),
        _ => Err(AppError::rule(not_host)),
    }
}

const NOT_HOST: &str = "Seul l'hôte peut décider pour un joueur déconnecté.";

/// Présence en mémoire : le joueur doit être hors ligne.
fn assert_offline(env: &Env, game_id: i64, p: &Player) -> AppResult<()> {
    if env.hub.is_online(&game_channel(game_id), p.id) {
        return Err(AppError::rule(format!("{} est de nouveau connecté : laisse-lui le temps de jouer.", p.pseudo)));
    }
    Ok(())
}

/// Question ou accusation du joueur courant qui attend encore la cible.
fn pending_action_of(c: &impl Q, r: &Round) -> AppResult<Option<Action>> {
    let last = round_actions(c, r.id)?.into_iter().filter(|a| a.player_id == r.current_player_id).max_by_key(|a| a.id);
    Ok(last.filter(|a| if a.is_question() { a.is_valid && a.answer.is_none() } else { a.accusation_confirmed.is_none() }))
}

/// Qui la partie attend-elle ? (joueur, raison, action en attente)
fn find_blocker(c: &impl Q, r: &Round) -> AppResult<(Player, &'static str, Option<Action>)> {
    let pending = pending_action_of(c, r)?;
    if pending.is_none() {
        let draw = round_actions(c, r.id)?
            .into_iter()
            .filter(|a| a.player_id == r.current_player_id && matches!(a.card_draw_status.as_deref(), Some("pending" | "drawn")))
            .max_by_key(|a| a.id);
        if let Some(d) = draw {
            return Ok((player_or_404(c, d.player_id)?, "draw", Some(d)));
        }
        if c.exists("SELECT 1 FROM actions WHERE round_id = ?1 AND player_id = ?2", [r.id, r.current_player_id])? {
            return Err(AppError::rule("Personne ne bloque la partie pour l'instant."));
        }
        return Ok((player_or_404(c, r.current_player_id)?, "turn", None));
    }
    let p = pending.expect("action en attente");
    let target = player_or_404(c, p.target_player_id.unwrap_or_default())?;
    let reason = if p.is_question() { "answer" } else { "accusation" };
    Ok((target, reason, Some(p)))
}

/// POST /games/{game}/skip-turn — hôte
pub fn skip_turn(tx: &Transaction, env: &Env, game_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(tx, game_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    v.finish()?;
    let host = player_or_404(tx, pid.expect("validé"))?;
    assert_host_of_running_game(tx, &g, host.id, NOT_HOST)?;
    let r = current_round(tx, g.id)?.ok_or_else(|| AppError::rule("Aucun tour en cours."))?;
    let (blocker, reason, pending) = find_blocker(tx, &r)?;
    if reason == "answer" {
        return Err(AppError::rule(format!(
            "{} doit répondre à la question : attends son retour, ou exclus-le de la partie.",
            blocker.pseudo
        )));
    }
    if blocker.id == host.id {
        return Err(AppError::rule("C'est à toi de jouer : tu ne peux pas passer ton propre tour."));
    }
    assert_offline(env, g.id, &blocker)?;

    let mut events = vec![Event::on(
        game_channel(g.id),
        "turn.skipped",
        json!({ "reason": reason, "action_id": pending.as_ref().map(|a| a.id), "player": brief(&blocker), "skipped_by": brief(&host) }),
    )];
    match (reason, pending) {
        ("accusation", Some(a)) => {
            // Le serveur tranche : nom proposé == vrai personnage (casse, accents, espaces ignorés)
            let truth = character_in_game(tx, g.id, a.target_player_id.unwrap_or_default())?;
            let proposed = a.character_name.clone().or(a.content.clone()).unwrap_or_default();
            let correct = truth.is_some_and(|c| squish_ascii(&c.name) == squish_ascii(&proposed));
            resolve_accusation(tx, &a, correct, &mut events)?;
        }
        ("draw", Some(a)) => expire_card_draw(tx, a.id, &mut events)?,
        _ => advance_round(tx, &r, None, &mut events)?,
    }
    Ok(Out::ok(json!({ "message": format!("Tour de {} passé.", blocker.pseudo), "reason": reason })).with(events))
}

/// POST /games/{game}/actions/{action}/correct-answer — hôte, oui ⇄ non
pub fn correct_answer(tx: &Transaction, game_id: i64, action_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(tx, game_id)?;
    let a = action(tx, action_id)?.ok_or(AppError::NotFound)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let value = v.req_str("answer", None, None);
    v.in_list("answer", value.as_ref(), &["yes", "no"]);
    v.finish()?;
    let (host, value) = (player_or_404(tx, pid.expect("validé"))?, value.expect("validé"));

    assert_host_of_running_game(tx, &g, host.id, "Seul l'hôte peut corriger une réponse.")?;
    if round(tx, a.round_id)?.map(|r| r.game_id) != Some(g.id) {
        return Err(AppError::rule("Cette question n'appartient pas à la partie."));
    }
    if !a.is_question() || !a.is_valid {
        return Err(AppError::rule("Seule une question valide peut être corrigée."));
    }
    let previous = a.answer.clone().unwrap_or_default();
    if !matches!(previous.as_str(), "yes" | "no") {
        return Err(AppError::rule("Seules les réponses oui / non peuvent être corrigées."));
    }
    let respond = |a: &Action| json!({ "action_id": a.id, "answer": a.answer, "answer_corrected_from": a.answer_corrected_from, "message": "Réponse corrigée." });
    if previous == value {
        return Ok(Out::ok(respond(&a)));
    }
    // On garde la réponse d'ORIGINE ; y revenir efface la trace.
    let original = a.answer_corrected_from.clone().unwrap_or_else(|| previous.clone());
    let now = now_db();
    if original == value {
        tx.run("UPDATE actions SET answer = ?1, answer_corrected_from = NULL, answer_corrected_at = NULL, updated_at = ?2 WHERE id = ?3", (&value, &now, a.id))?;
    } else {
        tx.run("UPDATE actions SET answer = ?1, answer_corrected_from = ?2, answer_corrected_at = ?3, updated_at = ?3 WHERE id = ?4", (&value, &original, &now, a.id))?;
    }
    let a = action(tx, a.id)?.expect("action");
    let answerer = player_or_404(tx, a.target_player_id.unwrap_or_default())?;
    let event = Event::on(
        game_channel(g.id),
        "answer.corrected",
        json!({
            "action_id": a.id,
            "round_id": a.round_id,
            "answer": a.answer,
            "previous_answer": previous,
            "answer_corrected_from": a.answer_corrected_from,
            "question": a.content,
            "answerer": brief(&answerer),
            "host": brief(&host),
        }),
    );
    Ok(Out::ok(respond(&a)).event(event))
}

/// POST /games/{game}/players/{player}/exclude — hôte : l'exclu et son binôme sont éliminés.
pub fn exclude(tx: &Transaction, env: &Env, game_id: i64, target_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(tx, game_id)?;
    let target = player_or_404(tx, target_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    v.finish()?;
    let host = player_or_404(tx, pid.expect("validé"))?;

    assert_host_of_running_game(tx, &g, host.id, NOT_HOST)?;
    if target.id == host.id {
        return Err(AppError::rule("Tu ne peux pas t'exclure toi-même."));
    }
    let seated = seats(tx, g.id)?;
    let Some(seat) = seated.iter().find(|s| s.player_id == target.id) else {
        return Err(AppError::rule(format!("{} ne joue pas dans cette partie.", target.pseudo)));
    };
    if seat.is_eliminated {
        return Err(AppError::rule(format!("{} est déjà éliminé.", target.pseudo)));
    }
    assert_offline(env, g.id, &target)?;
    let partner = seated.iter().find(|s| s.binome_id == seat.binome_id && s.player_id != target.id && !s.is_eliminated);

    let r = current_round(tx, g.id)?;
    let round_number = match &r {
        Some(r) => r.number,
        None => tx.one("SELECT COALESCE(MAX(number), 0) FROM rounds WHERE game_id = ?1", [g.id])?,
    };
    let now = now_db();
    tx.run(
        "UPDATE binome_player SET is_eliminated = 1, is_excluded = 1, eliminated_round = ?1, updated_at = ?2 WHERE binome_id = ?3 AND player_id = ?4",
        (round_number, &now, seat.binome_id, target.id),
    )?;
    if let Some(p) = partner {
        tx.run(
            "UPDATE binome_player SET is_eliminated = 1, eliminated_round = ?1, updated_at = ?2 WHERE binome_id = ?3 AND player_id = ?4",
            (round_number, &now, p.binome_id, p.player_id),
        )?;
    }

    // Question / accusation en attente impliquant un éliminé : supprimée
    let removed: Vec<i64> = std::iter::once(target.id).chain(partner.map(|p| p.player_id)).collect();
    let (mut cancelled, mut advance) = (None, false);
    if let Some(r) = &r {
        let pending = pending_action_of(tx, r)?;
        if removed.contains(&r.current_player_id) {
            cancelled = pending;
            advance = true;
        } else if pending.as_ref().is_some_and(|p| p.target_player_id.is_some_and(|t| removed.contains(&t))) {
            cancelled = pending;
        }
    }
    if let Some(a) = &cancelled {
        tx.run("DELETE FROM actions WHERE id = ?1", [a.id])?;
    }

    let partner_player = partner.map(|p| Player { id: p.player_id, pseudo: p.pseudo.clone() });
    let mut events = vec![Event::on(
        game_channel(g.id),
        "player.excluded",
        json!({
            "player": brief(&target),
            "partner": partner_player.as_ref().map(brief),
            "excluded_by": brief(&host),
            "cancelled_action_id": cancelled.as_ref().map(|a| a.id),
        }),
    )];
    if let Some(winners) = check_game_over(tx, g.id)? {
        end_game(tx, g.id, &winners, &mut events)?;
    } else if advance {
        if let Some(r) = &r {
            let fresh = round(tx, r.id)?.expect("round");
            advance_round(tx, &fresh, Some(fresh.current_player_id), &mut events)?;
        }
    }
    let msg = match &partner_player {
        Some(p) => format!("{} est exclu, son binôme {} est éliminé.", target.pseudo, p.pseudo),
        None => format!("{} est exclu.", target.pseudo),
    };
    Ok(Out::message(&msg).with(events))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn seat(binome: i64, player: i64, eliminated: bool) -> Seat {
        Seat {
            binome_id: binome,
            is_orphan: false,
            universe_id: 1,
            player_id: player,
            pseudo: format!("p{player}"),
            character_id: player,
            character_vote: None,
            score: 0,
            is_eliminated: eliminated,
            is_excluded: false,
            eliminated_round: None,
        }
    }

    fn over(seated: Vec<Seat>) -> Option<Vec<i64>> {
        winners(seated).map(|w| w.iter().map(|s| s.player_id).collect())
    }

    #[test]
    fn game_over_rules() {
        // Deux paires intactes : la partie continue
        assert_eq!(over(vec![seat(1, 1, false), seat(1, 2, false), seat(2, 3, false), seat(2, 4, false)]), None);
        // Une paire intacte, l'autre tombée : la paire gagne
        assert_eq!(over(vec![seat(1, 1, false), seat(1, 2, false), seat(2, 3, true), seat(2, 4, true)]), Some(vec![1, 2]));
        // Une paire intacte + un rescapé : continue
        assert_eq!(over(vec![seat(1, 1, false), seat(1, 2, false), seat(2, 3, false), seat(2, 4, true)]), None);
        // Orphelin en vie seul face à des rescapés éliminés : il gagne
        assert_eq!(over(vec![seat(1, 1, true), seat(1, 2, true), seat(3, 5, false)]), Some(vec![5]));
        // Aucune équipe intacte, un seul rescapé : il gagne seul
        assert_eq!(over(vec![seat(1, 1, false), seat(1, 2, true), seat(2, 3, true), seat(2, 4, true)]), Some(vec![1]));
    }
}
