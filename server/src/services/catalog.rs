//! Catalogue : accès admin, cosmos, univers, personnages et validation des
//! propositions. Images : data URI en base, servies en binaire par URL versionnée.

use hmac::{Hmac, KeyInit, Mac};
use rusqlite::Transaction;
use serde_json::{json, Value};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::clock::now_db;
use crate::db::{is_constraint, Q};
use crate::error::{AppError, AppResult};
use crate::http::input::{Input, V};
use crate::services::common::*;
use crate::services::Out;
use crate::support::image;
use crate::support::text::slug;

// ─── Accès ──────────────────────────────────────────────────────────────────

/// Jeton admin stable, dérivé de l'empreinte configurée
/// (`BINOME_ADMIN_PASSWORD_HASH`) : le serveur ne connaît pas le mot de passe.
/// Changer de mot de passe change l'empreinte, donc invalide les jetons émis.
pub fn access_token(password_hash: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(password_hash.trim().as_bytes()).expect("clé HMAC");
    mac.update(b"binome-characters-access");
    hex::encode(mac.finalize().into_bytes())
}

/// Contrôle de l'en-tête `X-Characters-Access` (jetons de même longueur :
/// la comparaison à temps constant ne sort pas tôt).
pub fn check_access(password_hash: Option<&str>, provided: Option<&str>) -> AppResult<()> {
    let Some(password_hash) = password_hash else {
        return Err(AppError::forbidden("L'accès à la gestion des personnages n'est pas configuré."));
    };
    let expected = access_token(password_hash);
    match provided {
        Some(p) if bool::from(p.as_bytes().ct_eq(expected.as_bytes())) => Ok(()),
        _ => Err(AppError::forbidden("Mot de passe requis pour accéder à la gestion des personnages.")),
    }
}

/// POST /access/characters — validation du corps, avant toute limite de débit.
pub fn login_input(input: &Input) -> AppResult<String> {
    let mut v = V::new(input);
    let given = v.req_str("password", None, None);
    v.finish()?;
    Ok(given.expect("validé"))
}

/// POST /access/characters — vérification PBKDF2 (coûteuse : à lancer hors
/// des threads réseau). Sans empreinte configurée, aucun mot de passe n'ouvre.
pub fn login(password_hash: Option<&str>, given: &str) -> AppResult<Out> {
    match password_hash {
        Some(h) if crate::support::password::verify(given, h) => Ok(Out::ok(json!({ "token": access_token(h) }))),
        _ => Err(AppError::unprocessable("Mot de passe incorrect.")),
    }
}

// ─── Cosmos / univers ───────────────────────────────────────────────────────

const PLAYABLE_UNIVERSES: &str = "SELECT universe_id FROM characters WHERE verif_manual = 1 AND hidden = 0
     GROUP BY universe_id, level_affectation HAVING COUNT(*) >= 2";

pub fn cosmos_index(c: &impl Q) -> AppResult<Out> {
    let list = c.all(
        &format!(
            "SELECT co.id, co.name,
                    (SELECT COUNT(*) FROM universes u WHERE u.cosmos_id = co.id),
                    (SELECT COUNT(*) FROM universes u WHERE u.cosmos_id = co.id AND u.id IN ({PLAYABLE_UNIVERSES}))
             FROM cosmos co ORDER BY co.name"
        ),
        [],
        |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "universes_count": r.get::<_, i64>(2)?, "playable_universe_count": r.get::<_, i64>(3)? })),
    )?;
    Ok(Out::ok(json!({ "cosmos": list })))
}

fn unique_slug(tx: &Transaction, table: &str, base: &str, extra: &str, params: &[i64]) -> AppResult<String> {
    let original = slug(base);
    let mut candidate = original.clone();
    let mut i = 2;
    loop {
        let sql = format!("SELECT 1 FROM {table} WHERE slug = ?1 {extra}");
        let mut p: Vec<rusqlite::types::Value> = vec![candidate.clone().into()];
        p.extend(params.iter().map(|x| rusqlite::types::Value::from(*x)));
        if !tx.exists(&sql, rusqlite::params_from_iter(p))? {
            return Ok(candidate);
        }
        candidate = format!("{original}-{i}");
        i += 1;
    }
}

