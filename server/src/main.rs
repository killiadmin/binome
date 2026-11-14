//! Binome — serveur unique : API REST, WebSocket temps réel, minuteries et
//! fichiers statiques du front, dans un seul processus.
//!
//!   binome [serve]                        serveur (défaut)
//!   binome healthcheck                    sonde HTTP pour Docker (image sans shell ni curl)
//!   binome import-characters <fichier.json> [--images-dir D] [--skip-existing]
//!   binome list-characters                catalogue complet (refusés compris)
//!   binome count-pending                  binômes en attente de validation
//!   binome hash-password [--admin] [--iterations N]
//!                                         empreinte du mot de passe de suppression, ou
//!                                         d'administration avec --admin (lu sur stdin)
//!   binome backup <fichier.sqlite>        copie cohérente de la base, serveur en marche
//!   binome check                          contrôle d'intégrité de la base (BINOME_DATABASE)
//!   binome prune-rooms [--apply]          adhésions de salon mortes (essai à blanc sans --apply)
//!   binome prune-players [--apply]        profils inactifs depuis 24 mois à anonymiser (essai à
//!                                         blanc sans --apply ; le serveur le fait chaque jour)

mod clock;
mod config;
mod db;
mod error;
mod http;
mod ratelimit;
mod realtime;
mod services;
mod state;
mod support;
mod timers;
mod tools;

use std::sync::Arc;

use config::Config;
use db::Db;
use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "binome=info".into()))
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => fail(&e),
    };
    let code = match args.first().map(String::as_str) {
        None | Some("serve") => serve(cfg),
        Some("healthcheck") => healthcheck(&cfg),
        Some("import-characters") => match args.get(1) {
            Some(file) => {
                let images = args.iter().position(|a| a == "--images-dir").and_then(|i| args.get(i + 1)).cloned();
                let skip = args.iter().any(|a| a == "--skip-existing");
                run_tool(&cfg, |db| tools::import_characters(db, std::path::Path::new(file), images.as_deref(), skip))
            }
            None => fail("usage : binome import-characters <fichier.json> [--images-dir D] [--skip-existing]"),
        },
        Some("check") => run_tool(&cfg, |db| db.check()),
        Some("hash-password") => hash_password(&args),
        Some("list-characters") => run_tool(&cfg, tools::list_characters),
        Some("count-pending") => run_tool(&cfg, tools::count_pending),
        Some("prune-rooms") => {
            let apply = args.iter().any(|a| a == "--apply");
            run_tool(&cfg, |db| tools::prune_rooms(db, apply))
        }
        Some("prune-players") => {
            let apply = args.iter().any(|a| a == "--apply");
            let admin_pseudo = cfg.admin_pseudo.clone();
            run_tool(&cfg, |db| tools::prune_players(db, admin_pseudo, apply))
        }
        Some("backup") => match args.get(1) {
            Some(file) => run_tool(&cfg, |db| {
                db.backup(std::path::Path::new(file))?;
                Ok(format!("Sauvegarde écrite : {file}"))
            }),
            None => fail("usage : binome backup <fichier.sqlite>"),
        },
        Some(other) => fail(&format!("commande inconnue : {other}")),
    };
    std::process::exit(code);
}

/// Lit le mot de passe sur l'entrée standard (jamais en argument : il finirait
/// dans l'historique du shell) et affiche la ligne à mettre dans le .env :
/// mot de passe de suppression par défaut, d'administration avec `--admin`.
fn hash_password(args: &[String]) -> i32 {
    let iterations = match args.iter().position(|a| a == "--iterations").and_then(|i| args.get(i + 1)) {
        Some(n) => n.parse().unwrap_or_else(|_| fail("--iterations : entier attendu")),
        None => support::password::DEFAULT_ITERATIONS,
    };
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        fail("lecture du mot de passe impossible");
    }
    let password = line.trim_end_matches(['\n', '\r']);
    if password.chars().count() < 8 {
        fail("mot de passe trop court (8 caractères minimum)");
    }
    let var = if args.iter().any(|a| a == "--admin") { "BINOME_ADMIN_PASSWORD_HASH" } else { "BINOME_DELETE_PASSWORD_HASH" };
    println!("{var}={}", support::password::hash(password, iterations));
    0
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2)
}

