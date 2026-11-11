//! Scores de fin de partie (ScoreService), vote « meilleure question »
//! (QuestionVoteService) et récapitulatif (GameRecapService).

use std::collections::HashMap;

use rusqlite::Transaction;
use serde_json::{json, Value};

use crate::clock::{self, now_db};
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Input, V};
use crate::realtime::hub::Event;
use crate::services::common::*;
use crate::services::game::game_channel;
use crate::services::Out;
use crate::support::forbidden;
use crate::support::text::ci_key;

/// Bonus de victoire : équipe intacte (binôme complet ou orphelin) / rescapé seul.
pub const WIN_BONUS_INTACT: i64 = 5;
pub const WIN_BONUS_SURVIVOR: i64 = 3;

#[derive(Debug, Clone)]
pub struct Stat {
    pub player_pseudo: String,
    pub character_name: String,
    pub eliminations: i64,
    pub best_questions: i64,
    pub rounds_survived: i64,
    pub survived_full_game: bool,
    pub score: i64,
    pub is_winner: bool,
    pub is_eliminated: bool,
    pub is_excluded: bool,
}

impl Stat {
    pub fn event_json(&self) -> Value {
        json!({
            "player_pseudo": self.player_pseudo,
            "character_name": self.character_name,
            "eliminations": self.eliminations,
            "best_questions": self.best_questions,
            "rounds_survived": self.rounds_survived,
            "survived_full_game": self.survived_full_game,
            "score": self.score,
            "is_winner": self.is_winner,
            "is_eliminated": self.is_eliminated,
            "is_excluded": self.is_excluded,
        })
    }
}

/// Calcule et enregistre les stats de fin de partie.
pub fn compute_and_store(tx: &Transaction, game_id: i64, winner_ids: &[i64]) -> AppResult<Vec<Stat>> {
    let seated = seats(tx, game_id)?;
    let all_rounds = rounds(tx, game_id)?;
    let actions = game_actions(tx, game_id)?;
    let best = best_question_counts(tx, game_id)?;
    let now = now_db();
    let mut stats = vec![];

    for s in &seated {
        let team_intact = seated.iter().filter(|o| o.binome_id == s.binome_id).all(|o| !o.is_eliminated);
        let is_winner = winner_ids.contains(&s.player_id);
        let character_name = character(tx, s.character_id)?.map(|c| c.name).unwrap_or_else(|| "?".into());
        let correct = |a: &&Action| a.kind == "accusation" && a.accusation_correct == Some(true);
        let eliminations = actions.iter().filter(correct).filter(|a| a.player_id == s.player_id).count() as i64;

        // Rounds survécus : jusqu'à l'exclusion, jusqu'au round de l'élimination (inclus), sinon tous
        let rounds_survived = match s.eliminated_round {
            Some(n) => n,
            None => match actions.iter().filter(correct).find(|a| a.target_player_id == Some(s.player_id)) {
                Some(a) => all_rounds.iter().find(|r| r.id == a.round_id).map_or(0, |r| r.number),
                None => all_rounds.len() as i64,
            },
        };
        let win_bonus = if is_winner && !s.is_eliminated {
            if team_intact { WIN_BONUS_INTACT } else { WIN_BONUS_SURVIVOR }
        } else {
            0
        };
        let best_questions = best.get(&s.player_id).copied().unwrap_or(0);
        // Un joueur exclu par l'hôte marque 0
        let score = if s.is_excluded { 0 } else { eliminations + rounds_survived + best_questions + win_bonus };

        let stat = Stat {
            player_pseudo: s.pseudo.clone(),
            character_name,
            eliminations,
            best_questions,
            rounds_survived,
            survived_full_game: is_winner && !s.is_eliminated,
            score,
            is_winner,
            is_eliminated: s.is_eliminated,
            is_excluded: s.is_excluded,
        };
        tx.run(
            "INSERT INTO game_stats (game_id, player_pseudo, character_name, eliminations, best_questions, rounds_survived,
                 survived_full_game, score, is_winner, is_eliminated, is_excluded, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
            rusqlite::params![
                game_id, stat.player_pseudo, stat.character_name, stat.eliminations, stat.best_questions, stat.rounds_survived,
                stat.survived_full_game, stat.score, stat.is_winner, stat.is_eliminated, stat.is_excluded, now
            ],
        )?;
        stats.push(stat);
    }
    Ok(stats)
}

// ─── Meilleure question du round ────────────────────────────────────────────

pub struct Win {
    pub round: i64,
    pub action: Action,
    pub votes: i64,
}

