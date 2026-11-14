//! Profils joueurs : connexion par pseudo (mot de passe facultatif), jeton de
//! profil signé, page profil (pseudo, mot de passe, parties).
//!
//! Le jeton (`X-Profile-Token`) protège le PROFIL : sa page, sa photo quand il
//! est protégé, et l'entrée dans un salon sous un pseudo protégé.
//!
//! Jeton : `<player_id>.<HMAC-SHA256(secret, "profile|id|token_version")>` en
//! hexadécimal. Le secret est généré par le serveur (`app_secrets`) ; poser ou
//! changer le mot de passe incrémente `token_version`, ce qui invalide tous les
//! jetons émis jusque-là (ceux d'un inconnu connecté au profil quand il était public).
//!
//! Depuis le point 5 de la checklist, ce jeton prouve aussi l'identité du joueur
//! pour toutes les actions de jeu (`player_id` vérifié par `input::Caller`) et pour
//! le WebSocket (`?token=`). Il est émis à la création / l'arrivée dans un salon.
//!
//! Profil administrateur (le « propriétaire ») : pseudo `BINOME_ADMIN_PSEUDO`, ouvert
//! uniquement par le mot de passe d'administration (jamais par un mot de passe de profil).
//! Il peut désigner d'autres administrateurs (`admin_granted_at`, migration 7) : ceux-ci
//! accèdent à l'administration avec leur jeton de profil, tant que leur profil a un mot
//! de passe, mais ne peuvent pas désigner d'administrateur à leur tour.

use axum::http::StatusCode;
use hmac::{Hmac, KeyInit, Mac};
use rusqlite::Transaction;
use serde_json::{json, Value};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::clock::{self, now_db};
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Input, V};
use crate::services::common::*;
use crate::services::Out;
use crate::support::text::ci_key;

pub const PASSWORD_MIN: usize = 6;
/// Un salon sans partie en cours, inactif depuis plus longtemps, est abandonné.
pub const LOBBY_TTL_HOURS: i64 = 12;

/// Adhésion « vivante » (alias `rp` = room_player, `r` = rooms ; paramètre
/// `:since` = [`lobby_cutoff`]) : partie en cours dans le salon, ou salon qui
/// attend une partie (aucune partie terminée depuis l'arrivée du joueur, et de
/// l'activité depuis moins de [`LOBBY_TTL_HOURS`] heures). Le reste est mort :
/// salon d'une partie finie (le front quitte la partie sans quitter le salon) ou
/// hall abandonné.
pub const LIVE_MEMBERSHIP: &str = "(EXISTS (SELECT 1 FROM games g WHERE g.room_id = r.id AND g.status = 'in_progress')
     OR (NOT EXISTS (SELECT 1 FROM games g WHERE g.room_id = r.id AND g.status = 'finished' AND g.updated_at >= rp.created_at)
         AND MAX(COALESCE(r.created_at, ''),
                 COALESCE((SELECT MAX(COALESCE(x.updated_at, x.created_at)) FROM room_player x WHERE x.room_id = r.id), '')) >= :since))";

/// Seuil d'inactivité d'un hall, au format des dates en base.
pub fn lobby_cutoff() -> String {
    clock::in_seconds_db(-LOBBY_TTL_HOURS * 3600)
}
pub const PASSWORD_MAX: usize = 128;

const PSEUDO_MESSAGES: &[(&str, &str)] = &[
    ("pseudo.required", "Le pseudo est requis."),
    ("pseudo.min", "Le pseudo doit faire au moins 2 caractères."),
    ("pseudo.max", "Le pseudo ne peut pas dépasser 20 caractères."),
];
const PASSWORD_MESSAGES: &[(&str, &str)] = &[
    ("password.required", "Choisis un mot de passe."),
    ("password.min", "Le mot de passe doit faire au moins 6 caractères."),
    ("password.max", "Le mot de passe ne peut pas dépasser 128 caractères."),
    ("current_password.required", "Entre ton mot de passe actuel."),
];

