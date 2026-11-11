//! Outils hors serveur :
//! - reprise unique des données de l'ancien backend (export MySQL → SQLite) ;
//! - import de personnages en masse (port de `php artisan characters:import`).

use std::fs;
use std::io::BufRead;
use std::path::Path;

use rusqlite::types::Value as Sql;
use serde_json::Value;

use crate::clock::now_db;
use crate::db::{Db, Q};
use crate::error::{AppError, AppResult};
use crate::support::image;
use crate::support::text::slug;

/// Tables dans l'ordre des clés étrangères, avec les colonnes du nouveau schéma.
/// Les colonnes mortes de MySQL (actions.character_id, binomes.is_discovered,
/// binomes.discovered_by_player_id) et les tables Laravel techniques sont ignorées.
const TABLES: &[(&str, &[&str])] = &[
    ("players", &["id", "pseudo", "created_at", "updated_at"]),
    ("cosmos", &["id", "name", "slug", "created_at", "updated_at"]),
    ("universes", &["id", "name", "slug", "cosmos_id", "created_at", "updated_at"]),
    ("characters", &["id", "universe_id", "level_affectation", "verif_manual", "hidden", "name", "slug", "image", "forbidden_words", "created_at", "updated_at"]),
    ("rooms", &["id", "code", "is_private", "is_locked", "max_players", "game_mode", "cosmos_id", "reveal_to_eliminated", "best_question_vote", "created_by", "created_at", "updated_at"]),
    ("room_player", &["id", "room_id", "player_id", "is_ready", "created_at", "updated_at"]),
    ("games", &["id", "room_id", "status", "characters_confirmed_at", "draw_number", "rejected_character_ids", "created_at", "updated_at"]),
    ("binomes", &["id", "game_id", "universe_id", "is_orphan", "created_at", "updated_at"]),
    ("binome_player", &["id", "binome_id", "player_id", "character_id", "character_vote", "score", "is_eliminated", "is_excluded", "eliminated_round", "created_at", "updated_at"]),
    ("rounds", &["id", "game_id", "number", "current_player_id", "is_finished", "created_at", "updated_at"]),
    ("actions", &["id", "round_id", "player_id", "target_player_id", "type", "content", "character_name", "is_valid", "accusation_correct", "accusation_confirmed", "answer", "answer_corrected_from", "answer_corrected_at", "card_draw_status", "card_count", "card_draw_deadline", "drawn_word", "created_at", "updated_at"]),
    ("question_votes", &["id", "round_id", "action_id", "voter_id", "created_at", "updated_at"]),
    ("game_stats", &["id", "game_id", "player_pseudo", "character_name", "eliminations", "best_questions", "rounds_survived", "survived_full_game", "score", "is_winner", "is_eliminated", "is_excluded", "created_at", "updated_at"]),
    ("chat_messages", &["id", "player_id", "author_name", "author_key", "body", "created_at", "updated_at"]),
    ("player_avatars", &["id", "pseudo", "image", "created_at", "updated_at"]),
    ("custom_reactions", &["id", "name", "emoji", "image", "created_by_pseudo", "is_enabled", "created_at", "updated_at"]),
];

const JSON_COLUMNS: &[&str] = &["forbidden_words", "rejected_character_ids"];

/// Valeur JSON exportée par MySQL → valeur SQLite.
fn to_sql(column: &str, v: &Value) -> Sql {
    match v {
        Value::Null => Sql::Null,
        Value::Bool(b) => Sql::Integer(i64::from(*b)),
        Value::Number(n) => n.as_i64().map(Sql::Integer).unwrap_or_else(|| Sql::Real(n.as_f64().unwrap_or_default())),
        // Colonnes JSON : MySQL les exporte en tableau, SQLite les stocke en texte
        Value::Array(_) | Value::Object(_) => Sql::Text(v.to_string()),
        Value::String(s) if JSON_COLUMNS.contains(&column) => Sql::Text(s.clone()),
        // Dates « 2026-11-11 10:00:00.000000 » → précision seconde, comme TIMESTAMP
        Value::String(s) if column.ends_with("_at") || column == "card_draw_deadline" => {
            Sql::Text(s.get(..19).unwrap_or(s).replace('T', " "))
        }
        Value::String(s) => Sql::Text(s.clone()),
    }
}

