//! Contrôles de l'administrateur (migration 6) : modes de jeu et cosmos proposés
//! aux hôtes, binômes de personnages écartés du tirage, validation manuelle des
//! profils (accès à l'historique et aux emojis personnalisés).

use rusqlite::Transaction;
use serde_json::{json, Value};

use crate::clock::now_db;
use crate::db::Q;
use crate::error::{AppError, AppResult};
use crate::http::input::{Caller, Input, V};
use crate::services::common::*;
use crate::services::profile;
use crate::services::Out;

/// Modes de jeu connus : (clé stockée dans `rooms.game_mode`, libellé).
pub const GAME_MODES: &[(&str, &str)] = &[("random", "Aléatoire"), ("cosmos", "Cosmos imposé")];

/// Condition SQL d'un personnage jouable, sur la table `characters` non aliasée :
/// relu (`verif_manual`), non refusé (`hidden`), binôme non désactivé par l'admin.
pub const PLAYABLE_CHARACTER: &str = "verif_manual = 1 AND hidden = 0 AND NOT EXISTS (
    SELECT 1 FROM disabled_binomes db
    WHERE db.universe_id = characters.universe_id AND db.level_affectation = characters.level_affectation)";

/// Univers ayant au moins un binôme jouable (2 personnages jouables au même niveau).
pub fn playable_universes_sql() -> String {
    format!("SELECT universe_id FROM characters WHERE {PLAYABLE_CHARACTER} GROUP BY universe_id, level_affectation HAVING COUNT(*) >= 2")
}

fn mode_label(mode: &str) -> &str {
    GAME_MODES.iter().find(|(k, _)| *k == mode).map_or(mode, |(_, l)| l)
}

// ─── Modes de jeu ───────────────────────────────────────────────────────────

/// Modes proposés aux hôtes, dans l'ordre de `GAME_MODES`. Réglage absent ou
/// illisible : tous les modes (comportement d'avant la migration 6).
pub fn enabled_modes(c: &impl Q) -> AppResult<Vec<&'static str>> {
    let raw: Option<String> = c.opt("SELECT value FROM app_settings WHERE name = 'game_modes'", [], |r| r.get(0))?;
    let Some(list) = raw.and_then(|r| serde_json::from_str::<Vec<String>>(&r).ok()) else {
        return Ok(GAME_MODES.iter().map(|(k, _)| *k).collect());
    };
    Ok(GAME_MODES.iter().map(|(k, _)| *k).filter(|k| list.iter().any(|m| m == k)).collect())
}

/// Mode d'un salon à sa création : Aléatoire s'il est proposé, sinon le premier proposé.
pub fn default_mode(c: &impl Q) -> AppResult<&'static str> {
    let modes = enabled_modes(c)?;
    Ok(if modes.contains(&"random") { "random" } else { modes.first().copied().unwrap_or("random") })
}

pub fn cosmos_enabled(c: &impl Q, id: i64) -> AppResult<bool> {
    Ok(c.opt("SELECT is_enabled FROM cosmos WHERE id = ?1", [id], |r| r.get::<_, bool>(0))?.unwrap_or(false))
}

/// Réglages du salon : un mode ou un cosmos désactivé ne peut pas être CHOISI
/// (le garder tel quel reste permis, pour régler les autres options ; le
/// lancement le refusera).
pub fn check_settings_choice(c: &impl Q, room: &Room, mode: &str, cosmos_id: Option<i64>) -> AppResult<()> {
    if mode != room.game_mode && !enabled_modes(c)?.contains(&mode) {
        return Err(AppError::field("game_mode", format!("Le mode « {} » n'est pas disponible.", mode_label(mode))));
    }
    if let Some(id) = cosmos_id {
        if Some(id) != room.cosmos_id && !cosmos_enabled(c, id)? {
            return Err(AppError::field("cosmos_id", "Ce cosmos n'est pas disponible."));
        }
    }
    Ok(())
}