#[derive(Debug, Clone)]
pub struct Profile {
    pub id: i64,
    pub pseudo: String,
    pub password_hash: Option<String>,
    pub password_prompted: bool,
    pub token_version: i64,
    /// Validé à la main par l'administrateur (`validated_at`, migration 6).
    pub validated: bool,
    /// Désigné administrateur par le propriétaire (`admin_granted_at`, migration 7).
    pub admin_granted: bool,
}

const COLS: &str = "id, pseudo, password_hash, password_prompted, token_version, validated_at IS NOT NULL, admin_granted_at IS NOT NULL";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: r.get(0)?,
        pseudo: r.get(1)?,
        password_hash: r.get(2)?,
        password_prompted: r.get(3)?,
        token_version: r.get(4)?,
        validated: r.get(5)?,
        admin_granted: r.get(6)?,
    })
}

pub fn by_pseudo(c: &impl Q, pseudo: &str) -> AppResult<Option<Profile>> {
    c.opt(&format!("SELECT {COLS} FROM players WHERE pseudo = ?1"), [pseudo], row)
}

pub fn by_id(c: &impl Q, id: i64) -> AppResult<Option<Profile>> {
    c.opt(&format!("SELECT {COLS} FROM players WHERE id = ?1"), [id], row)
}

/// Le pseudo est-il celui du profil administrateur (insensible casse / accents) ?
pub fn is_admin_pseudo(env: &Env, pseudo: &str) -> bool {
    env.admin_pseudo.as_deref().is_some_and(|a| ci_key(a) == ci_key(pseudo))
}

impl Profile {
    /// Profil propriétaire (`BINOME_ADMIN_PSEUDO`).
    pub fn is_admin(&self, env: &Env) -> bool {
        is_admin_pseudo(env, &self.pseudo)
    }
    /// Administrateur désigné, avec accès effectif : il faut un mot de passe au profil
    /// (un profil public s'ouvre avec le seul pseudo).
    pub fn is_delegate_admin(&self, env: &Env) -> bool {
        self.admin_granted && self.password_hash.is_some() && !self.is_admin(env)
    }
    /// `"owner"` (propriétaire), `"admin"` (désigné) ou `None`.
    pub fn admin_role(&self, env: &Env) -> Option<&'static str> {
        if self.is_admin(env) {
            Some("owner")
        } else if self.is_delegate_admin(env) {
            Some("admin")
        } else {
            None
        }
    }
    /// Profil fermé : il faut son mot de passe (ou le mot de passe admin) pour s'y connecter.
    pub fn is_protected(&self, env: &Env) -> bool {
        self.password_hash.is_some() || self.is_admin(env)
    }
    /// Accès à l'historique et aux emojis personnalisés (les administrateurs l'ont toujours).
    pub fn is_validated(&self, env: &Env) -> bool {
        self.validated || self.admin_role(env).is_some()
    }
}

// ─── Jeton ──────────────────────────────────────────────────────────────────