/// Importe `<dossier>/<table>.ndjson` (une ligne JSON par enregistrement,
/// produit par `scripts/export-mysql.sh`) dans une base SQLite **vide**.
pub fn import_mysql_export(db: &Db, dir: &Path) -> AppResult<String> {
    db.with_writer(|conn| {
        let filled: i64 = conn.one("SELECT (SELECT COUNT(*) FROM players) + (SELECT COUNT(*) FROM characters)", [])?;
        if filled > 0 {
            return Err(AppError::Internal("la base SQLite n'est pas vide : import refusé (rien n'est écrasé)".into()));
        }
        let tx = conn.transaction()?;
        let mut report = vec![];
        for (table, columns) in TABLES {
            let path = dir.join(format!("{table}.ndjson"));
            let file = fs::File::open(&path).map_err(|e| AppError::Internal(format!("{} : {e}", path.display())))?;
            let placeholders = (1..=columns.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
            let sql = format!("INSERT INTO {table} ({}) VALUES ({placeholders})", columns.join(", "));
            let mut n = 0;
            for line in std::io::BufReader::new(file).lines() {
                let line = line.map_err(|e| AppError::Internal(format!("lecture {table} : {e}")))?;
                if line.trim().is_empty() {
                    continue;
                }
                let row: Value = serde_json::from_str(&line).map_err(|e| AppError::Internal(format!("{table} ligne {} : {e}", n + 1)))?;
                let values: Vec<Sql> = columns.iter().map(|c| to_sql(c, row.get(*c).unwrap_or(&Value::Null))).collect();
                tx.run(&sql, rusqlite::params_from_iter(values))?;
                n += 1;
            }
            report.push(format!("{table:<18} {n:>6}"));
        }
        // Contrôles : clés étrangères et taille des images (à comparer à MySQL)
        let broken: i64 = tx.one("SELECT COUNT(*) FROM pragma_foreign_key_check", [])?;
        if broken > 0 {
            return Err(AppError::Internal(format!("{broken} référence(s) cassée(s) : import annulé")));
        }
        let images: i64 = tx.one("SELECT COALESCE(SUM(LENGTH(image)), 0) FROM characters", [])?;
        tx.commit()?;
        report.push(format!("octets d'images de personnages : {images}"));
        Ok(format!("Import terminé :\n{}", report.join("\n")))
    })
}

// ─── Import de personnages (JSON) ───────────────────────────────────────────

fn image_for(images_dir: &Path, reference: Option<&str>, slug_name: &str) -> Result<Option<String>, String> {
    let encode = |path: &Path| -> Result<String, String> {
        let bytes = fs::read(path).map_err(|_| format!("Image introuvable : {}", path.display()))?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(format!("Image trop lourde (> 4 Mo) : {}", path.display()));
        }
        image::to_data_uri(&bytes).ok_or_else(|| format!("Type d'image non supporté : {}", path.display()))
    };
    match reference.map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) if r.starts_with("data:") => Ok(Some(r.to_string())),
        Some(r) if r.starts_with("http://") || r.starts_with("https://") => {
            Err(format!("Téléchargement par URL non pris en charge : enregistre l'image dans le dossier d'images ({r})"))
        }
        Some(r) => {
            let p = Path::new(r);
            let path = if p.is_file() { p.to_path_buf() } else { images_dir.join(r.trim_start_matches('/')) };
            encode(&path).map(Some)
        }
        None => {
            for ext in ["png", "jpg", "jpeg", "webp", "gif"] {
                let candidate = images_dir.join(format!("{slug_name}.{ext}"));
                if candidate.is_file() {
                    return encode(&candidate).map(Some);
                }
            }
            Ok(None)
        }
    }
}

fn str_field<'a>(row: &'a Value, key: &str, min: usize, max: usize, required: bool) -> Result<Option<&'a str>, String> {
    match row.get(key) {
        None | Some(Value::Null) if !required => Ok(None),
        Some(Value::String(s)) if (min..=max).contains(&s.trim().chars().count()) => Ok(Some(s.trim())),
        _ => Err(format!("Champ « {key} » invalide ({min} à {max} caractères).")),
    }
}