pub fn cosmos_store(tx: &Transaction, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input).messages(&[
        ("name.required", "Le nom du cosmos est requis."),
        ("name.min", "Le nom du cosmos doit faire au moins 2 caractères."),
        ("name.max", "Le nom du cosmos ne peut pas dépasser 50 caractères."),
        ("name.unique", "Un cosmos avec ce nom existe déjà."),
    ]);
    let name = v.req_str("name", Some(2), Some(50));
    if let Some(n) = &name {
        if tx.exists("SELECT 1 FROM cosmos WHERE name = ?1", [n])? {
            v.fail("name", "unique", "The name has already been taken.".into());
        }
    }
    v.finish()?;
    let name = name.expect("validé");
    let s = unique_slug(tx, "cosmos", &name, "", &[])?;
    let now = now_db();
    let id = tx.insert("INSERT INTO cosmos (name, slug, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)", (&name, &s, &now))?;
    Ok(Out::created(json!({ "message": "Cosmos créé avec succès.", "cosmos": { "id": id, "name": name } })))
}

fn universe_json(c: &impl Q, id: i64) -> AppResult<Value> {
    Ok(c.opt(
        "SELECT u.id, u.name, u.cosmos_id, co.name FROM universes u LEFT JOIN cosmos co ON co.id = u.cosmos_id WHERE u.id = ?1",
        [id],
        |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "cosmos_id": r.get::<_, Option<i64>>(2)?, "cosmos_name": r.get::<_, Option<String>>(3)? })),
    )?
    .unwrap_or(Value::Null))
}

pub fn universes_index(c: &impl Q) -> AppResult<Out> {
    let list = c.all(
        "SELECT u.id, u.name, u.cosmos_id, co.name, (SELECT COUNT(*) FROM characters ch WHERE ch.universe_id = u.id)
         FROM universes u LEFT JOIN cosmos co ON co.id = u.cosmos_id ORDER BY u.name",
        [],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "cosmos_id": r.get::<_, Option<i64>>(2)?,
                "cosmos_name": r.get::<_, Option<String>>(3)?,
                "characters_count": r.get::<_, i64>(4)?,
            }))
        },
    )?;
    Ok(Out::ok(json!({ "universes": list })))
}

fn cosmos_field(v: &mut V, tx: &Transaction) -> AppResult<Option<i64>> {
    let id = v.opt_int("cosmos_id");
    v.exists(tx, "cosmos_id", id, "SELECT 1 FROM cosmos WHERE id = ?1")
}

pub fn universes_store(tx: &Transaction, input: &Input) -> AppResult<Out> {
    let mut v = V::new(input).messages(&[
        ("name.required", "Le nom de l'univers est requis."),
        ("name.min", "Le nom de l'univers doit faire au moins 2 caractères."),
        ("name.max", "Le nom de l'univers ne peut pas dépasser 50 caractères."),
        ("name.unique", "Un univers avec ce nom existe déjà."),
        ("cosmos_id.exists", "Le cosmos sélectionné n'existe pas."),
    ]);
    let name = v.req_str("name", Some(2), Some(50));
    if let Some(n) = &name {
        if tx.exists("SELECT 1 FROM universes WHERE name = ?1", [n])? {
            v.fail("name", "unique", "The name has already been taken.".into());
        }
    }
    let cosmos_id = cosmos_field(&mut v, tx)?;
    v.finish()?;
    let name = name.expect("validé");
    let s = unique_slug(tx, "universes", &name, "", &[])?;
    let now = now_db();
    let id = tx.insert(
        "INSERT INTO universes (name, slug, cosmos_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        (&name, &s, cosmos_id, &now),
    )?;
    Ok(Out::created(json!({ "message": "Univers créé avec succès.", "universe": universe_json(tx, id)? })))
}

