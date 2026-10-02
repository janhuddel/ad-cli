use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::credentials;
use crate::error::{Error, Result};
use crate::App;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub base_dn: String,
    /// Bind identity exactly as the user supplied it (UPN, DOMAIN\user, or a
    /// full bind DN) — passed through to simple_bind unmodified. `None` means
    /// anonymous access: no bind and no stored password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind_identity: Option<String>,
    #[serde(default)]
    pub insecure_skip_verify: bool,
}

impl Config {
    pub fn load(app: &App, override_path: Option<&PathBuf>) -> Result<Self> {
        let path = app.config_file_path(override_path)?;
        let text = std::fs::read_to_string(&path).map_err(|_| Error::NotLoggedIn {
            login_command: app.login_command(),
        })?;
        toml::from_str(&text).map_err(|e| Error::Config(e.to_string()))
    }

    pub fn save(&self, app: &App, override_path: Option<&PathBuf>) -> Result<()> {
        app.ensure_config_dir()?;
        let path = app.config_file_path(override_path)?;
        let text = toml::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    pub fn ldaps_url(&self) -> String {
        format!("ldaps://{}:{}", self.host, self.port)
    }
}

/// A fully loaded session: connection settings + decrypted password (`None`
/// for anonymous access). Loaded at the start of every subcommand other than
/// `login`.
pub struct Session {
    pub config: Config,
    pub password: Option<Zeroizing<String>>,
}

pub fn load_session(app: &App, override_path: Option<&PathBuf>) -> Result<Session> {
    let config = Config::load(app, override_path)?;
    let password = match config.bind_identity {
        Some(_) => Some(credentials::load(app)?),
        None => None,
    };
    Ok(Session { config, password })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_toml_round_trips_with_and_without_bind_identity() {
        let with = "host = \"dc.example.com\"\nport = 636\nbase_dn = \"DC=example,DC=com\"\nbind_identity = \"u@example.com\"\ninsecure_skip_verify = false\n";
        let config: Config = toml::from_str(with).unwrap();
        assert_eq!(config.bind_identity.as_deref(), Some("u@example.com"));
        assert_eq!(toml::to_string_pretty(&config).unwrap(), with);

        let anonymous = "host = \"idm.example.com\"\nport = 636\nbase_dn = \"ou=users,o=example\"\ninsecure_skip_verify = false\n";
        let config: Config = toml::from_str(anonymous).unwrap();
        assert_eq!(config.bind_identity, None);
        assert_eq!(toml::to_string_pretty(&config).unwrap(), anonymous);
    }
}