/// Lancement : le mode et le cosmos du salon doivent être proposés.
pub fn check_room_playable(c: &impl Q, room: &Room) -> AppResult<()> {
    if !enabled_modes(c)?.contains(&room.game_mode.as_str()) {
        return Err(AppError::unprocessable(format!(
            "Le mode « {} » n'est plus disponible : choisis un autre mode de jeu.",
            mode_label(&room.game_mode)
        )));
    }
    if room.game_mode == "cosmos" {
        match room.cosmos_id {
            None => return Err(AppError::unprocessable("Choisis un cosmos pour démarrer.")),
            Some(id) if !cosmos_enabled(c, id)? => {
                let name = cosmos_name(c, Some(id))?.unwrap_or_default();
                return Err(AppError::unprocessable(format!("Le cosmos « {name} » n'est plus disponible : choisis-en un autre.")));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

// ─── Validation des profils ─────────────────────────────────────────────────

/// Accès réservé aux profils validés par l'administrateur : 401 sans profil
/// connecté, 403 si le profil n'est pas (encore) validé.
pub fn require_validated(c: &impl Q, env: &Env, caller: Caller, what: &str) -> AppResult<()> {
    let p = match caller {
        Caller::Player(id) => profile::by_id(c, id)?,
        Caller::Anonymous | Caller::Invalid => None,
    };
    match p {
        Some(p) if p.is_validated(env) => Ok(()),
        Some(_) => Err(AppError::forbidden(format!("{what} est réservé aux profils validés par l'administrateur."))),
        None => Err(AppError::Status(axum::http::StatusCode::UNAUTHORIZED, format!("Connecte-toi à ton profil : {} est réservé aux profils validés.", what.to_lowercase()))),
    }
}

/// Le joueur `caller` a-t-il un profil validé ? (sans erreur, pour filtrer)
pub fn caller_validated(c: &impl Q, env: &Env, caller: Caller) -> AppResult<bool> {
    Ok(match caller {
        Caller::Player(id) => profile::by_id(c, id)?.is_some_and(|p| p.is_validated(env)),
        _ => false,
    })
}

// ─── Administration ─────────────────────────────────────────────────────────

/// GET /admin/settings — modes, cosmos et binômes, avec leur état.
pub fn settings_index(c: &impl Q) -> AppResult<Out> {
    let enabled = enabled_modes(c)?;
    let modes: Vec<Value> = GAME_MODES.iter().map(|(k, l)| json!({ "key": k, "label": l, "enabled": enabled.contains(k) })).collect();
    let playable = playable_universes_sql();
    let cosmos = c.all(
        &format!(
            "SELECT co.id, co.name, co.is_enabled,
                    (SELECT COUNT(*) FROM universes u WHERE u.cosmos_id = co.id),
                    (SELECT COUNT(*) FROM universes u WHERE u.cosmos_id = co.id AND u.id IN ({playable}))
             FROM cosmos co ORDER BY co.name"
        ),
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "is_enabled": r.get::<_, bool>(2)?,
                "universes_count": r.get::<_, i64>(3)?,
                "playable_universe_count": r.get::<_, i64>(4)?,
            }))
        },
    )?;
    // Binômes complets du catalogue (2 personnages relus et non refusés au même
    // niveau), qu'ils soient actifs ou désactivés.
    let binomes = c.all(
        "SELECT ch.universe_id, u.name, u.cosmos_id, ch.level_affectation, json_group_array(ch.name),
                EXISTS (SELECT 1 FROM disabled_binomes db WHERE db.universe_id = ch.universe_id AND db.level_affectation = ch.level_affectation)
         FROM characters ch JOIN universes u ON u.id = ch.universe_id
         WHERE ch.verif_manual = 1 AND ch.hidden = 0
         GROUP BY ch.universe_id, ch.level_affectation HAVING COUNT(*) >= 2
         ORDER BY u.name, ch.level_affectation",
        [],
        |r| {
            let names: Vec<String> = serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default();
            Ok(json!({
                "universe_id": r.get::<_, i64>(0)?,
                "universe_name": r.get::<_, String>(1)?,
                "cosmos_id": r.get::<_, Option<i64>>(2)?,
                "level_affectation": r.get::<_, i64>(3)?,
                "characters": names,
                "is_enabled": !r.get::<_, bool>(5)?,
            }))
        },
    )?;
    Ok(Out::ok(json!({ "game_modes": modes, "cosmos": cosmos, "binomes": binomes })))
}

/// PATCH /admin/settings/game-modes/{mode} — `{ enabled }`. Au moins un mode reste proposé.
pub fn set_game_mode(tx: &Transaction, mode: &str, input: &Input) -> AppResult<Out> {
    let Some(&(key, label)) = GAME_MODES.iter().find(|(k, _)| *k == mode) else { return Err(AppError::NotFound) };
    let mut v = V::new(input);
    let enabled = v.req_bool("enabled");
    v.finish()?;
    let enabled = enabled.expect("validé");
    let mut modes = enabled_modes(tx)?;
    modes.retain(|m| *m != key);
    if enabled {
        modes.push(key);
    }
    if modes.is_empty() {
        return Err(AppError::rule("Au moins un mode de jeu doit rester disponible."));
    }
    let ordered: Vec<&str> = GAME_MODES.iter().map(|(k, _)| *k).filter(|k| modes.contains(k)).collect();
    tx.run(
        "INSERT INTO app_settings (name, value) VALUES ('game_modes', ?1) ON CONFLICT (name) DO UPDATE SET value = excluded.value",
        [serde_json::to_string(&ordered).expect("json")],
    )?;
    let msg = format!("Mode « {label} » {}.", if enabled { "activé" } else { "désactivé" });
    Ok(Out::ok(json!({ "message": msg, "game_modes": ordered })))
}

