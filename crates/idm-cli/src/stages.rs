//! Stage selection. The IdM exists once per environment, each with its own
//! server, so every stage is a separate connection profile in
//! `<config dir>/idm-cli/stages/<name>`; `settings.toml` in the tool
//! directory remembers the default stage.

use std::fs;

use ldap_cli_core::{App, Error, Result};
use serde::{Deserialize, Serialize};

const BIN_NAME: &str = "idm";
const DIR_NAME: &str = "idm-cli";
const SETTINGS_FILE_NAME: &str = "settings.toml";
const MAX_NAME_LEN: usize = 32;

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    default_stage: Option<String>,
}

/// The tool without a stage, for paths shared by all stages.
fn tool() -> App {
    for_stage(None)
}

fn for_stage(stage: Option<String>) -> App {
    App {
        bin_name: BIN_NAME,
        dir_name: DIR_NAME,
        stage,
    }
}

/// Stage names become directory names, so keep them to a safe alphabet.
pub fn validate_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "invalid stage name '{name}': use up to {MAX_NAME_LEN} letters, digits, '-' or '_'"
        )))
    }
}

/// Names of all stages that have been set up (have a config file), sorted.
pub fn list() -> Result<Vec<String>> {
    let dir = tool().stages_dir()?;
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| validate_name(name).is_ok())
        .filter(|name| {
            for_stage(Some(name.clone()))
                .config_file_path(None)
                .is_ok_and(|p| p.exists())
        })
        .collect();
    names.sort();
    Ok(names)
}

fn load_settings() -> Result<Settings> {
    let path = tool().root_dir()?.join(SETTINGS_FILE_NAME);
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| Error::Config(e.to_string())),
        Err(_) => Ok(Settings::default()),
    }
}

fn save_settings(settings: &Settings) -> Result<()> {
    let path = tool().root_dir()?.join(SETTINGS_FILE_NAME);
    let text = toml::to_string_pretty(settings).map_err(|e| Error::Config(e.to_string()))?;
    fs::write(path, text)?;
    Ok(())
}

pub fn default_stage() -> Result<Option<String>> {
    Ok(load_settings()?.default_stage)
}

pub fn set_default(name: &str) -> Result<()> {
    save_settings(&Settings {
        default_stage: Some(name.to_string()),
    })
}

pub fn clear_default_if(name: &str) -> Result<()> {
    if default_stage()?.as_deref() == Some(name) {
        save_settings(&Settings::default())?;
    }
    Ok(())
}

/// Picks the stage for a command that needs an existing one: the explicit
/// choice (`--stage` / `IDM_STAGE`), else the default, else the only stage.
pub fn select(explicit: Option<String>) -> Result<App> {
    let configured = list()?;
    let stage = match explicit {
        Some(name) => {
            validate_name(&name)?;
            if !configured.contains(&name) && !configured.is_empty() {
                return Err(Error::Other(format!(
                    "stage '{name}' is not set up (configured: {}). Run '{BIN_NAME} login --stage {name}' to set it up.",
                    configured.join(", ")
                )));
            }
            name
        }
        None => implicit_stage(&configured)?.ok_or_else(|| Error::NotLoggedIn {
            login_command: format!("{BIN_NAME} login --stage <name>"),
        })?,
    };
    Ok(for_stage(Some(stage)))
}

/// Picks the stage `login` sets up: like `select`, but the stage need not
/// exist yet, and with nothing set up at all a name is required.
pub fn select_for_login(explicit: Option<String>) -> Result<App> {
    let stage = match explicit {
        Some(name) => name,
        None => implicit_stage(&list()?)?.ok_or_else(|| {
            Error::Other(format!(
                "name the stage to set up, e.g. '{BIN_NAME} login --stage prod'"
            ))
        })?,
    };
    validate_name(&stage)?;
    Ok(for_stage(Some(stage)))
}

/// The default stage if it's still set up, else the only stage. `None` if
/// nothing is set up; an error if several are but none is the default.
fn implicit_stage(configured: &[String]) -> Result<Option<String>> {
    if let Some(default) = default_stage()?.filter(|d| configured.contains(d)) {
        return Ok(Some(default));
    }
    match configured {
        [] => Ok(None),
        [only] => Ok(Some(only.clone())),
        _ => Err(Error::Other(format!(
            "several stages are set up ({}) but none is the default. Pass --stage <name> or run '{BIN_NAME} stage default <name>'.",
            configured.join(", ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_accepts_safe_names_only() {
        for ok in ["prod", "test-2", "abn_a", "E1"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "../x", "a b", "a/b", "ü", &"x".repeat(33)] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }
}
