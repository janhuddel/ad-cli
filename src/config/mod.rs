pub mod paths;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::credentials;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub base_dn: String,
    /// Bind identity exactly as the user supplied it (UPN, DOMAIN\user, or a
    /// full bind DN) — passed through to simple_bind unmodified.
    pub bind_identity: String,
    #[serde(default)]
    pub insecure_skip_verify: bool,
}

impl Config {
    pub fn load(override_path: Option<&PathBuf>) -> Result<Self> {
        let path = paths::config_file_path(override_path)?;
        let text = std::fs::read_to_string(&path).map_err(|_| AppError::NotLoggedIn)?;
        toml::from_str(&text).map_err(|e| AppError::Config(e.to_string()))
    }

    pub fn save(&self, override_path: Option<&PathBuf>) -> Result<()> {
        paths::ensure_config_dir()?;
        let path = paths::config_file_path(override_path)?;
        let text = toml::to_string_pretty(self).map_err(|e| AppError::Config(e.to_string()))?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    pub fn ldaps_url(&self) -> String {
        format!("ldaps://{}:{}", self.host, self.port)
    }
}

/// A fully loaded session: connection settings + decrypted password.
/// Loaded at the start of every subcommand other than `login`.
pub struct Session {
    pub config: Config,
    pub password: Zeroizing<String>,
}

pub fn load_session(override_path: Option<&PathBuf>) -> Result<Session> {
    let config = Config::load(override_path)?;
    let password = credentials::load()?;
    Ok(Session { config, password })
}