/// Question(s) la/les plus votée(s) de chaque round (ex æquo compris).
pub fn winning_questions(c: &impl Q, game_id: i64) -> AppResult<Vec<Win>> {
    let votes = c.all(
        "SELECT qv.round_id, qv.action_id, r.number FROM question_votes qv JOIN rounds r ON r.id = qv.round_id
         WHERE r.game_id = ?1 ORDER BY qv.id",
        [game_id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
    )?;
    // Regroupement par round puis par action, dans l'ordre d'apparition
    let mut rounds_seen: Vec<(i64, i64, Vec<(i64, i64)>)> = vec![];
    for (round_id, action_id, number) in votes {
        let entry = match rounds_seen.iter().position(|r| r.0 == round_id) {
            Some(i) => &mut rounds_seen[i],
            None => {
                rounds_seen.push((round_id, number, vec![]));
                rounds_seen.last_mut().expect("ajouté")
            }
        };
        match entry.2.iter_mut().find(|(a, _)| *a == action_id) {
            Some((_, n)) => *n += 1,
            None => entry.2.push((action_id, 1)),
        }
    }
    let mut wins = vec![];
    for (_, number, counts) in rounds_seen {
        let best = counts.iter().map(|(_, n)| *n).max().unwrap_or(0);
        for (action_id, n) in counts.into_iter().filter(|(_, n)| *n == best) {
            if let Some(a) = action(c, action_id)? {
                wins.push(Win { round: number, action: a, votes: n });
            }
        }
    }
    wins.sort_by_key(|w| w.round);
    Ok(wins)
}

/// player_id → rounds remportés au vote.
pub fn best_question_counts(c: &impl Q, game_id: i64) -> AppResult<HashMap<i64, i64>> {
    let mut out = HashMap::new();
    for w in winning_questions(c, game_id)? {
        *out.entry(w.action.player_id).or_insert(0) += 1;
    }
    Ok(out)
}

/// POST /games/{game}/question-votes — un éliminé vote (et peut changer d'avis).
pub fn vote_question(tx: &Transaction, game_id: i64, input: &Input) -> AppResult<Out> {
    let g = game_or_404(tx, game_id)?;
    let mut v = V::new(input);
    let pid = v.player(tx, "player_id")?;
    let aid = v.req_int("action_id");
    let aid = v.exists(tx, "action_id", aid, "SELECT 1 FROM actions WHERE id = ?1")?;
    v.finish()?;
    let (voter, a) = (pid.expect("validé"), action(tx, aid.expect("validé"))?.expect("action"));

    if !game_room(tx, &g)?.is_some_and(|r| r.best_question_vote) {
        return Err(AppError::rule("Le vote des éliminés est désactivé pour cette partie."));
    }
    if !g.in_progress() || g.voting() {
        return Err(AppError::rule("La partie n'est pas en cours."));
    }
    let seated = seats(tx, g.id)?;
    let me = seated.iter().find(|s| s.player_id == voter).ok_or_else(|| AppError::rule("Tu ne joues pas dans cette partie."))?;
    if !me.is_eliminated {
        return Err(AppError::rule("Seuls les joueurs éliminés votent."));
    }
    let r = round(tx, a.round_id)?.expect("round");
    if r.game_id != g.id {
        return Err(AppError::rule("Cette question n'appartient pas à cette partie."));
    }
    if !a.is_question() || !a.is_valid {
        return Err(AppError::rule("On ne vote que pour une question."));
    }
    if a.player_id == voter {
        return Err(AppError::rule("Tu ne peux pas voter pour ta propre question."));
    }
    if seated.iter().any(|s| s.binome_id == me.binome_id && s.player_id == a.player_id) {
        return Err(AppError::rule("Tu ne peux pas voter pour ton binôme."));
    }
    let max: i64 = tx.one("SELECT COALESCE(MAX(number), 0) FROM rounds WHERE game_id = ?1", [g.id])?;
    if r.number < max - 1 {
        return Err(AppError::rule("Le vote est clos pour ce round."));
    }
    let now = now_db();
    let changed = tx.run("UPDATE question_votes SET action_id = ?1, updated_at = ?2 WHERE round_id = ?3 AND voter_id = ?4", (a.id, &now, r.id, voter))?;
    if changed == 0 {
        tx.run(
            "INSERT INTO question_votes (round_id, action_id, voter_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            (r.id, a.id, voter, &now),
        )?;
    }
    let votes = tx.all("SELECT action_id, voter_id FROM question_votes WHERE round_id = ?1 ORDER BY id", [r.id], |row| {
        Ok(json!({ "action_id": row.get::<_, i64>(0)?, "voter_id": row.get::<_, i64>(1)? }))
    })?;
    Ok(Out::message("Vote enregistré.").event(Event::on(game_channel(g.id), "question.voted", json!({ "round_id": r.id, "votes": votes }))))
}

// ─── Récap ──────────────────────────────────────────────────────────────────

pub fn game_stats(c: &impl Q, game_id: i64) -> AppResult<Vec<Stat>> {
    c.all(
        "SELECT player_pseudo, character_name, eliminations, best_questions, rounds_survived, survived_full_game, score,
                is_winner, is_eliminated, is_excluded FROM game_stats WHERE game_id = ?1 ORDER BY id",
        [game_id],
        |r| {
            Ok(Stat {
                player_pseudo: r.get(0)?,
                character_name: r.get(1)?,
                eliminations: r.get(2)?,
                best_questions: r.get(3)?,
                rounds_survived: r.get(4)?,
                survived_full_game: r.get(5)?,
                score: r.get(6)?,
                is_winner: r.get(7)?,
                is_eliminated: r.get(8)?,
                is_excluded: r.get(9)?,
            })
        },
    )
}

/// Tri décroissant par score, stable (sortByDesc).
pub fn by_score_desc(mut stats: Vec<Stat>) -> Vec<Stat> {
    stats.sort_by(|a, b| b.score.cmp(&a.score));
    stats
}

fn plural(n: i64, word: &str) -> String {
    if n > 1 { format!("{word}s") } else { word.to_string() }
}

/// Trophée du/des joueur(s) le(s) plus représenté(s) ; aucun au-delà de 2 ex æquo.
fn award(key: &str, emoji: &str, title: &str, ids: &[i64], names: &HashMap<i64, String>, detail: impl Fn(i64) -> String) -> Option<Value> {
    let mut counts: Vec<(i64, i64)> = vec![];
    for id in ids {
        match counts.iter_mut().find(|(p, _)| p == id) {
            Some((_, n)) => *n += 1,
            None => counts.push((*id, 1)),
        }
    }
    let best = counts.iter().map(|(_, n)| *n).max()?;
    let pseudos: Vec<&String> = counts.iter().filter(|(_, n)| *n == best).filter_map(|(id, _)| names.get(id)).collect();
    if pseudos.len() > 2 {
        return None;
    }
    Some(json!({ "key": key, "emoji": emoji, "title": title, "pseudos": pseudos, "detail": detail(best) }))
}

/// GET /games/{game}/recap — uniquement une partie terminée (tout y est révélé).
pub fn recap(c: &impl Q, env: &Env, game_id: i64) -> AppResult<Out> {
    let g = game_or_404(c, game_id)?;
    if !g.finished() {
        return Err(AppError::rule("La partie n'est pas terminée."));
    }
    let seated = seats(c, g.id)?;
    let all_rounds = rounds(c, g.id)?;
    let number_of = |round_id: i64| all_rounds.iter().find(|r| r.id == round_id).map_or(0, |r| r.number);
    let mut actions = game_actions(c, g.id)?;
    actions.sort_by_key(|a| (number_of(a.round_id), a.id));
    let names = pseudos(c, actions.iter().flat_map(|a| [Some(a.player_id), a.target_player_id]).flatten())?;

    let mut characters: HashMap<i64, Character> = HashMap::new();
    for s in &seated {
        if let Some(ch) = character(c, s.character_id)? {
            characters.insert(s.character_id, ch);
        }
    }
    let words_of = |player: Option<i64>| -> Vec<String> {
        seated.iter().find(|s| Some(s.player_id) == player).and_then(|s| characters.get(&s.character_id)).map(|c| c.forbidden_words.clone()).unwrap_or_default()
    };

    let questions: Vec<&Action> = actions.iter().filter(|a| a.is_question() && a.is_valid).collect();
    let accusations: Vec<&Action> = actions.iter().filter(|a| a.kind == "accusation").collect();
    let trapped: Vec<&&Action> = questions.iter().filter(|a| forbidden::matches_any(a.content.as_deref().unwrap_or(""), &words_of(a.target_player_id))).collect();
    let correct: Vec<&&Action> = accusations.iter().filter(|a| a.accusation_correct == Some(true)).collect();
    let wrong: Vec<&&Action> = accusations.iter().filter(|a| a.accusation_correct == Some(false)).collect();

    let last = actions.iter().filter_map(|a| a.updated_at.clone()).max().or(g.updated_at.clone());
    let duration = match (g.created_at.as_deref().and_then(clock::parse), last.as_deref().and_then(clock::parse)) {
        (Some(a), Some(b)) => json!((b - a).whole_seconds().abs()),
        _ => Value::Null,
    };
    let avatars = avatar_urls(c, env, seated.iter().map(|s| s.pseudo.as_str()))?;
    let stats = game_stats(c, g.id)?;

    let mut binomes: Vec<(i64, Vec<&Seat>)> = vec![];
    for s in &seated {
        match binomes.iter_mut().find(|(id, _)| *id == s.binome_id) {
            Some((_, l)) => l.push(s),
            None => binomes.push((s.binome_id, vec![s])),
        }
    }
    let mut binomes_json = vec![];
    let mut exclusions = vec![];
    for (id, members) in &binomes {
        let (universe, cosmos) = crate::services::game::universe_names(c, members[0].universe_id)?;
        binomes_json.push(json!({
            "id": id,
            "universe": universe,
            "cosmos": cosmos,
            "is_orphan": members[0].is_orphan,
            "players": members.iter().map(|s| {
                let ch = characters.get(&s.character_id);
                json!({
                    "pseudo": s.pseudo,
                    "character": ch.map(|c| c.name.clone()),
                    "character_image": ch.map_or(Value::Null, |c| character_image_url(env, c.id, c.has_image, c.updated_at.as_deref())),
                    "avatar_url": avatars.get(&ci_key(&s.pseudo)),
                    "is_eliminated": s.is_eliminated,
                })
            }).collect::<Vec<_>>(),
        }));
        for s in members.iter().filter(|s| s.is_excluded) {
            let partner = members.iter().find(|p| p.player_id != s.player_id && !p.is_excluded && p.eliminated_round.is_some());
            exclusions.push(json!({ "round": s.eliminated_round, "pseudo": s.pseudo, "partner": partner.map(|p| p.pseudo.clone()) }));
        }
    }

    let by = |list: &[&&Action], target: bool| -> Vec<i64> {
        list.iter().filter_map(|a| if target { a.target_player_id } else { Some(a.player_id) }).collect()
    };
    let q_refs: Vec<&&Action> = questions.iter().collect();
    let acc_refs: Vec<&&Action> = accusations.iter().collect();
    let awards: Vec<Value> = [
        award("detective", "🕵️", "Fin limier", &by(&correct, false), &names, |n| format!("{n} {} {}", plural(n, "accusation"), plural(n, "juste"))),
        award("trigger_happy", "🎲", "Tir à côté", &by(&wrong, false), &names, |n| format!("{n} {} {}", plural(n, "accusation"), plural(n, "ratée"))),
        award("suspect", "🎯", "Le plus soupçonné", &by(&acc_refs, true), &names, |n| format!("accusé {n} fois")),
        award("curious", "🔎", "Le plus curieux", &by(&q_refs, false), &names, |n| format!("{n} {} {}", plural(n, "question"), plural(n, "posée"))),
        award("grilled", "🔥", "Sur le grill", &by(&q_refs, true), &names, |n| format!("{n} {} {}", plural(n, "question"), plural(n, "reçue"))),
        award("clumsy", "💣", "Maladroit", &by(&trapped, false), &names, |n| format!("a prononcé {n} {} {}", plural(n, "mot"), plural(n, "interdit"))),
        award("liar", "🤥", "Permis de mentir", &by(&trapped, true), &names, |n| format!("{n} fois autorisé à mentir")),
    ]
    .into_iter()
    .flatten()
    .collect();

    let best_questions: Vec<Value> = winning_questions(c, g.id)?
        .into_iter()
        .map(|w| json!({ "round": w.round, "author": names.get(&w.action.player_id).cloned().or_else(|| player(c, w.action.player_id).ok().flatten().map(|p| p.pseudo)), "question": w.action.content, "votes": w.votes }))
        .collect();

    Ok(Out::ok(json!({
        "game_id": g.id,
        "duration_seconds": duration,
        "rounds_count": all_rounds.len(),
        "questions_count": questions.len(),
        "accusations_count": accusations.len(),
        "winners": stats.iter().filter(|s| s.is_winner).map(|s| s.player_pseudo.clone()).collect::<Vec<_>>(),
        "stats": by_score_desc(stats).iter().map(Stat::event_json).collect::<Vec<_>>(),
        "binomes": binomes_json,
        "exclusions": exclusions,
        "best_questions": best_questions,
        "eliminations": correct.iter().map(|a| json!({
            "round": number_of(a.round_id),
            "by": names.get(&a.player_id),
            "target": a.target_player_id.and_then(|t| names.get(&t)),
            "character": a.character_name.as_ref().or(a.content.as_ref()),
        })).collect::<Vec<_>>(),
        "awards": awards,
    })))
}
