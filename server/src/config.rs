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
    /// Empreinte du mot de passe d'administration (`binome hash-password
    /// --admin`) ; absente → administration fermée (403).
    pub admin_password_hash: Option<String>,
    /// Pseudo du profil administrateur (`BINOME_ADMIN_PSEUDO`) : ce profil ne
    /// s'ouvre qu'avec le mot de passe d'administration, et ouvre l'espace admin.
    pub admin_pseudo: Option<String>,
    /// Ancienne variable en clair (BINOME_ADMIN_PASSWORD / CHARACTERS_ACCESS_PASSWORD)
    /// encore présente : ignorée, signalée au démarrage pour être retirée du .env.
    pub legacy_admin_password_set: bool,
    /// Empreinte du mot de passe exigé pour supprimer des parties (`binome
    /// hash-password`) ; absente → suppression de parties désactivée.
    pub delete_password_hash: Option<String>,
    /// `lan-url.json` (lien de partage sur le réseau local, écrit par
    /// scripts/update-lan-ip.sh) relu à chaque requête ; absent hors LAN.
    pub lan_url_file: Option<PathBuf>,
    /// `BINOME_HSTS=1` : envoie `Strict-Transport-Security`. Seulement derrière
    /// HTTPS (Railway) ; jamais sur le Mac en HTTP du réseau local.
    pub hsts: bool,
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
            admin_password_hash: var("BINOME_ADMIN_PASSWORD_HASH"),
            admin_pseudo: var("BINOME_ADMIN_PSEUDO").map(|p| p.trim().to_string()),
            legacy_admin_password_set: var("BINOME_ADMIN_PASSWORD").is_some() || var("CHARACTERS_ACCESS_PASSWORD").is_some(),
            lan_url_file: var("BINOME_LAN_URL_FILE").map(PathBuf::from),
            delete_password_hash: var("BINOME_DELETE_PASSWORD_HASH"),
            hsts: var("BINOME_HSTS").is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes")),
        })
    }
}
