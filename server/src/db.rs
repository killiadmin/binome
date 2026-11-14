//! Accès SQLite.
//!
//! - **Une connexion d'écriture** : chaque écriture est une transaction
//!   `BEGIN IMMEDIATE`, donc les écritures passent l'une après l'autre. C'est
//!   plus fort que les `lockForUpdate` de l'ancien backend (deux votes, deux
//!   réponses ou un double clic ne peuvent jamais se croiser), et sans coût à
//!   l'échelle de Binome (une écriture dure quelques dizaines de µs).
//! - **Quelques connexions de lecture** (mode WAL : elles lisent pendant une écriture).
//! - Tout le travail SQLite (bloquant) passe par `spawn_blocking` pour ne
//!   jamais geler les tâches réseau.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};

use crate::error::{AppError, AppResult};
use crate::support::text::ci_key;

const SCHEMA: &str = include_str!("../migrations/0001_schema.sql");

/// Migrations appliquées après le schéma initial (version = `PRAGMA user_version`).
/// Une base créée par une version antérieure à ce mécanisme est en version 1.
const MIGRATIONS: &[(i64, &str)] = &[
    (2, include_str!("../migrations/0002_parties_conservees.sql")),
    (3, include_str!("../migrations/0003_profils.sql")),
    (4, include_str!("../migrations/0004_code_salon_des_parties.sql")),
    (5, include_str!("../migrations/0005_changements_personnage.sql")),
    (6, include_str!("../migrations/0006_controles_admin.sql")),
    (7, include_str!("../migrations/0007_administrateurs.sql")),
    (8, include_str!("../migrations/0008_boite_a_idees.sql")),
    (9, include_str!("../migrations/0009_effacement.sql")),
    (10, include_str!("../migrations/0010_fiches_univers.sql")),
];
const READERS: usize = 4;

#[derive(Clone)]
pub struct Db {
    writer: Arc<Mutex<Connection>>,
    readers: Arc<Vec<Mutex<Connection>>>,
    next: Arc<AtomicUsize>,
}

/// Ouvre une connexion configurée (collation, clés étrangères, WAL).
fn open_connection(path: &Path, flags: OpenFlags) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(path, flags)?;
    conn.create_collation("BINOME_CI", |a, b| ci_key(a).cmp(&ci_key(b)))?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;",
    )?;
    Ok(conn)
}

impl Db {
    /// Ouvre (et crée au besoin) la base, applique le schéma s'il est absent.
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| AppError::Internal(format!("dossier de la base : {e}")))?;
        }
        let rw = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut writer = open_connection(path, rw)?;
        writer.execute_batch("PRAGMA journal_mode = WAL;")?;

        let has_schema: bool = writer.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'players')",
            [],
            |r| r.get(0),
        )?;
        if !has_schema {
            writer.execute_batch(SCHEMA)?;
            writer.pragma_update(None, "user_version", 1)?;
            tracing::info!("schéma créé dans {}", path.display());
        }
        migrate(&mut writer)?;
        ensure_secrets(&writer)?;

        let ro = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let readers = (0..READERS)
            .map(|_| open_connection(path, ro).map(Mutex::new))
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            readers: Arc::new(readers),
            next: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Transaction d'écriture : validée si `f` réussit, annulée sinon.
    pub async fn write<T, F>(&self, f: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction) -> AppResult<T> + Send + 'static,
    {
        let writer = self.writer.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = writer.lock().unwrap_or_else(|p| p.into_inner());
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let out = f(&tx)?;
            tx.commit()?;
            Ok(out)
        })
        .await
        .map_err(|e| AppError::Internal(format!("tâche d'écriture : {e}")))?
    }

    /// Lecture seule.
    pub async fn read<T, F>(&self, f: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> AppResult<T> + Send + 'static,
    {
        let readers = self.readers.clone();
        let i = self.next.fetch_add(1, Ordering::Relaxed) % readers.len();
        tokio::task::spawn_blocking(move || {
            let conn = readers[i].lock().unwrap_or_else(|p| p.into_inner());
            f(&conn)
        })
        .await
        .map_err(|e| AppError::Internal(format!("tâche de lecture : {e}")))?
    }

    /// Accès direct à la connexion d'écriture hors transaction (outils : import, migration).
    pub fn with_writer<T>(&self, f: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        let mut conn = self.writer.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut conn)
    }
}