fn sign(secret: &[u8], id: i64, version: i64) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("clé HMAC");
    mac.update(format!("profile|{id}|{version}").as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn token(env: &Env, p: &Profile) -> String {
    format!("{}.{}", p.id, sign(&env.profile_secret, p.id, p.token_version))
}

/// Profil désigné par le jeton ; `None` s'il est absent, mal formé ou périmé.
pub fn authenticate(c: &impl Q, env: &Env, token: Option<&str>) -> AppResult<Option<Profile>> {
    authenticate_with(c, &env.profile_secret, token)
}

/// `authenticate` avec le seul secret (WebSocket, hors `Env`).
pub fn authenticate_with(c: &impl Q, secret: &[u8], token: Option<&str>) -> AppResult<Option<Profile>> {
    let Some((id, mac)) = token.and_then(|t| t.split_once('.')) else { return Ok(None) };
    let Ok(id) = id.parse::<i64>() else { return Ok(None) };
    let Some(p) = by_id(c, id)? else { return Ok(None) };
    let expected = sign(secret, p.id, p.token_version);
    Ok((mac.len() == expected.len() && bool::from(mac.as_bytes().ct_eq(expected.as_bytes()))).then_some(p))
}

/// Profil connecté, sinon 401.
pub fn require(c: &impl Q, env: &Env, token: Option<&str>) -> AppResult<Profile> {
    authenticate(c, env, token)?.ok_or_else(|| AppError::Status(StatusCode::UNAUTHORIZED, "Connecte-toi à ton profil.".into()))
}

/// Entrée dans un salon sous `pseudo` : un profil protégé exige son jeton.
/// Le pseudo admin est réservé même avant la création de son profil.
pub fn guard_pseudo(c: &impl Q, env: &Env, pseudo: &str, token: Option<&str>) -> AppResult<()> {
    match by_pseudo(c, pseudo)? {
        Some(p) if p.is_protected(env) => {
            if authenticate(c, env, token)?.is_some_and(|a| a.id == p.id) {
                Ok(())
            } else {
                Err(AppError::forbidden(format!(
                    "Le profil « {} » est protégé par un mot de passe : connecte-toi depuis l'onglet Connexion.",
                    p.pseudo
                )))
            }
        }
        None if is_admin_pseudo(env, pseudo) => Err(AppError::forbidden("Ce pseudo est réservé.")),
        _ => Ok(()),
    }
}

// ─── Charges utiles ─────────────────────────────────────────────────────────

pub fn profile_json(c: &impl Q, env: &Env, p: &Profile) -> AppResult<Value> {
    let urls = avatar_urls(c, env, std::iter::once(p.pseudo.as_str()))?;
    Ok(json!({
        "id": p.id,
        "pseudo": p.pseudo,
        "avatar_url": urls.get(&ci_key(&p.pseudo)),
        "has_password": p.password_hash.is_some(),
        "password_prompted": p.password_prompted,
        "is_admin": p.is_admin(env),
        "is_validated": p.is_validated(env),
        "admin_role": p.admin_role(env),
    }))
}

/// Salon actuel du joueur (le dernier rejoint) et sa partie en cours.
/// `game_id` : partie où il a une place ; `room_game_id` : partie du salon,
/// qu'il peut suivre en spectateur s'il est arrivé après le lancement.
///
/// En fin de partie, le front renvoie à l'accueil sans quitter le salon : le
/// joueur en reste membre, et des halls abandonnés traînent aussi. Un salon ne
/// compte donc que s'il a une partie en cours, ou s'il attend une partie : aucune
/// partie terminée depuis l'arrivée du joueur ET de l'activité (création du
/// salon, arrivée ou « prêt » d'un joueur présent) depuis moins de
/// [`LOBBY_TTL_HOURS`] heures. `rooms.updated_at` n'entre pas en compte : un
/// départ (transfert d'hôte) le rafraîchit sans que le salon soit actif.
/// Même règle que le nettoyage `binome prune-rooms` ([`LIVE_MEMBERSHIP`]).
pub fn current(c: &impl Q, player_id: i64) -> AppResult<Value> {
    let Some((room_id, code, host)) = c.opt(
        &format!(
            "SELECT r.id, r.code, r.created_by FROM room_player rp JOIN rooms r ON r.id = rp.room_id
             WHERE rp.player_id = :pid AND {LIVE_MEMBERSHIP}
             ORDER BY rp.id DESC LIMIT 1"
        ),
        rusqlite::named_params! { ":pid": player_id, ":since": lobby_cutoff() },
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)),
    )?
    else {
        return Ok(Value::Null);
    };
    let room_game = current_game_id(c, room_id)?;
    let seated = match room_game {
        Some(g) if has_seat(c, g, player_id)? => Some(g),
        _ => None,
    };
    Ok(json!({ "room_id": room_id, "room_code": code, "host_id": host, "game_id": seated, "room_game_id": room_game }))
}