/// Import idempotent : un personnage est identifié par (univers, nom). Les créés
/// sont des propositions (`verif_manual = 0`), les refusés ne sont jamais recréés.
pub fn import_characters(db: &Db, file: &Path, images_dir: Option<&str>, skip_existing: bool) -> AppResult<String> {
    let raw = fs::read_to_string(file).map_err(|e| AppError::Internal(format!("Fichier introuvable : {} ({e})", file.display())))?;
    let rows: Vec<Value> = serde_json::from_str(&raw).map_err(|_| AppError::Internal("Le fichier JSON doit contenir un tableau de personnages.".into()))?;
    let default_dir = file.parent().unwrap_or(Path::new(".")).join("images");
    let images_dir = images_dir.map(Path::new).unwrap_or(&default_dir).to_path_buf();

    db.with_writer(|conn| {
        let (mut created, mut updated, mut skipped, mut errors) = (0, 0, 0, vec![]);
        for (index, row) in rows.iter().enumerate() {
            let tx = conn.transaction()?;
            let result: Result<&str, String> = (|| {
                let name = str_field(row, "name", 2, 50, true)?.expect("requis");
                let universe = str_field(row, "universe", 2, 50, true)?.expect("requis");
                let cosmos = str_field(row, "cosmos", 2, 50, false)?;
                let words: Vec<String> = row
                    .get("forbidden_words")
                    .and_then(Value::as_array)
                    .filter(|a| a.len() == 3)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::trim).filter(|w| (1..=50).contains(&w.chars().count())).map(String::from).collect())
                    .filter(|w: &Vec<String>| w.len() == 3)
                    .ok_or("Il faut exactement 3 mots interdits (1 à 50 caractères).")?;
                let level = row.get("level_affectation").and_then(Value::as_i64).filter(|l| *l >= 1).ok_or("level_affectation doit être un entier ≥ 1.")?;
                let db_err = |e: AppError| format!("{e:?}");
                let now = now_db();

                let cosmos_id = match cosmos {
                    Some(c) => Some(match tx.opt("SELECT id FROM cosmos WHERE slug = ?1", [slug(c)], |r| r.get::<_, i64>(0)).map_err(db_err)? {
                        Some(id) => id,
                        None => tx.insert("INSERT INTO cosmos (name, slug, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)", (c, slug(c), &now)).map_err(db_err)?,
                    }),
                    None => None,
                };
                let universe_id = match tx.opt("SELECT id, cosmos_id FROM universes WHERE slug = ?1", [slug(universe)], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))).map_err(db_err)? {
                    Some((id, current)) => {
                        if cosmos_id.is_some() && current != cosmos_id {
                            tx.run("UPDATE universes SET cosmos_id = ?1, updated_at = ?2 WHERE id = ?3", (cosmos_id, &now, id)).map_err(db_err)?;
                        }
                        id
                    }
                    None => tx
                        .insert("INSERT INTO universes (name, slug, cosmos_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)", (universe, slug(universe), cosmos_id, &now))
                        .map_err(db_err)?,
                };
                let existing = tx
                    .opt("SELECT id, hidden, image FROM characters WHERE universe_id = ?1 AND name = ?2", (universe_id, name), |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?, r.get::<_, Option<String>>(2)?))
                    })
                    .map_err(db_err)?;
                if existing.as_ref().is_some_and(|e| e.1) {
                    return Ok("skipped");
                }
                if existing.is_some() && skip_existing {
                    return Ok("skipped");
                }
                let same_level: i64 = tx
                    .one("SELECT COUNT(*) FROM characters WHERE universe_id = ?1 AND level_affectation = ?2 AND id != ?3", (universe_id, level, existing.as_ref().map_or(0, |e| e.0)))
                    .map_err(db_err)?;
                if same_level >= 2 {
                    return Err(format!("L'univers « {universe} » a déjà 2 personnages au niveau d'affectation {level}."));
                }
                let img = image_for(&images_dir, row.get("image").and_then(Value::as_str), &slug(name))?.or(existing.as_ref().and_then(|e| e.2.clone()));
                let words = serde_json::to_string(&words).expect("json");
                if let Some((id, _, _)) = existing {
                    tx.run("UPDATE characters SET forbidden_words = ?1, level_affectation = ?2, image = ?3, updated_at = ?4 WHERE id = ?5", (&words, level, &img, &now, id)).map_err(db_err)?;
                    Ok("updated")
                } else {
                    let base = slug(name);
                    let mut s = base.clone();
                    let mut i = 2;
                    while tx.exists("SELECT 1 FROM characters WHERE universe_id = ?1 AND slug = ?2", (universe_id, &s)).map_err(db_err)? {
                        s = format!("{base}-{i}");
                        i += 1;
                    }
                    tx.run(
                        "INSERT INTO characters (universe_id, name, slug, forbidden_words, level_affectation, image, verif_manual, hidden, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, ?7)",
                        rusqlite::params![universe_id, name, s, words, level, img, now],
                    )
                    .map_err(db_err)?;
                    Ok("created")
                }
            })();
            match result {
                Ok(kind) => {
                    tx.commit()?;
                    match kind {
                        "created" => created += 1,
                        "updated" => updated += 1,
                        _ => skipped += 1,
                    }
                }
                Err(message) => {
                    let name = row.get("name").and_then(Value::as_str).unwrap_or("?");
                    errors.push(format!("  #{index} {name} : {message}"));
                }
            }
        }
        let mut report = format!("{created} créé(s), {updated} mis à jour, {skipped} ignoré(s), {} erreur(s)", errors.len());
        if !errors.is_empty() {
            report.push('\n');
            report.push_str(&errors.join("\n"));
        }
        Ok(report)
    })
}