/// Applique les migrations manquantes, chacune dans sa transaction. Les clés
/// étrangères sont suspendues pendant la reconstruction d'une table (procédure
/// SQLite) puis revérifiées : la moindre référence cassée annule la migration.
fn migrate(conn: &mut Connection) -> AppResult<()> {
    let mut version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version == 0 {
        // Base créée avant le suivi des versions : schéma initial
        version = 1;
        conn.pragma_update(None, "user_version", 1)?;
    }
    for (target, sql) in MIGRATIONS.iter().filter(|(n, _)| *n > version) {
        conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
        let result = (|| -> AppResult<()> {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            let broken: i64 = tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
            if broken > 0 {
                return Err(AppError::Internal(format!("migration {target} : {broken} référence(s) cassée(s), annulée")));
            }
            tx.pragma_update(None, "user_version", target)?;
            tx.commit()?;
            Ok(())
        })();
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        result?;
        tracing::info!("base migrée en version {target}");
    }
    Ok(())
}

/// Secrets générés une fois, au premier démarrage après la migration 3.
fn ensure_secrets(conn: &Connection) -> AppResult<()> {
    use rand::RngExt;
    let mut key = [0u8; 32];
    rand::rng().fill(&mut key);
    conn.execute("INSERT OR IGNORE INTO app_secrets (name, value) VALUES ('profile_token', ?1)", [hex::encode(key)])?;
    Ok(())
}

impl Db {
    /// Clé de signature des jetons de profil (32 octets).
    pub fn profile_secret(&self) -> AppResult<Vec<u8>> {
        self.with_writer(|c| {
            let v: String = c.query_row("SELECT value FROM app_secrets WHERE name = 'profile_token'", [], |r| r.get(0))?;
            hex::decode(v).map_err(|e| AppError::Internal(format!("secret de profil illisible : {e}")))
        })
    }

    /// Contrôle d'intégrité (pages, index, clés étrangères) et volumes par table.
    /// L'outil `sqlite3` standard ne connaît pas la collation BINOME_CI : ce contrôle
    /// passe donc par le binaire.
    pub fn check(&self) -> AppResult<String> {
        let conn = self.readers[0].lock().unwrap_or_else(|p| p.into_inner());
        let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        let broken: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let mut lines = vec![format!("intégrité : {integrity}"), format!("références cassées : {broken}"), format!("version du schéma : {version}")];
        for table in ["players", "characters", "games", "game_stats", "player_avatars", "custom_reactions"] {
            let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            lines.push(format!("{table:<18} {n:>6}"));
        }
        if integrity != "ok" || broken > 0 {
            return Err(AppError::Internal(lines.join("\n")));
        }
        Ok(lines.join("\n"))
    }

    /// Copie cohérente de la base, serveur en marche (`VACUUM INTO`).
    pub fn backup(&self, target: &Path) -> AppResult<()> {
        if target.exists() {
            return Err(AppError::Internal(format!("{} existe déjà : rien n'est écrasé", target.display())));
        }
        let conn = self.readers[0].lock().unwrap_or_else(|p| p.into_inner());
        conn.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
        Ok(())
    }
}

/// Raccourcis de requêtes.
pub trait Q {
    fn conn(&self) -> &Connection;

    /// Une ligne optionnelle.
    fn opt<T>(&self, sql: &str, params: impl rusqlite::Params, f: impl FnOnce(&rusqlite::Row) -> rusqlite::Result<T>) -> AppResult<Option<T>> {
        use rusqlite::OptionalExtension;
        Ok(self.conn().query_row(sql, params, f).optional()?)
    }

    /// Toutes les lignes.
    fn all<T>(&self, sql: &str, params: impl rusqlite::Params, f: impl FnMut(&rusqlite::Row) -> rusqlite::Result<T>) -> AppResult<Vec<T>> {
        let mut stmt = self.conn().prepare_cached(sql)?;
        let rows = stmt.query_map(params, f)?.collect::<rusqlite::Result<Vec<T>>>()?;
        Ok(rows)
    }

    /// Une valeur scalaire (la requête doit renvoyer une ligne).
    fn one<T: rusqlite::types::FromSql>(&self, sql: &str, params: impl rusqlite::Params) -> AppResult<T> {
        Ok(self.conn().query_row(sql, params, |r| r.get(0))?)
    }

    fn exists(&self, sql: &str, params: impl rusqlite::Params) -> AppResult<bool> {
        Ok(self.opt(sql, params, |_| Ok(()))?.is_some())
    }

    /// INSERT / UPDATE / DELETE → nombre de lignes touchées.
    fn run(&self, sql: &str, params: impl rusqlite::Params) -> AppResult<usize> {
        Ok(self.conn().prepare_cached(sql)?.execute(params)?)
    }

    fn insert(&self, sql: &str, params: impl rusqlite::Params) -> AppResult<i64> {
        self.conn().prepare_cached(sql)?.execute(params)?;
        Ok(self.conn().last_insert_rowid())
    }
}

impl Q for Connection {
    fn conn(&self) -> &Connection {
        self
    }
}

impl Q for Transaction<'_> {
    fn conn(&self) -> &Connection {
        self
    }
}

