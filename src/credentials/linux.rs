use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

use crate::error::{AppError, Result};

const NONCE_LEN: usize = 12;

/// Derives a symmetric key from stable local machine/user characteristics
/// (no GUI keyring daemon is assumed to be running, since this tool is meant
/// to also work headless over SSH).
///
/// Security note: this protects stored credentials against casual file copy
/// or other non-root users on the same box — it is NOT equivalent to a real
/// OS keyring and does not protect against root, or anyone else who can read
/// /etc/machine-id and knows this derivation scheme.
fn derive_key() -> Result<[u8; 32]> {
    let machine_id = std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .map_err(|_| {
            AppError::Credential("could not read /etc/machine-id for key derivation".into())
        })?;
    let uid = unsafe { libc::getuid() };
    let ikm = format!("{}:{}", machine_id.trim(), uid);

    let hk = Hkdf::<Sha256>::new(None, ikm.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(b"ad-cli-credential-key", &mut key)
        .map_err(|e| AppError::Credential(format!("key derivation failed: {e}")))?;
    Ok(key)
}

pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>> {
    let key = derive_key()?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| AppError::Credential(e.to_string()))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AppError::Credential(format!("encryption failed: {e}")))?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() < NONCE_LEN {
        return Err(AppError::Credential(
            "credential file is corrupt (too short)".into(),
        ));
    }
    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let key = derive_key()?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| AppError::Credential(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher.decrypt(nonce, ciphertext).map_err(|_| {
        AppError::Credential(
            "failed to decrypt stored credentials (wrong machine/user, or file corrupted)".into(),
        )
    })
}
