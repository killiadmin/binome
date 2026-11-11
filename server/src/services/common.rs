//! Types de lignes, chargements et charges utiles partagés par les services.

use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::Row;
use serde_json::{json, Value};

use crate::clock;
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::realtime::hub::Hub;
use crate::support::text::ci_key;

/// Contexte passé aux services exécutés dans une transaction.
#[derive(Clone)]
pub struct Env {
    pub prefix: String,
    pub hub: Arc<Hub>,
}

// ─── Joueurs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Player {
    pub id: i64,
    pub pseudo: String,
}

pub fn player(c: &impl Q, id: i64) -> AppResult<Option<Player>> {
    c.opt("SELECT id, pseudo FROM players WHERE id = ?1", [id], |r| Ok(Player { id: r.get(0)?, pseudo: r.get(1)? }))
}

pub fn player_or_404(c: &impl Q, id: i64) -> AppResult<Player> {
    player(c, id)?.ok_or(AppError::NotFound)
}

pub fn brief(p: &Player) -> Value {
    json!({ "id": p.id, "pseudo": p.pseudo })
}

// ─── Salons ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Room {
    pub id: i64,
    pub code: String,
    pub is_locked: bool,
    pub max_players: i64,
    pub game_mode: String,
    pub cosmos_id: Option<i64>,
    pub reveal_to_eliminated: bool,
    pub best_question_vote: bool,
    pub created_by: i64,
}

const ROOM_COLS: &str = "id, code, is_locked, max_players, game_mode, cosmos_id, reveal_to_eliminated, best_question_vote, created_by";

fn room_row(r: &Row) -> rusqlite::Result<Room> {
    Ok(Room {
        id: r.get(0)?,
        code: r.get(1)?,
        is_locked: r.get(2)?,
        max_players: r.get(3)?,
        game_mode: r.get(4)?,
        cosmos_id: r.get(5)?,
        reveal_to_eliminated: r.get(6)?,
        best_question_vote: r.get(7)?,
        created_by: r.get(8)?,
    })
}

pub fn room(c: &impl Q, id: i64) -> AppResult<Option<Room>> {
    c.opt(&format!("SELECT {ROOM_COLS} FROM rooms WHERE id = ?1"), [id], room_row)
}

pub fn room_or_404(c: &impl Q, id: i64) -> AppResult<Room> {
    room(c, id)?.ok_or(AppError::NotFound)
}

pub fn room_by_code(c: &impl Q, code: &str) -> AppResult<Option<Room>> {
    c.opt(&format!("SELECT {ROOM_COLS} FROM rooms WHERE code = ?1"), [code], room_row)
}

pub fn cosmos_name(c: &impl Q, id: Option<i64>) -> AppResult<Option<String>> {
    match id {
        Some(id) => c.opt("SELECT name FROM cosmos WHERE id = ?1", [id], |r| r.get(0)),
        None => Ok(None),
    }
}

/// Partie en cours du salon (`Room::currentGame`).
pub fn current_game_id(c: &impl Q, room_id: i64) -> AppResult<Option<i64>> {
    c.opt("SELECT id FROM games WHERE room_id = ?1 AND status = 'in_progress' ORDER BY id LIMIT 1", [room_id], |r| r.get(0))
}