/// Une violation de clé étrangère (suppression d'une ligne encore référencée).
pub fn is_constraint(e: &AppError) -> bool {
    matches!(e, AppError::Internal(m) if m.contains("FOREIGN KEY constraint failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base au format d'origine (version 1, cascade salon → parties), migrée à l'ouverture.
    #[test]
    fn migration_2_keeps_games_when_room_is_deleted() {
        let dir = std::env::temp_dir().join(format!("binome-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v1.sqlite");
        {
            let conn = open_connection(&path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute_batch(
                "INSERT INTO players (id, pseudo) VALUES (1, 'Hote');
                 INSERT INTO cosmos (id, name, slug) VALUES (1, 'Disney', 'disney'), (2, 'Pixar', 'pixar');
                 INSERT INTO rooms (id, code, created_by) VALUES (7, 'ABCDEF', 1);
                 INSERT INTO games (id, room_id, status) VALUES (41, 7, 'finished');
                 INSERT INTO game_stats (game_id, player_pseudo, character_name, score) VALUES (41, 'Hote', 'X', 9);
                 INSERT INTO games (id, room_id, status) VALUES (50, 7, 'finished');
                 DELETE FROM games WHERE id = 50;",
            )
            .unwrap();
            // Avant migration : la cascade emporterait les parties
            let cascade: String = conn.query_row("SELECT sql FROM sqlite_master WHERE name = 'games'", [], |r| r.get(0)).unwrap();
            assert!(cascade.contains("ON DELETE CASCADE"));
        }

        let db = Db::open(&path).unwrap();
        db.with_writer(|c| {
            assert_eq!(c.one::<i64>("PRAGMA user_version", [])?, 10);
            // Migration 10 : fiche rédigée pour Disney, aucune pour les autres, sans image
            assert!(c.one::<Option<String>>("SELECT description FROM cosmos WHERE slug = 'disney'", [])?.is_some_and(|d| d.contains("Pixar")));
            assert_eq!(c.one::<Option<String>>("SELECT description FROM cosmos WHERE slug = 'pixar'", [])?, None);
            assert_eq!(c.one::<i64>("SELECT COUNT(*) FROM cosmos WHERE image IS NOT NULL", [])?, 0);
            // Migration 9 : activité initialisée à la migration, aucun profil anonymisé
            assert!(c.one::<Option<String>>("SELECT last_seen_at FROM players WHERE id = 1", [])?.is_some());
            assert_eq!(c.one::<Option<String>>("SELECT deleted_at FROM players WHERE id = 1", [])?, None);
            // Migration 8 : boîte à idées vide
            assert_eq!(c.one::<i64>("SELECT COUNT(*) FROM ideas", [])? + c.one::<i64>("SELECT COUNT(*) FROM idea_votes", [])?, 0);
            // Migration 7 : aucun administrateur désigné
            assert_eq!(c.one::<Option<String>>("SELECT admin_granted_at FROM players WHERE id = 1", [])?, None);
            // Migration 6 : seul « Cosmos imposé », seuls Disney et Films saga, profils non validés
            assert_eq!(c.one::<String>("SELECT value FROM app_settings WHERE name = 'game_modes'", [])?, r#"["cosmos"]"#);
            assert_eq!(c.one::<String>("SELECT group_concat(slug) FROM cosmos WHERE is_enabled = 1", [])?, "disney");
            assert_eq!(c.one::<Option<String>>("SELECT validated_at FROM players WHERE id = 1", [])?, None);
            // Migration 5 : compteur de changements de personnage, vide pour l'existant
            assert_eq!(c.one::<Option<String>>("SELECT character_changes FROM games WHERE id = 41", [])?, None);
            // Migration 4 : le code du salon est copié dans la partie
            assert_eq!(c.one::<Option<String>>("SELECT room_code FROM games WHERE id = 41", [])?.as_deref(), Some("ABCDEF"));
            c.run("DELETE FROM rooms WHERE id = 7", [])?;
            assert_eq!(c.one::<Option<i64>>("SELECT room_id FROM games WHERE id = 41", [])?, None);
            // … sans perdre le code du salon (migration 4)
            assert_eq!(c.one::<Option<String>>("SELECT room_code FROM games WHERE id = 41", [])?.as_deref(), Some("ABCDEF"));
            assert_eq!(c.one::<i64>("SELECT score FROM game_stats WHERE game_id = 41", [])?, 9);
            // Séquence conservée : pas de réutilisation de l'id 50
            let next = c.insert("INSERT INTO games (status) VALUES ('waiting')", [])?;
            assert_eq!(next, 51);
            Ok(())
        })
        .unwrap();
        assert!(db.check().unwrap().contains("version du schéma : 10"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