/// Parties où le joueur a eu une place, la plus récente d'abord. Le personnage
/// n'apparaît que pour une partie terminée (invariant de sécurité de l'API).
fn games(c: &impl Q, p: &Profile) -> AppResult<Value> {
    let rows = c.all(
        "SELECT g.id, g.status, g.created_at, g.room_code, ch.name, COALESCE(s.score, bp.score), COALESCE(s.is_winner, 0),
                bp.is_eliminated, bp.is_excluded
         FROM binome_player bp
         JOIN binomes b ON b.id = bp.binome_id
         JOIN games g ON g.id = b.game_id
         LEFT JOIN characters ch ON ch.id = bp.character_id
         LEFT JOIN game_stats s ON s.game_id = g.id AND s.player_pseudo = ?2
         WHERE bp.player_id = ?1
         ORDER BY g.id DESC",
        rusqlite::params![p.id, p.pseudo],
        |r| {
            let status: String = r.get(1)?;
            let finished = status == "finished";
            Ok(json!({
                "game_id": r.get::<_, i64>(0)?,
                "status": status,
                "played_at": clock::iso(r.get::<_, Option<String>>(2)?.as_deref()),
                "room_code": r.get::<_, Option<String>>(3)?,
                "character_name": if finished { r.get::<_, Option<String>>(4)? } else { None },
                "score": r.get::<_, i64>(5)?,
                "is_winner": r.get::<_, bool>(6)?,
                "is_eliminated": r.get::<_, bool>(7)?,
                "is_excluded": r.get::<_, bool>(8)?,
            }))
        },
    )?;
    Ok(Value::Array(rows))
}

fn totals(c: &impl Q, p: &Profile) -> AppResult<Value> {
    let (played, score, wins): (i64, i64, i64) = c
        .opt(
            "SELECT COUNT(*), COALESCE(SUM(s.score), 0), COALESCE(SUM(s.is_winner), 0)
             FROM game_stats s JOIN games g ON g.id = s.game_id
             WHERE s.player_pseudo = ?1 AND g.status = 'finished'",
            [&p.pseudo],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?
        .unwrap_or_default();
    Ok(json!({ "games_played": played, "total_score": score, "wins": wins }))
}

fn session_body(c: &impl Q, env: &Env, p: &Profile, message: &str) -> AppResult<Value> {
    Ok(json!({ "message": message, "token": token(env, p), "player": profile_json(c, env, p)? }))
}

// ─── Connexion ──────────────────────────────────────────────────────────────

/// POST /profile/login — validation : (pseudo, mot de passe saisi ou None).
pub fn login_input(input: &Input) -> AppResult<(String, Option<String>)> {
    let mut v = V::new(input).messages(PSEUDO_MESSAGES);
    let pseudo = v.req_str("pseudo", Some(2), Some(20));
    let password = match input.get("password") {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::String(_)) | None => None,
        Some(_) => {
            v.fail("password", "string", "The password field must be a string.".into());
            None
        }
    };
    v.finish()?;
    Ok((pseudo.expect("validé"), password))
}

/// Ce qu'il faut vérifier avant d'ouvrir le profil `pseudo` (lecture, avant PBKDF2).
pub enum Gate {
    /// Profil public (ou à créer) : rien à vérifier.
    Open,
    /// Mot de passe admin (`BINOME_ADMIN_PASSWORD_HASH`).
    Admin,
    /// Mot de passe du profil : (id, empreinte).
    Password(i64, String),
}

pub fn gate(c: &impl Q, env: &Env, pseudo: &str) -> AppResult<Gate> {
    if is_admin_pseudo(env, pseudo) {
        return Ok(Gate::Admin);
    }
    Ok(match by_pseudo(c, pseudo)? {
        Some(Profile { id, password_hash: Some(h), .. }) => Gate::Password(id, h),
        _ => Gate::Open,
    })
}