/// Joueurs du salon dans l'ordre d'arrivée : (id, pseudo, is_ready).
pub fn room_players(c: &impl Q, room_id: i64) -> AppResult<Vec<(i64, String, bool)>> {
    c.all(
        "SELECT p.id, p.pseudo, rp.is_ready FROM room_player rp JOIN players p ON p.id = rp.player_id
         WHERE rp.room_id = ?1 ORDER BY rp.id",
        [room_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
}

/// Liste du lobby (`Room::playersPayload`). `is_ready` reste en 0 / 1 (contrat).
pub fn players_payload(c: &impl Q, env: &Env, room_id: i64) -> AppResult<Value> {
    let players = room_players(c, room_id)?;
    let avatars = avatar_urls(c, env, players.iter().map(|p| p.1.as_str()))?;
    Ok(Value::Array(
        players
            .iter()
            .map(|(id, pseudo, ready)| {
                json!({ "id": id, "pseudo": pseudo, "is_ready": i64::from(*ready), "avatar_url": avatars.get(&ci_key(pseudo)) })
            })
            .collect(),
    ))
}

// ─── Images ─────────────────────────────────────────────────────────────────

/// URLs des photos par pseudo (clé : `ci_key(pseudo)`), en une requête.
pub fn avatar_urls<'a>(c: &impl Q, env: &Env, pseudos: impl Iterator<Item = &'a str>) -> AppResult<HashMap<String, String>> {
    let wanted: std::collections::HashSet<String> = pseudos.map(ci_key).collect();
    if wanted.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = c.all("SELECT id, pseudo, updated_at FROM player_avatars", [], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
    })?;
    Ok(rows
        .into_iter()
        .filter(|(_, p, _)| wanted.contains(&ci_key(p)))
        .map(|(id, p, at)| (ci_key(&p), avatar_url(env, id, at.as_deref())))
        .collect())
}

pub fn avatar_url(env: &Env, id: i64, updated_at: Option<&str>) -> String {
    format!("{}/api/avatars/{id}?v={}", env.prefix, clock::unix(updated_at))
}

pub fn character_image_url(env: &Env, id: i64, has_image: bool, updated_at: Option<&str>) -> Value {
    if has_image {
        json!(format!("{}/api/character-images/{id}?v={}", env.prefix, clock::unix(updated_at)))
    } else {
        Value::Null
    }
}

pub fn reaction_url(env: &Env, id: i64, is_unicode: bool, updated_at: Option<&str>) -> Value {
    if is_unicode {
        Value::Null
    } else {
        json!(format!("{}/api/reactions/{id}/image?v={}", env.prefix, clock::unix(updated_at)))
    }
}

// ─── Parties ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Game {
    pub id: i64,
    pub room_id: i64,
    pub status: String,
    pub characters_confirmed_at: Option<String>,
    pub draw_number: i64,
    pub rejected_character_ids: Vec<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Game {
    pub fn in_progress(&self) -> bool {
        self.status == "in_progress"
    }
    pub fn finished(&self) -> bool {
        self.status == "finished"
    }
    /// Vote des personnages en cours : aucun round tant qu'il dure.
    pub fn voting(&self) -> bool {
        self.in_progress() && self.characters_confirmed_at.is_none()
    }
}

