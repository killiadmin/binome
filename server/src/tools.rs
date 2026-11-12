//! Outils en ligne de commande pour le catalogue de personnages (flux décrit dans
//! docs/generation-personnages.md et la page admin « Prompts ») :
//! - `import-characters` : import de personnages en masse (propositions à valider) ;
//! - `list-characters`   : tout le catalogue, refusés compris (avant de générer un lot) ;
//! - `count-pending`     : nombre de binômes en attente de validation.
//!
//! Et `prune-rooms` : nettoyage des adhésions de salon mortes.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::clock::now_db;
use crate::db::{Db, Q};
use crate::error::{AppError, AppResult};
use crate::support::image;
use crate::support::text::slug;

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

/// `Univers / Nom`, suivi de « [refusé] » ou « [à valider] », trié par univers puis nom.
pub fn list_characters(db: &Db) -> AppResult<String> {
    db.with_writer(|conn| {
        let rows = conn.all(
            "SELECT u.name, c.name, c.hidden, c.verif_manual FROM characters c JOIN universes u ON u.id = c.universe_id
             ORDER BY u.name, c.name",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?, r.get::<_, bool>(3)?)),
        )?;
        Ok(rows
            .iter()
            .map(|(u, n, hidden, verified)| {
                let state = if *hidden { " [refusé]" } else if !verified { " [à valider]" } else { "" };
                format!("{u} / {n}{state}")
            })
            .collect::<Vec<_>>()
            .join("\n"))
    })
}

/// Binômes proposés en attente de validation (groupes univers + niveau).
pub fn count_pending(db: &Db) -> AppResult<String> {
    db.with_writer(|conn| {
        let n: i64 = conn.one(
            "SELECT COUNT(*) FROM (SELECT 1 FROM characters WHERE verif_manual = 0 AND hidden = 0
             GROUP BY universe_id, level_affectation)",
            [],
        )?;
        Ok(n.to_string())
    })
}

// ─── Nettoyage des adhésions mortes ─────────────────────────────────────────

/// `prune-rooms [--apply]` : adhésions mortes (salon d'une partie terminée, hall
/// abandonné — règle `profile::LIVE_MEMBERSHIP`) et salons vides sans partie en
/// cours. Sans `--apply`, simple liste. Avec : suppression en une transaction,
/// comme des départs (salon vidé supprimé — ses parties restent dans
/// l'historique avec leur code de salon, migration 4 ; sinon l'hôte passe au plus
/// ancien restant).
pub fn prune_rooms(db: &Db, apply: bool) -> AppResult<String> {
    use crate::services::profile::{lobby_cutoff, LIVE_MEMBERSHIP, LOBBY_TTL_HOURS};
    db.with_writer(|conn| {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let dead: Vec<(i64, i64, String, String)> = tx.all(
            &format!(
                "SELECT rp.id, r.id, r.code, p.pseudo FROM room_player rp
                 JOIN rooms r ON r.id = rp.room_id JOIN players p ON p.id = rp.player_id
                 WHERE NOT {LIVE_MEMBERSHIP} ORDER BY r.id, rp.id"
            ),
            rusqlite::named_params! { ":since": lobby_cutoff() },
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let empty: Vec<(i64, String)> = tx.all(
            "SELECT r.id, r.code FROM rooms r
             WHERE NOT EXISTS (SELECT 1 FROM room_player rp WHERE rp.room_id = r.id)
               AND NOT EXISTS (SELECT 1 FROM games g WHERE g.room_id = r.id AND g.status = 'in_progress')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        // Regroupement par salon, dans l'ordre
        let mut rooms: Vec<(i64, String, Vec<String>)> = vec![];
        for (_, room_id, code, pseudo) in &dead {
            match rooms.last_mut() {
                Some((id, _, names)) if id == room_id => names.push(pseudo.clone()),
                _ => rooms.push((*room_id, code.clone(), vec![pseudo.clone()])),
            }
        }
        let mut lines = vec![format!(
            "Adhésions mortes : {} dans {} salon(s) (partie terminée depuis l'arrivée, ou hall inactif depuis plus de {LOBBY_TTL_HOURS} h)",
            dead.len(),
            rooms.len()
        )];
        let mut deleted_rooms = 0;
        let mut orphaned_games = 0;
        for (room_id, code, names) in &rooms {
            let total: i64 = tx.one("SELECT COUNT(*) FROM room_player WHERE room_id = ?1", [room_id])?;
            let games: i64 = tx.one("SELECT COUNT(*) FROM games WHERE room_id = ?1", [room_id])?;
            let emptied = total as usize == names.len();
            let fate = if emptied {
                deleted_rooms += 1;
                orphaned_games += games;
                format!("salon supprimé ({games} partie(s) gardée(s) dans l'historique)")
            } else {
                format!("{} joueur(s) restent", total as usize - names.len())
            };
            lines.push(format!("  {code} (#{room_id}) : {} → {fate}", names.join(", ")));
        }
        for (room_id, code) in &empty {
            let games: i64 = tx.one("SELECT COUNT(*) FROM games WHERE room_id = ?1", [room_id])?;
            deleted_rooms += 1;
            orphaned_games += games;
            lines.push(format!("  {code} (#{room_id}) : déjà vide → salon supprimé ({games} partie(s) gardée(s) dans l'historique)"));
        }

        if !apply {
            lines.push(format!(
                "Essai à blanc : rien n'est modifié. Relancer avec --apply pour supprimer ({deleted_rooms} salon(s) ; {orphaned_games} partie(s) gardée(s) avec leur code)."
            ));
            return Ok(lines.join("\n"));
        }
        let now = now_db();
        for (rp_id, ..) in &dead {
            tx.run("DELETE FROM room_player WHERE id = ?1", [rp_id])?;
        }
        for room_id in rooms.iter().map(|r| r.0).chain(empty.iter().map(|r| r.0)) {
            let left: Option<i64> = tx.opt("SELECT player_id FROM room_player WHERE room_id = ?1 ORDER BY id LIMIT 1", [room_id], |r| r.get(0))?;
            match left {
                None => {
                    tx.run("DELETE FROM rooms WHERE id = ?1", [room_id])?;
                }
                Some(oldest) => {
                    // L'hôte faisait partie des adhésions mortes : il passe au plus ancien restant
                    tx.run(
                        "UPDATE rooms SET created_by = ?1, updated_at = ?2 WHERE id = ?3
                         AND created_by NOT IN (SELECT player_id FROM room_player WHERE room_id = ?3)",
                        rusqlite::params![oldest, now, room_id],
                    )?;
                }
            }
        }
        let broken: i64 = tx.one("SELECT COUNT(*) FROM pragma_foreign_key_check", [])?;
        if broken > 0 {
            return Err(AppError::Internal(format!("{broken} référence(s) cassée(s) : nettoyage annulé")));
        }
        tx.commit()?;
        lines.push(format!("Supprimé : {} adhésion(s), {deleted_rooms} salon(s).", dead.len()));
        Ok(lines.join("\n"))
    })
}
