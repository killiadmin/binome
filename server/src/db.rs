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
        let writer = open_connection(path, rw)?;
        writer.execute_batch("PRAGMA journal_mode = WAL;")?;

        let has_schema: bool = writer.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'players')",
            [],
            |r| r.get(0),
        )?;
        if !has_schema {
            writer.execute_batch(SCHEMA)?;
            tracing::info!("schéma créé dans {}", path.display());
        }

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

impl Db {
    /// Contrôle d'intégrité (pages, index, clés étrangères) et volumes par table.
    /// L'outil `sqlite3` standard ne connaît pas la collation BINOME_CI : ce contrôle
    /// passe donc par le binaire.
    pub fn check(&self) -> AppResult<String> {
        let conn = self.readers[0].lock().unwrap_or_else(|p| p.into_inner());
        let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        let broken: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
        let mut lines = vec![format!("intégrité : {integrity}"), format!("références cassées : {broken}")];
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
