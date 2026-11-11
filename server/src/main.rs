//! Binome — serveur unique : API REST, WebSocket temps réel, minuteries et
//! fichiers statiques du front, dans un seul processus.
//!
//!   binome [serve]                        serveur (défaut)
//!   binome healthcheck                    sonde HTTP pour Docker (image sans shell ni curl)
//!   binome import-mysql <dossier>         reprise unique des données de l'ancien backend
//!   binome import-characters <fichier.json> [--images-dir D] [--skip-existing]

mod clock;
mod config;
mod db;
mod error;
mod http;
mod migrate;
mod ratelimit;
mod realtime;
mod services;
mod state;
mod support;
mod timers;

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
        Some("import-mysql") => match args.get(1) {
            Some(dir) => run_tool(&cfg, |db| migrate::import_mysql_export(db, std::path::Path::new(dir))),
            None => fail("usage : binome import-mysql <dossier d'export>"),
        },
        Some("import-characters") => match args.get(1) {
            Some(file) => {
                let images = args.iter().position(|a| a == "--images-dir").and_then(|i| args.get(i + 1)).cloned();
                let skip = args.iter().any(|a| a == "--skip-existing");
                run_tool(&cfg, |db| migrate::import_characters(db, std::path::Path::new(file), images.as_deref(), skip))
            }
            None => fail("usage : binome import-characters <fichier.json> [--images-dir D] [--skip-existing]"),
        },
        Some(other) => fail(&format!("commande inconnue : {other}")),
    };
    std::process::exit(code);
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
        if cfg.admin_password.is_none() {
            tracing::warn!("aucun mot de passe d'administration : la gestion des personnages est fermée (403)");
        }
        let state = AppState {
            db,
            hub: realtime::hub::Hub::new(),
            cfg: Arc::new(cfg.clone()),
            limiter: Arc::new(ratelimit::RateLimiter::default()),
        };
        timers::rearm(&state).await;

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
