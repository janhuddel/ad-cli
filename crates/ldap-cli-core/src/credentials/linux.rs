use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

use crate::error::{Error, Result};
use crate::App;

const NONCE_LEN: usize = 12;

/// Derives a symmetric key from stable local machine/user characteristics
/// (no GUI keyring daemon is assumed to be running, since this tool is meant
/// to also work headless over SSH).
///
/// Security note: this protects stored credentials against casual file copy
/// or other non-root users on the same box — it is NOT equivalent to a real
/// OS keyring and does not protect against root, or anyone else who can read
/// /etc/machine-id and knows this derivation scheme.
fn derive_key(app: &App) -> Result<[u8; 32]> {
    let machine_id = std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .map_err(|_| {
            Error::Credential("could not read /etc/machine-id for key derivation".into())
        })?;
    let uid = unsafe { libc::getuid() };
    let ikm = format!("{}:{}", machine_id.trim(), uid);

    let hk = Hkdf::<Sha256>::new(None, ikm.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(key_info(app).as_bytes(), &mut key)
        .map_err(|e| Error::Credential(format!("key derivation failed: {e}")))?;
    Ok(key)
}

/// HKDF info string. For `ad` this must stay "ad-cli-credential-key" (its
/// value before the workspace split), or existing credential files would no
/// longer decrypt.
fn key_info(app: &App) -> String {
    format!("{}-credential-key", app.dir_name)
}

pub fn encrypt(app: &App, plaintext: &[u8]) -> Result<Vec<u8>> {
    let key = derive_key(app)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| Error::Credential(e.to_string()))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| Error::Credential(format!("encryption failed: {e}")))?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(app: &App, payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() < NONCE_LEN {
        return Err(Error::Credential(
            "credential file is corrupt (too short)".into(),
        ));
    }
    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let key = derive_key(app)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| Error::Credential(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher.decrypt(nonce, ciphertext).map_err(|_| {
        Error::Credential(
            "failed to decrypt stored credentials (wrong machine/user, or file corrupted)".into(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_info_for_ad_is_unchanged() {
        let ad = App {
            bin_name: "ad",
            dir_name: "ad-cli",
            stage: None,
        };
        assert_eq!(key_info(&ad), "ad-cli-credential-key");
    }
}