pub fn universes_update(tx: &Transaction, universe_id: i64, input: &Input) -> AppResult<Out> {
    if !tx.exists("SELECT 1 FROM universes WHERE id = ?1", [universe_id])? {
        return Err(AppError::NotFound);
    }
    let mut v = V::new(input).messages(&[("cosmos_id.exists", "Le cosmos sélectionné n'existe pas.")]);
    let cosmos_id = cosmos_field(&mut v, tx)?;
    v.finish()?;
    tx.run("UPDATE universes SET cosmos_id = ?1, updated_at = ?2 WHERE id = ?3", (cosmos_id, now_db(), universe_id))?;
    Ok(Out::ok(json!({ "message": "Univers mis à jour avec succès.", "universe": universe_json(tx, universe_id)? })))
}

// ─── Personnages ────────────────────────────────────────────────────────────

const LIST_COLS: &str = "ch.id, ch.universe_id, u.name, u.cosmos_id, co.name, ch.name, (ch.image IS NOT NULL AND ch.image <> ''),
    ch.updated_at, ch.forbidden_words, ch.level_affectation, ch.verif_manual, ch.hidden";

fn list_row(env: &Env, r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let words: String = r.get(8)?;
    let id: i64 = r.get(0)?;
    let updated: Option<String> = r.get(7)?;
    Ok(json!({
        "id": id,
        "universe_id": r.get::<_, i64>(1)?,
        "universe_name": r.get::<_, String>(2)?,
        "cosmos_id": r.get::<_, Option<i64>>(3)?,
        "cosmos_name": r.get::<_, Option<String>>(4)?,
        "name": r.get::<_, String>(5)?,
        "image": character_image_url(env, id, r.get(6)?, updated.as_deref()),
        "forbidden_words": serde_json::from_str::<Value>(&words).unwrap_or(json!([])),
        "level_affectation": r.get::<_, i64>(9)?,
        "verif_manual": r.get::<_, bool>(10)?,
        "hidden": r.get::<_, bool>(11)?,
    }))
}

/// GET /characters — tout le catalogue (refusés compris : le front les filtre), sans data URI.
pub fn characters_index(c: &impl Q, env: &Env) -> AppResult<Out> {
    let list = c.all(
        &format!(
            "SELECT {LIST_COLS} FROM characters ch JOIN universes u ON u.id = ch.universe_id LEFT JOIN cosmos co ON co.id = u.cosmos_id
             ORDER BY ch.name, ch.id"
        ),
        [],
        |r| list_row(env, r),
    )?;
    Ok(Out::ok(json!({ "characters": list })))
}

/// GET /characters/pending — binômes à valider, groupés par univers + niveau.
pub fn characters_pending(c: &impl Q, env: &Env) -> AppResult<Out> {
    let rows = c.all(
        &format!(
            "SELECT {LIST_COLS} FROM characters ch JOIN universes u ON u.id = ch.universe_id LEFT JOIN cosmos co ON co.id = u.cosmos_id
             WHERE ch.verif_manual = 0 AND ch.hidden = 0 ORDER BY ch.universe_id, ch.level_affectation, ch.id"
        ),
        [],
        |r| list_row(env, r),
    )?;
    let mut groups: Vec<(String, Value, Vec<Value>)> = vec![];
    for row in rows {
        let key = format!("{}-{}", row["universe_id"], row["level_affectation"]);
        let character = json!({ "id": row["id"], "name": row["name"], "forbidden_words": row["forbidden_words"], "image": row["image"] });
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.2.push(character),
            None => groups.push((key, row, vec![character])),
        }
    }
    let binomes: Vec<Value> = groups
        .into_iter()
        .map(|(key, first, characters)| {
            json!({
                "key": key,
                "universe_id": first["universe_id"],
                "universe_name": first["universe_name"],
                "cosmos_name": first["cosmos_name"],
                "level_affectation": first["level_affectation"],
                "characters": characters,
            })
        })
        .collect();
    Ok(Out::ok(json!({ "binomes": binomes })))
}