/// Connexion validée (mot de passe déjà vérifié hors transaction si besoin).
/// `verified` : l'empreinte contrôlée (None pour un profil public) — si le
/// profil a changé entre-temps (mot de passe posé ou changé), on refuse.
pub fn login(tx: &Transaction, env: &Env, pseudo: &str, verified: Option<&str>, admin_verified: bool) -> AppResult<Out> {
    let p = match by_pseudo(tx, pseudo)? {
        Some(p) => p,
        None => {
            if is_admin_pseudo(env, pseudo) && !admin_verified {
                return Err(AppError::forbidden("Ce pseudo est réservé."));
            }
            let now = now_db();
            let id = tx.insert("INSERT INTO players (pseudo, created_at, updated_at) VALUES (?1, ?2, ?2)", (pseudo, &now))?;
            by_id(tx, id)?.expect("profil créé")
        }
    };
    if p.is_admin(env) {
        if !admin_verified {
            return Err(AppError::field("password", "Mot de passe incorrect."));
        }
    } else if let Some(h) = &p.password_hash {
        if verified != Some(h.as_str()) {
            return Err(AppError::field("password", "Mot de passe incorrect."));
        }
    } else {
        // Profil public : pas de prise de contrôle d'un joueur connecté ailleurs
        crate::services::rooms::pseudo_in_use(tx, env, &p.pseudo)?;
    }
    let mut body = session_body(tx, env, &p, "Connecté.")?;
    body["current"] = current(tx, p.id)?;
    Ok(Out::ok(body))
}

// ─── Page profil ────────────────────────────────────────────────────────────

/// GET /profile
pub fn show(c: &impl Q, env: &Env, token: Option<&str>) -> AppResult<Out> {
    let p = require(c, env, token)?;
    Ok(Out::ok(json!({
        "player": profile_json(c, env, &p)?,
        "current": current(c, p.id)?,
        "totals": totals(c, &p)?,
        "games": games(c, &p)?,
    })))
}

/// PATCH /profile — changement de pseudo. Les données indexées par pseudo
/// (photo, statistiques, emojis créés) suivent le profil.
pub fn rename(tx: &Transaction, env: &Env, token: Option<&str>, input: &Input) -> AppResult<Out> {
    let p = require(tx, env, token)?;
    let mut v = V::new(input).messages(PSEUDO_MESSAGES);
    let new = v.req_str("pseudo", Some(2), Some(20));
    v.finish()?;
    let new = new.expect("validé");
    if new == p.pseudo {
        return Ok(Out::ok(session_body(tx, env, &p, "Pseudo inchangé.")?));
    }
    if p.is_admin(env) {
        return Err(AppError::forbidden("Le pseudo administrateur se change dans la configuration du serveur (BINOME_ADMIN_PSEUDO)."));
    }
    if is_admin_pseudo(env, &new) {
        return Err(AppError::field("pseudo", "Ce pseudo est réservé."));
    }
    if tx.exists("SELECT 1 FROM room_player WHERE player_id = ?1", [p.id])? {
        return Err(AppError::rule("Quitte ton salon avant de changer de pseudo."));
    }
    let same_person = ci_key(&new) == ci_key(&p.pseudo);
    if !same_person {
        if tx.exists("SELECT 1 FROM players WHERE pseudo = ?1 AND id <> ?2", rusqlite::params![new, p.id])? {
            return Err(AppError::field("pseudo", "Ce pseudo est déjà pris."));
        }
        if tx.exists("SELECT 1 FROM game_stats WHERE player_pseudo = ?1", [&new])? {
            return Err(AppError::field("pseudo", "Ce pseudo a déjà un historique de parties : choisis-en un autre."));
        }
        // Photo laissée par un ancien joueur de ce pseudo : elle ne passe pas au nouveau
        tx.run("DELETE FROM player_avatars WHERE pseudo = ?1", [&new])?;
    }
    let now = now_db();
    tx.run("UPDATE players SET pseudo = ?1, updated_at = ?2 WHERE id = ?3", rusqlite::params![new, now, p.id])?;
    tx.run("UPDATE player_avatars SET pseudo = ?1, updated_at = ?2 WHERE pseudo = ?3", rusqlite::params![new, now, p.pseudo])?;
    tx.run("UPDATE game_stats SET player_pseudo = ?1 WHERE player_pseudo = ?2", rusqlite::params![new, p.pseudo])?;
    tx.run("UPDATE custom_reactions SET created_by_pseudo = ?1 WHERE created_by_pseudo = ?2", rusqlite::params![new, p.pseudo])?;
    let p = by_id(tx, p.id)?.expect("profil");
    Ok(Out::ok(session_body(tx, env, &p, "Pseudo modifié.")?))
}

