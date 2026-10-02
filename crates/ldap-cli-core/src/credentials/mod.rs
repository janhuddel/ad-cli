#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as backend;

#[cfg(unix)]
mod linux;
#[cfg(unix)]
use linux as backend;

use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::App;

const FORMAT_VERSION: u8 = 1;

pub fn save(app: &App, password: &str) -> Result<()> {
    let payload = backend::encrypt(app, password.as_bytes())?;
    let mut out = Vec::with_capacity(payload.len() + 1);
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&payload);

    let path = app.credentials_file_path()?;
    std::fs::write(&path, &out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn load(app: &App) -> Result<Zeroizing<String>> {
    let not_logged_in = || Error::NotLoggedIn {
        login_command: app.login_command(),
    };
    let path = app.credentials_file_path()?;
    let raw = std::fs::read(&path).map_err(|_| not_logged_in())?;
    let (version, payload) = raw.split_first().ok_or_else(not_logged_in)?;
    if *version != FORMAT_VERSION {
        return Err(Error::Credential(format!(
            "unsupported credential file format version {version}"
        )));
    }
    let plaintext = backend::decrypt(app, payload)?;
    let s = String::from_utf8(plaintext)
        .map_err(|_| Error::Credential("decrypted credential was not valid UTF-8".into()))?;
    Ok(Zeroizing::new(s))
}

pub fn delete(app: &App) -> Result<()> {
    let path = app.credentials_file_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