pub fn game(c: &impl Q, id: i64) -> AppResult<Option<Game>> {
    c.opt(
        "SELECT id, room_id, status, characters_confirmed_at, draw_number, rejected_character_ids, created_at, updated_at FROM games WHERE id = ?1",
        [id],
        |r| {
            let rejected: Option<String> = r.get(5)?;
            Ok(Game {
                id: r.get(0)?,
                room_id: r.get(1)?,
                status: r.get(2)?,
                characters_confirmed_at: r.get(3)?,
                draw_number: r.get(4)?,
                rejected_character_ids: rejected.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default(),
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        },
    )
}

pub fn game_or_404(c: &impl Q, id: i64) -> AppResult<Game> {
    game(c, id)?.ok_or(AppError::NotFound)
}

/// Un joueur dans sa partie (ligne `binome_player` + binôme + pseudo).
#[derive(Debug, Clone)]
pub struct Seat {
    pub binome_id: i64,
    pub is_orphan: bool,
    pub universe_id: i64,
    pub player_id: i64,
    pub pseudo: String,
    pub character_id: i64,
    pub character_vote: Option<String>,
    pub score: i64,
    pub is_eliminated: bool,
    pub is_excluded: bool,
    pub eliminated_round: Option<i64>,
}

/// Places de la partie, par binôme puis par ordre d'attribution.
pub fn seats(c: &impl Q, game_id: i64) -> AppResult<Vec<Seat>> {
    c.all(
        "SELECT b.id, b.is_orphan, b.universe_id, p.id, p.pseudo, bp.character_id, bp.character_vote, bp.score,
                bp.is_eliminated, bp.is_excluded, bp.eliminated_round
         FROM binomes b JOIN binome_player bp ON bp.binome_id = b.id JOIN players p ON p.id = bp.player_id
         WHERE b.game_id = ?1 ORDER BY b.id, bp.id",
        [game_id],
        |r| {
            Ok(Seat {
                binome_id: r.get(0)?,
                is_orphan: r.get(1)?,
                universe_id: r.get(2)?,
                player_id: r.get(3)?,
                pseudo: r.get(4)?,
                character_id: r.get(5)?,
                character_vote: r.get(6)?,
                score: r.get(7)?,
                is_eliminated: r.get(8)?,
                is_excluded: r.get(9)?,
                eliminated_round: r.get(10)?,
            })
        },
    )
}

/// Le joueur a-t-il une place dans la partie ?
pub fn has_seat(c: &impl Q, game_id: i64, player_id: i64) -> AppResult<bool> {
    c.exists(
        "SELECT 1 FROM binome_player bp JOIN binomes b ON b.id = bp.binome_id WHERE b.game_id = ?1 AND bp.player_id = ?2",
        [game_id, player_id],
    )
}

/// Joueur de la partie ou membre du salon (`Game::canBeWatchedBy`).
pub fn can_watch(c: &impl Q, g: &Game, player_id: i64) -> AppResult<bool> {
    Ok(has_seat(c, g.id, player_id)? || c.exists("SELECT 1 FROM room_player WHERE room_id = ?1 AND player_id = ?2", [g.room_id, player_id])?)
}

#[derive(Debug, Clone)]
pub struct Round {
    pub id: i64,
    pub game_id: i64,
    pub number: i64,
    pub current_player_id: i64,
    pub is_finished: bool,
}

const ROUND_COLS: &str = "id, game_id, number, current_player_id, is_finished";

fn round_row(r: &Row) -> rusqlite::Result<Round> {
    Ok(Round { id: r.get(0)?, game_id: r.get(1)?, number: r.get(2)?, current_player_id: r.get(3)?, is_finished: r.get(4)? })
}

pub fn round(c: &impl Q, id: i64) -> AppResult<Option<Round>> {
    c.opt(&format!("SELECT {ROUND_COLS} FROM rounds WHERE id = ?1"), [id], round_row)
}

/// Round non terminé le plus récent (`Game::currentRound`).
pub fn current_round(c: &impl Q, game_id: i64) -> AppResult<Option<Round>> {
    c.opt(
        &format!("SELECT {ROUND_COLS} FROM rounds WHERE game_id = ?1 AND is_finished = 0 ORDER BY created_at DESC, id DESC LIMIT 1"),
        [game_id],
        round_row,
    )
}

pub fn rounds(c: &impl Q, game_id: i64) -> AppResult<Vec<Round>> {
    c.all(&format!("SELECT {ROUND_COLS} FROM rounds WHERE game_id = ?1 ORDER BY number, id"), [game_id], round_row)
}

#[derive(Debug, Clone)]
pub struct Action {
    pub id: i64,
    pub round_id: i64,
    pub player_id: i64,
    pub target_player_id: Option<i64>,
    pub kind: String,
    pub content: Option<String>,
    pub character_name: Option<String>,
    pub is_valid: bool,
    pub accusation_correct: Option<bool>,
    pub accusation_confirmed: Option<bool>,
    pub answer: Option<String>,
    pub answer_corrected_from: Option<String>,
    pub answer_corrected_at: Option<String>,
    pub card_draw_status: Option<String>,
    pub card_count: Option<i64>,
    pub card_draw_deadline: Option<String>,
    pub drawn_word: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

const ACTION_COLS: &str = "id, round_id, player_id, target_player_id, type, content, character_name, is_valid, accusation_correct,
    accusation_confirmed, answer, answer_corrected_from, answer_corrected_at, card_draw_status, card_count, card_draw_deadline,
    drawn_word, created_at, updated_at";

fn action_row(r: &Row) -> rusqlite::Result<Action> {
    Ok(Action {
        id: r.get(0)?,
        round_id: r.get(1)?,
        player_id: r.get(2)?,
        target_player_id: r.get(3)?,
        kind: r.get(4)?,
        content: r.get(5)?,
        character_name: r.get(6)?,
        is_valid: r.get(7)?,
        accusation_correct: r.get(8)?,
        accusation_confirmed: r.get(9)?,
        answer: r.get(10)?,
        answer_corrected_from: r.get(11)?,
        answer_corrected_at: r.get(12)?,
        card_draw_status: r.get(13)?,
        card_count: r.get(14)?,
        card_draw_deadline: r.get(15)?,
        drawn_word: r.get(16)?,
        created_at: r.get(17)?,
        updated_at: r.get(18)?,
    })
}

impl Action {
    pub fn is_question(&self) -> bool {
        self.kind == "question"
    }

    /// État public du tirage : shared / kept / expired → « done » (la cible ne
    /// doit pas savoir si son mot a circulé).
    pub fn public_card_draw(&self) -> Value {
        match &self.card_draw_status {
            None => Value::Null,
            Some(s) => json!({
                "status": if s == "pending" || s == "drawn" { s.as_str() } else { "done" },
                "card_count": self.card_count,
                "deadline": clock::iso(self.card_draw_deadline.as_deref()),
            }),
        }
    }
}

pub fn action(c: &impl Q, id: i64) -> AppResult<Option<Action>> {
    c.opt(&format!("SELECT {ACTION_COLS} FROM actions WHERE id = ?1"), [id], action_row)
}

pub fn round_actions(c: &impl Q, round_id: i64) -> AppResult<Vec<Action>> {
    c.all(&format!("SELECT {ACTION_COLS} FROM actions WHERE round_id = ?1 ORDER BY id"), [round_id], action_row)
}

pub fn game_actions(c: &impl Q, game_id: i64) -> AppResult<Vec<Action>> {
    c.all(
        &format!("SELECT {ACTION_COLS} FROM actions WHERE round_id IN (SELECT id FROM rounds WHERE game_id = ?1) ORDER BY id"),
        [game_id],
        action_row,
    )
}

/// Pseudos des joueurs, pour composer les charges utiles.
pub fn pseudos(c: &impl Q, ids: impl IntoIterator<Item = i64>) -> AppResult<HashMap<i64, String>> {
    let mut out = HashMap::new();
    for id in ids {
        if let std::collections::hash_map::Entry::Vacant(e) = out.entry(id) {
            if let Some(p) = player(c, id)? {
                e.insert(p.pseudo);
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct Character {
    pub id: i64,
    pub universe_id: i64,
    pub name: String,
    pub forbidden_words: Vec<String>,
    pub level_affectation: i64,
    pub verif_manual: bool,
    pub hidden: bool,
    pub has_image: bool,
    pub updated_at: Option<String>,
}

pub fn character(c: &impl Q, id: i64) -> AppResult<Option<Character>> {
    c.opt(
        "SELECT id, universe_id, name, forbidden_words, level_affectation, verif_manual, hidden,
                (image IS NOT NULL AND image <> ''), updated_at FROM characters WHERE id = ?1",
        [id],
        |r| {
            let words: String = r.get(3)?;
            Ok(Character {
                id: r.get(0)?,
                universe_id: r.get(1)?,
                name: r.get(2)?,
                forbidden_words: serde_json::from_str(&words).unwrap_or_default(),
                level_affectation: r.get(4)?,
                verif_manual: r.get(5)?,
                hidden: r.get(6)?,
                has_image: r.get(7)?,
                updated_at: r.get(8)?,
            })
        },
    )
}