/// PATCH /admin/cosmos/{cosmos} — `{ is_enabled }`.
pub fn set_cosmos(tx: &Transaction, cosmos_id: i64, input: &Input) -> AppResult<Out> {
    let name = cosmos_name(tx, Some(cosmos_id))?.ok_or(AppError::NotFound)?;
    let mut v = V::new(input);
    let enabled = v.req_bool("is_enabled");
    v.finish()?;
    let enabled = enabled.expect("validé");
    tx.run("UPDATE cosmos SET is_enabled = ?1, updated_at = ?2 WHERE id = ?3", (enabled, now_db(), cosmos_id))?;
    let msg = format!("Cosmos « {name} » {}.", if enabled { "activé" } else { "désactivé" });
    Ok(Out::ok(json!({ "message": msg, "cosmos": { "id": cosmos_id, "name": name, "is_enabled": enabled } })))
}

/// PATCH /admin/binomes — `{ universe_id, level_affectation, is_enabled }`.
pub fn set_binome(tx: &Transaction, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input);
    let universe_id = v.req_int("universe_id");
    let universe_id = v.exists(tx, "universe_id", universe_id, "SELECT 1 FROM universes WHERE id = ?1")?;
    let level = v.req_int("level_affectation");
    let enabled = v.req_bool("is_enabled");
    v.finish()?;
    let (universe_id, level, enabled) = (universe_id.expect("validé"), level.expect("validé"), enabled.expect("validé"));
    if enabled {
        tx.run("DELETE FROM disabled_binomes WHERE universe_id = ?1 AND level_affectation = ?2", (universe_id, level))?;
    } else {
        tx.run(
            "INSERT OR IGNORE INTO disabled_binomes (universe_id, level_affectation, created_at) VALUES (?1, ?2, ?3)",
            (universe_id, level, now_db()),
        )?;
    }
    let universe: String = tx.one("SELECT name FROM universes WHERE id = ?1", [universe_id])?;
    let msg = format!("Binôme {universe} (niveau {level}) {}.", if enabled { "activé" } else { "désactivé" });
    Ok(Out::ok(json!({
        "message": msg,
        "binome": { "universe_id": universe_id, "level_affectation": level, "is_enabled": enabled },
    })))
}

/// PATCH /admin/players/{player}/validation — `{ validated }`.
pub fn set_validation(tx: &Transaction, env: &Env, player_id: i64, input: &Input) -> AppResult<Out> {
    let p = profile::by_id(tx, player_id)?.ok_or(AppError::NotFound)?;
    let mut v = V::new(input);
    let validated = v.req_bool("validated");
    v.finish()?;
    if p.is_admin(env) {
        return Err(AppError::rule("Le profil administrateur est toujours validé."));
    }
    let validated = validated.expect("validé");
    let stamp = validated.then(now_db);
    tx.run("UPDATE players SET validated_at = ?1 WHERE id = ?2", (&stamp, p.id))?;
    let msg = format!("Profil de {} {}.", p.pseudo, if validated { "validé" } else { "n'est plus validé" });
    Ok(Out::ok(json!({ "message": msg, "player": { "id": p.id, "is_validated": validated, "validated_at": crate::clock::iso(stamp.as_deref()) } })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE app_settings (name TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE disabled_binomes (universe_id INTEGER, level_affectation INTEGER, created_at TEXT, PRIMARY KEY (universe_id, level_affectation));
             CREATE TABLE characters (id INTEGER PRIMARY KEY, universe_id INTEGER, level_affectation INTEGER, verif_manual INTEGER, hidden INTEGER);
             INSERT INTO characters VALUES (1, 10, 1, 1, 0), (2, 10, 1, 1, 0), (3, 11, 1, 1, 0), (4, 11, 1, 1, 0), (5, 11, 2, 1, 0), (6, 11, 2, 1, 0);",
        )
        .unwrap();
        c
    }

    #[test]
    fn modes_default_to_all_and_follow_setting() {
        let c = db();
        assert_eq!(enabled_modes(&c).unwrap(), vec!["random", "cosmos"]);
        assert_eq!(default_mode(&c).unwrap(), "random");
        c.execute("INSERT INTO app_settings VALUES ('game_modes', '[\"cosmos\", \"inconnu\"]')", []).unwrap();
        assert_eq!(enabled_modes(&c).unwrap(), vec!["cosmos"]);
        assert_eq!(default_mode(&c).unwrap(), "cosmos");
    }

    #[test]
    fn disabled_binome_leaves_the_draw() {
        let c = db();
        let playable = |c: &rusqlite::Connection| -> Vec<i64> { c.all(&format!("{} ORDER BY universe_id", playable_universes_sql()), [], |r| r.get(0)).unwrap() };
        assert_eq!(playable(&c), vec![10, 11, 11]);
        c.execute("INSERT INTO disabled_binomes VALUES (11, 1, NULL)", []).unwrap();
        assert_eq!(playable(&c), vec![10, 11]);
        c.execute("INSERT INTO disabled_binomes VALUES (10, 1, NULL)", []).unwrap();
        assert_eq!(playable(&c), vec![11]);
    }
}
