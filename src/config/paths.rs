use std::path::PathBuf;

use crate::error::{AppError, Result};

const APP_DIR_NAME: &str = "ad-cli";
const CONFIG_FILE_NAME: &str = "config.toml";
const CREDENTIALS_FILE_NAME: &str = "credentials.bin";

/// Per-user config directory: %APPDATA%\ad-cli on Windows, ~/.config/ad-cli
/// (or $XDG_CONFIG_HOME/ad-cli) on Linux.
pub fn config_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|d| d.join(APP_DIR_NAME))
        .ok_or_else(|| AppError::Config("could not determine home/config directory".into()))
}

pub fn config_file_path(override_path: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(p.clone());
    }
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

pub fn credentials_file_path() -> Result<PathBuf> {
    Ok(config_dir()?.join(CREDENTIALS_FILE_NAME))
}

pub fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}
