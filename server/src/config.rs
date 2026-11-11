//! Configuration centralisée, lue une fois au démarrage depuis l'environnement.

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    /// Adresse d'écoute HTTP + WebSocket.
    pub bind: SocketAddr,
    /// Fichier SQLite.
    pub database: PathBuf,
    /// Dossier du front compilé (`frontend/dist`) ; absent → API seule (dev avec Vite).
    pub static_dir: Option<PathBuf>,
    /// Préfixe public de l'application (`/binome`) : routes servies dessous et URLs d'images.
    pub public_prefix: String,
    /// Mot de passe d'administration ; vide → administration fermée (403), comme Laravel.
    pub admin_password: Option<String>,
    /// `lan-url.json` (lien de partage sur le réseau local, écrit par
    /// scripts/update-lan-ip.sh) relu à chaque requête ; absent hors LAN.
    pub lan_url_file: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());

        let bind = var("BINOME_BIND")
            .unwrap_or_else(|| "0.0.0.0:8080".into())
            .parse()
            .map_err(|e| format!("BINOME_BIND invalide : {e}"))?;

        let prefix = var("BINOME_PUBLIC_PREFIX").unwrap_or_else(|| "/binome".into());
        let public_prefix = format!("/{}", prefix.trim_matches('/'));

        Ok(Self {
            bind,
            database: var("BINOME_DATABASE").unwrap_or_else(|| "/data/binome.sqlite".into()).into(),
            static_dir: var("BINOME_STATIC_DIR").map(PathBuf::from),
            public_prefix: if public_prefix == "/" { String::new() } else { public_prefix },
            // Même variable que l'ancien backend, pour réutiliser le .env existant.
            admin_password: var("BINOME_ADMIN_PASSWORD").or_else(|| var("CHARACTERS_ACCESS_PASSWORD")),
            lan_url_file: var("BINOME_LAN_URL_FILE").map(PathBuf::from),
        })
    }
}