/// Mot de passe : validation de la requête (nouveau mot de passe et/ou actuel).
pub fn password_input(input: &Input, needs_new: bool, needs_current: bool) -> AppResult<(Option<String>, Option<String>)> {
    let mut v = V::new(input).messages(PASSWORD_MESSAGES);
    let new = if needs_new { v.req_str("password", Some(PASSWORD_MIN), Some(PASSWORD_MAX)) } else { None };
    let current = if needs_current { v.req_str("current_password", None, Some(PASSWORD_MAX)) } else { None };
    v.finish()?;
    Ok((new, current))
}

/// PUT /profile/password (empreinte calculée hors transaction). `seen` :
/// l'état lu avant la vérification ; s'il a changé entre-temps, on refuse.
pub fn set_password(tx: &Transaction, env: &Env, seen: &Profile, new_hash: Option<&str>) -> AppResult<Out> {
    let changed = match new_hash {
        // Pose / changement : les jetons émis jusque-là sont révoqués
        Some(h) => tx.run(
            "UPDATE players SET password_hash = ?1, password_prompted = 1, token_version = token_version + 1, updated_at = ?2
             WHERE id = ?3 AND token_version = ?4 AND password_hash IS ?5",
            rusqlite::params![h, now_db(), seen.id, seen.token_version, seen.password_hash],
        )?,
        // Un administrateur désigné garde un mot de passe : sans lui, son profil (et
        // l'administration) s'ouvrirait avec le seul pseudo
        None if seen.admin_granted || tx.exists("SELECT 1 FROM players WHERE id = ?1 AND admin_granted_at IS NOT NULL", [seen.id])? => {
            return Err(AppError::rule("Tu es administrateur : ton profil doit garder un mot de passe."));
        }
        None => tx.run(
            "UPDATE players SET password_hash = NULL, updated_at = ?1 WHERE id = ?2 AND token_version = ?3 AND password_hash IS ?4",
            rusqlite::params![now_db(), seen.id, seen.token_version, seen.password_hash],
        )?,
    };
    if changed == 0 {
        return Err(AppError::rule("Ton profil a changé entre-temps : recharge la page et réessaie."));
    }
    let p = by_id(tx, seen.id)?.expect("profil");
    let msg = if new_hash.is_some() { "Mot de passe enregistré : ton profil est protégé." } else { "Mot de passe retiré : ton profil est public." };
    Ok(Out::ok(session_body(tx, env, &p, msg)?))
}

