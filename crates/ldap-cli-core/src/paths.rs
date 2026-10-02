use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::App;

const CONFIG_FILE_NAME: &str = "config.toml";
const CREDENTIALS_FILE_NAME: &str = "credentials.bin";

impl App {
    /// Per-user tool directory: %APPDATA%\<dir_name> on Windows,
    /// ~/.config/<dir_name> (or $XDG_CONFIG_HOME/<dir_name>) on Linux.
    pub fn root_dir(&self) -> Result<PathBuf> {
        dirs::config_dir()
            .map(|d| d.join(self.dir_name))
            .ok_or_else(|| Error::Config("could not determine home/config directory".into()))
    }

    /// Directory holding all stages' subdirectories.
    pub fn stages_dir(&self) -> Result<PathBuf> {
        Ok(self.root_dir()?.join("stages"))
    }

    /// Where config.toml and credentials.bin live: the tool directory, or
    /// the selected stage's subdirectory of it.
    pub fn config_dir(&self) -> Result<PathBuf> {
        match &self.stage {
            Some(stage) => Ok(self.stages_dir()?.join(stage)),
            None => self.root_dir(),
        }
    }

    pub fn config_file_path(&self, override_path: Option<&PathBuf>) -> Result<PathBuf> {
        if let Some(p) = override_path {
            return Ok(p.clone());
        }
        Ok(self.config_dir()?.join(CONFIG_FILE_NAME))
    }

    pub fn credentials_file_path(&self) -> Result<PathBuf> {
        Ok(self.config_dir()?.join(CREDENTIALS_FILE_NAME))
    }

    pub fn ensure_config_dir(&self) -> Result<PathBuf> {
        let dir = self.config_dir()?;
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            // The tool root holds every stage, so lock it down as well.
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(self.root_dir()?, mode.clone())?;
            std::fs::set_permissions(&dir, mode)?;
        }
        Ok(dir)
    }
}
