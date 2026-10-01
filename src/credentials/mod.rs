#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as backend;

#[cfg(unix)]
mod linux;
#[cfg(unix)]
use linux as backend;

use zeroize::Zeroizing;

use crate::error::Result;

const FORMAT_VERSION: u8 = 1;

pub fn save(password: &str) -> Result<()> {
    let payload = backend::encrypt(password.as_bytes())?;
    let mut out = Vec::with_capacity(payload.len() + 1);
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&payload);

    let path = crate::config::paths::credentials_file_path()?;
    std::fs::write(&path, &out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn load() -> Result<Zeroizing<String>> {
    use crate::error::AppError;

    let path = crate::config::paths::credentials_file_path()?;
    let raw = std::fs::read(&path).map_err(|_| AppError::NotLoggedIn)?;
    let (version, payload) = raw.split_first().ok_or(AppError::NotLoggedIn)?;
    if *version != FORMAT_VERSION {
        return Err(AppError::Credential(format!(
            "unsupported credential file format version {version}"
        )));
    }
    let plaintext = backend::decrypt(payload)?;
    let s = String::from_utf8(plaintext)
        .map_err(|_| AppError::Credential("decrypted credential was not valid UTF-8".into()))?;
    Ok(Zeroizing::new(s))
}

pub fn delete() -> Result<()> {
    let path = crate::config::paths::credentials_file_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