/// POST /profile/password-prompt — la proposition de mot de passe a été vue.
pub fn dismiss_prompt(tx: &Transaction, env: &Env, token: Option<&str>) -> AppResult<Out> {
    let p = require(tx, env, token)?;
    tx.run("UPDATE players SET password_prompted = 1, updated_at = ?1 WHERE id = ?2", rusqlite::params![now_db(), p.id])?;
    let p = by_id(tx, p.id)?.expect("profil");
    Ok(Out::ok(json!({ "player": profile_json(tx, env, &p)? })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Salon d'une partie terminée (le joueur n'a jamais cliqué « quitter ») :
    /// pas « en cours ». Partie en cours, ou arrivée après la fin : en cours.
    #[test]
    fn current_ignores_rooms_of_finished_games() {
        let dir = std::env::temp_dir().join(format!("binome-profile-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("t.sqlite")).unwrap();
        db.with_writer(|c| {
            let ago = |h: i64| clock::in_seconds_db(-h * 3600);
            c.execute(
                "INSERT INTO players (id, pseudo) VALUES (1, 'Hote'), (2, 'Tard'), (3, 'Seul')",
                [],
            )?;
            c.execute("INSERT INTO rooms (id, code, created_by, created_at, updated_at) VALUES (7, 'ABCDEF', 1, ?1, ?1)", [ago(3)])?;
            c.execute("INSERT INTO room_player (room_id, player_id, created_at, updated_at) VALUES (7, 1, ?1, ?1)", [ago(3)])?;
            c.execute("INSERT INTO games (id, room_id, status, updated_at) VALUES (9, 7, 'finished', ?1)", [ago(2)])?;
            assert_eq!(current(c, 1)?, Value::Null, "salon d'une partie terminée");
            // Arrivé après la fin : il attend la prochaine partie
            c.execute("INSERT INTO room_player (room_id, player_id, created_at, updated_at) VALUES (7, 2, ?1, ?1)", [ago(1)])?;
            assert_eq!(current(c, 2)?["room_code"], "ABCDEF");
            // Nouvelle partie en cours dans le salon : il redevient « en cours »
            c.run("INSERT INTO games (id, room_id, status) VALUES (10, 7, 'in_progress')", [])?;
            assert_eq!(current(c, 1)?["room_game_id"], 10);
            // Hall jamais lancé : en cours s'il est récent, abandonné au-delà de 12 h
            c.execute("INSERT INTO rooms (id, code, created_by, created_at, updated_at) VALUES (8, 'GHIJKL', 3, ?1, ?1)", [ago(1)])?;
            c.execute("INSERT INTO room_player (room_id, player_id, created_at, updated_at) VALUES (8, 3, ?1, ?1)", [ago(1)])?;
            assert_eq!(current(c, 3)?["room_code"], "GHIJKL");
            c.execute("UPDATE rooms SET created_at = ?1, updated_at = ?1 WHERE id = 8", [ago(13)])?;
            c.execute("UPDATE room_player SET created_at = ?1, updated_at = ?1 WHERE room_id = 8", [ago(13)])?;
            assert_eq!(current(c, 3)?, Value::Null, "hall abandonné depuis plus de 12 h");
            Ok(())
        })
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rôles : le propriétaire l'est par son pseudo ; un désigné n'a l'accès
    /// qu'avec un mot de passe ; tous deux sont toujours validés.
    #[test]
    fn admin_roles() {
        let env = Env {
            prefix: String::new(),
            hub: Arc::new(crate::realtime::hub::Hub::default()),
            profile_secret: Arc::new(vec![]),
            admin_pseudo: Some("Patron".into()),
        };
        let p = |pseudo: &str, password: bool, granted: bool| Profile {
            id: 1,
            pseudo: pseudo.into(),
            password_hash: password.then(|| "h".to_string()),
            password_prompted: false,
            token_version: 0,
            validated: false,
            admin_granted: granted,
        };
        assert_eq!(p("pAtrón", false, false).admin_role(&env), Some("owner"));
        assert_eq!(p("Ami", true, true).admin_role(&env), Some("admin"));
        assert_eq!(p("Ami", false, true).admin_role(&env), None, "désigné sans mot de passe");
        assert_eq!(p("Ami", true, false).admin_role(&env), None);
        assert!(p("Ami", true, true).is_validated(&env));
        assert!(!p("Ami", true, false).is_validated(&env));
    }

    #[test]
    fn token_signature_depends_on_id_version_and_secret() {
        let a = sign(b"secret-a", 7, 0);
        assert_eq!(a.len(), 64);
        assert_eq!(a, sign(b"secret-a", 7, 0));
        assert_ne!(a, sign(b"secret-a", 8, 0));
        assert_ne!(a, sign(b"secret-a", 7, 1));
        assert_ne!(a, sign(b"secret-b", 7, 0));
    }
}