fn ids_field(input: &Input) -> AppResult<Vec<i64>> {
    let mut v = V::new(input);
    let ids = v.req_int_array("character_ids");
    v.finish()?;
    Ok(ids.expect("validé"))
}

fn ids_where(tx: &Transaction, ids: &[i64], condition: &str) -> AppResult<Vec<i64>> {
    tx.all(
        &format!("SELECT id FROM characters WHERE {condition} AND id IN (SELECT value FROM json_each(?1)) ORDER BY id"),
        [serde_json::to_string(ids).expect("json")],
        |r| r.get(0),
    )
}

pub fn accept(tx: &Transaction, input: &Input) -> AppResult<Out> {
    let ids = ids_where(tx, &ids_field(input)?, "verif_manual = 0 AND hidden = 0")?;
    if ids.is_empty() {
        return Err(AppError::unprocessable("Aucun personnage en attente pour ces identifiants."));
    }
    for id in &ids {
        let (name, has_image): (String, bool) = tx.opt("SELECT name, (image IS NOT NULL AND image <> '') FROM characters WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?.expect("existe");
        if !has_image {
            return Err(AppError::unprocessable(format!("Chaque personnage doit avoir une image avant validation (manquante : « {name} »).")));
        }
    }
    let now = now_db();
    for id in &ids {
        tx.run("UPDATE characters SET verif_manual = 1, updated_at = ?1 WHERE id = ?2", (&now, id))?;
    }
    Ok(Out::ok(json!({ "message": "Binôme validé.", "validated_ids": ids })))
}

pub fn reject(tx: &Transaction, input: &Input) -> AppResult<Out> {
    let ids = ids_where(tx, &ids_field(input)?, "verif_manual = 0 AND hidden = 0")?;
    if ids.is_empty() {
        return Err(AppError::unprocessable("Aucun personnage en attente pour ces identifiants."));
    }
    let now = now_db();
    for id in &ids {
        tx.run("UPDATE characters SET hidden = 1, updated_at = ?1 WHERE id = ?2", (&now, id))?;
    }
    Ok(Out::ok(json!({ "message": "Binôme refusé.", "rejected_ids": ids })))
}

pub fn restore(tx: &Transaction, input: &Input) -> AppResult<Out> {
    let ids = ids_where(tx, &ids_field(input)?, "hidden = 1")?;
    if ids.is_empty() {
        return Err(AppError::unprocessable("Aucun personnage refusé pour ces identifiants."));
    }
    let now = now_db();
    for id in &ids {
        tx.run("UPDATE characters SET hidden = 0, verif_manual = 0, updated_at = ?1 WHERE id = ?2", (&now, id))?;
    }
    Ok(Out::ok(json!({ "message": "Binôme remis en validation.", "restored_ids": ids })))
}

/// Règles Laravel `image|mimes:jpeg,jpg,png,gif,webp|max:4096`.
fn check_upload(v: &mut V, bytes: Option<&Vec<u8>>, required: bool) -> Option<String> {
    let Some(bytes) = bytes else {
        if required {
            v.fail("image", "required", "The image field is required.".into());
        }
        return None;
    };
    match image::to_data_uri(bytes) {
        None => {
            v.fail("image", "image", "The image field must be an image.".into());
            None
        }
        Some(_) if bytes.len() > 4096 * 1024 => {
            v.fail("image", "max", "The image field must not be greater than 4096 kilobytes.".into());
            None
        }
        Some(uri) => Some(uri),
    }
}

fn character_json(c: &impl Q, env: &Env, id: i64, with_status: bool) -> AppResult<Value> {
    let ch = character(c, id)?.ok_or(AppError::NotFound)?;
    let mut v = json!({
        "id": ch.id,
        "universe_id": ch.universe_id,
        "name": ch.name,
        "image": character_image_url(env, ch.id, ch.has_image, ch.updated_at.as_deref()),
        "forbidden_words": ch.forbidden_words,
        "level_affectation": ch.level_affectation,
    });
    if with_status {
        v["verif_manual"] = json!(ch.verif_manual);
        v["hidden"] = json!(ch.hidden);
    }
    Ok(v)
}

struct CharacterForm {
    name: String,
    universe_id: Option<i64>,
    image: Option<String>,
    words: Vec<String>,
    level: i64,
    status: Option<String>,
}

fn character_form(tx: &Transaction, input: &Input, file: Option<&Vec<u8>>) -> AppResult<CharacterForm> {
    let mut v = V::new(input).messages(&[
        ("name.required", "Le nom du personnage est requis."),
        ("name.min", "Le nom du personnage doit faire au moins 2 caractères."),
        ("name.max", "Le nom du personnage ne peut pas dépasser 50 caractères."),
        ("universe_id.exists", "L'univers sélectionné n'existe pas."),
    ]);
    let name = v.req_str("name", Some(2), Some(50));
    let universe_id = v.opt_int("universe_id");
    let universe_id = v.exists(tx, "universe_id", universe_id, "SELECT 1 FROM universes WHERE id = ?1")?;
    let image = check_upload(&mut v, file, false);
    let words = v.req_str_array("forbidden_words", Some(3), 50);
    let level = v.req_int("level_affectation");
    v.min_int("level_affectation", level, 1);
    let status = v.opt_str("status", None, None);
    v.in_list("status", status.as_ref(), &["validated", "pending", "rejected"]);
    v.finish()?;
    Ok(CharacterForm { name: name.expect("validé"), universe_id, image, words: words.expect("validé"), level: level.expect("validé"), status })
}

/// POST /universes/{universe}/characters (multipart) — créé validé.
pub fn character_store(tx: &Transaction, env: &Env, universe_id: i64, input: &Input, file: Option<Vec<u8>>) -> AppResult<Out> {
    if !tx.exists("SELECT 1 FROM universes WHERE id = ?1", [universe_id])? {
        return Err(AppError::NotFound);
    }
    let f = character_form(tx, input, file.as_ref())?;
    let s = unique_slug(tx, "characters", &f.name, "AND universe_id = ?2", &[universe_id])?;
    let now = now_db();
    let id = tx.insert(
        "INSERT INTO characters (universe_id, name, slug, image, forbidden_words, level_affectation, verif_manual, hidden, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7, ?7)",
        rusqlite::params![universe_id, f.name, s, f.image, serde_json::to_string(&f.words).expect("json"), f.level, now],
    )?;
    Ok(Out::created(json!({ "message": "Personnage créé avec succès.", "character": character_json(tx, env, id, false)? })))
}

/// PUT /characters/{character} (ou POST + `_method=PUT`, forme du front).
pub fn character_update(tx: &Transaction, env: &Env, id: i64, input: &Input, file: Option<Vec<u8>>) -> AppResult<Out> {
    let current = character(tx, id)?.ok_or(AppError::NotFound)?;
    let f = character_form(tx, input, file.as_ref())?;
    let universe_id = f.universe_id.unwrap_or(current.universe_id);
    let slug_value: Option<String> = if f.name != current.name || universe_id != current.universe_id {
        Some(unique_slug(tx, "characters", &f.name, "AND universe_id = ?2 AND id != ?3", &[universe_id, id])?)
    } else {
        None
    };
    let has_image = f.image.is_some() || current.has_image;
    let (mut verif, mut hidden) = (current.verif_manual, current.hidden);
    if let Some(status) = &f.status {
        if status == "validated" && !has_image {
            return Err(AppError::unprocessable("Un personnage validé doit avoir une image."));
        }
        verif = status == "validated";
        hidden = status == "rejected";
    }
    tx.run(
        "UPDATE characters SET universe_id = ?1, name = ?2, slug = COALESCE(?3, slug), image = COALESCE(?4, image), forbidden_words = ?5,
             level_affectation = ?6, verif_manual = ?7, hidden = ?8, updated_at = ?9 WHERE id = ?10",
        rusqlite::params![universe_id, f.name, slug_value, f.image, serde_json::to_string(&f.words).expect("json"), f.level, verif, hidden, now_db(), id],
    )?;
    Ok(Out::ok(json!({ "message": "Personnage modifié avec succès.", "character": character_json(tx, env, id, true)? })))
}

/// PATCH /characters/{character}/forbidden-words
pub fn forbidden_words(tx: &Transaction, id: i64, input: &Input) -> AppResult<Out> {
    if character(tx, id)?.is_none() {
        return Err(AppError::NotFound);
    }
    let mut v = V::new(input);
    let words = v.req_str_array("forbidden_words", Some(3), 50);
    v.finish()?;
    let words = words.expect("validé");
    tx.run("UPDATE characters SET forbidden_words = ?1, updated_at = ?2 WHERE id = ?3", (serde_json::to_string(&words).expect("json"), now_db(), id))?;
    Ok(Out::ok(json!({ "message": "Mots interdits mis à jour.", "character": { "id": id, "forbidden_words": words } })))
}

/// POST /characters/{character}/image (multipart)
pub fn upload_image(tx: &Transaction, env: &Env, id: i64, input: &Input, file: Option<Vec<u8>>) -> AppResult<Out> {
    if character(tx, id)?.is_none() {
        return Err(AppError::NotFound);
    }
    let mut v = V::new(input);
    let uri = check_upload(&mut v, file.as_ref(), true);
    v.finish()?;
    tx.run("UPDATE characters SET image = ?1, updated_at = ?2 WHERE id = ?3", (uri.expect("validé"), now_db(), id))?;
    let ch = character(tx, id)?.expect("existe");
    Ok(Out::ok(json!({
        "message": "Image importée.",
        "character": { "id": id, "image": character_image_url(env, id, ch.has_image, ch.updated_at.as_deref()) },
    })))
}

/// DELETE /characters/{character} — refusé s'il a déjà été joué.
pub fn character_destroy(tx: &Transaction, id: i64) -> AppResult<Out> {
    if character(tx, id)?.is_none() {
        return Err(AppError::NotFound);
    }
    match tx.run("DELETE FROM characters WHERE id = ?1", [id]) {
        Ok(_) => Ok(Out::message("Personnage supprimé avec succès.")),
        Err(e) if is_constraint(&e) => Err(AppError::rule("Ce personnage a déjà été utilisé dans une partie et ne peut pas être supprimé.")),
        Err(e) => Err(e),
    }
}

/// GET /character-images/{character} → (type, octets)
pub fn character_image(c: &impl Q, id: i64) -> AppResult<(String, Vec<u8>)> {
    let data: Option<String> = c.opt("SELECT image FROM characters WHERE id = ?1", [id], |r| r.get(0))?.flatten();
    data.as_deref().and_then(image::decode_data_uri).ok_or(AppError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::password;

    #[test]
    fn admin_login_with_hash() {
        let h = password::hash("mot de passe admin", 1000);
        let token = access_token(&h);
        assert_eq!(token.len(), 64);
        assert!(login(Some(&h), "mot de passe admin").is_ok());
        assert!(login(Some(&h), "mot de passe Admin").is_err());
        assert!(login(Some(&h), "x").is_err()); // longueur différente
        assert!(login(None, "mot de passe admin").is_err());
        assert!(check_access(Some(&h), Some(&token)).is_ok());
        assert!(check_access(Some(&h), Some("abc")).is_err());
        assert!(check_access(Some(&h), None).is_err());
        assert!(check_access(None, Some(&token)).is_err());
        // Nouvelle empreinte (même mot de passe, autre sel) → anciens jetons refusés
        let h2 = password::hash("mot de passe admin", 1000);
        assert!(check_access(Some(&h2), Some(&token)).is_err());
    }
}