fn run_tool(cfg: &Config, f: impl FnOnce(&Db) -> error::AppResult<String>) -> i32 {
    let db = match Db::open(&cfg.database) {
        Ok(db) => db,
        Err(e) => fail(&format!("base : {e:?}")),
    };
    match f(&db) {
        Ok(report) => {
            println!("{report}");
            0
        }
        Err(e) => {
            eprintln!("échec : {e:?}");
            1
        }
    }
}

fn serve(cfg: Config) -> i32 {
    // Deux threads suffisent largement : le serveur attend surtout le réseau,
    // et SQLite travaille sur le pool bloquant de Tokio.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .enable_all()
        .build()
        .expect("runtime Tokio");
    runtime.block_on(async move {
        let db = match Db::open(&cfg.database) {
            Ok(db) => db,
            Err(e) => fail(&format!("base {} : {e:?}", cfg.database.display())),
        };
        match cfg.delete_password_hash.as_deref() {
            None => tracing::info!("aucun mot de passe de suppression : la suppression de parties est désactivée"),
            Some(h) if !support::password::is_well_formed(h) => fail("BINOME_DELETE_PASSWORD_HASH mal formée : régénère-la avec `binome hash-password`"),
            Some(_) => {}
        }
        match cfg.admin_password_hash.as_deref() {
            None => tracing::warn!("aucun mot de passe d'administration : la gestion des personnages est fermée (403)"),
            Some(h) if !support::password::is_well_formed(h) => fail("BINOME_ADMIN_PASSWORD_HASH mal formée : régénère-la avec `binome hash-password --admin`"),
            Some(_) => {}
        }
        if cfg.legacy_admin_password_set {
            tracing::warn!(
                "BINOME_ADMIN_PASSWORD (mot de passe en clair) est ignorée : retire-la du .env et utilise BINOME_ADMIN_PASSWORD_HASH (`binome hash-password --admin`)"
            );
        }
        let profile_secret = match db.profile_secret() {
            Ok(s) => Arc::new(s),
            Err(e) => fail(&format!("secret des jetons de profil : {e:?}")),
        };
        match (cfg.admin_pseudo.as_deref(), cfg.admin_password_hash.is_some()) {
            (None, _) => tracing::warn!("BINOME_ADMIN_PSEUDO absent : aucun profil administrateur (le cadenas d'administration reste masqué)"),
            (Some(p), false) => tracing::warn!("profil administrateur « {p} » inaccessible : BINOME_ADMIN_PASSWORD_HASH absente"),
            (Some(p), true) => tracing::info!("profil administrateur : « {p} »"),
        }
        let state = AppState {
            profile_secret,
            db,
            hub: realtime::hub::Hub::new(),
            cfg: Arc::new(cfg.clone()),
            limiter: Arc::new(ratelimit::RateLimiter::default()),
            last_seen: Arc::default(),
        };
        timers::rearm(&state).await;
        timers::schedule_inactive_purge(state.clone());

        let listener = match tokio::net::TcpListener::bind(cfg.bind).await {
            Ok(l) => l,
            Err(e) => fail(&format!("écoute sur {} : {e}", cfg.bind)),
        };
        tracing::info!("Binome sur http://{}{}/", cfg.bind, cfg.public_prefix);
        let app = http::router(state);
        if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown()).await {
            tracing::error!("serveur : {e}");
            return 1;
        }
        0
    })
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("arrêt demandé");
}

/// `GET {prefix}/up` en HTTP/1.0 brut, sans dépendance.
fn healthcheck(cfg: &Config) -> i32 {
    use std::io::{Read, Write};
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], cfg.bind.port()));
    let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2)) else { return 1 };
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let req = format!("GET {}/up HTTP/1.0\r\nHost: localhost\r\n\r\n", cfg.public_prefix);
    if s.write_all(req.as_bytes()).is_err() {
        return 1;
    }
    let mut buf = [0u8; 16];
    match s.read(&mut buf) {
        Ok(n) if String::from_utf8_lossy(&buf[..n]).contains(" 200") => 0,
        _ => 1,
    }
}
