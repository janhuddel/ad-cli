use std::ffi::c_void;
use std::ptr;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

use crate::error::{AppError, Result};

/// CRYPTPROTECT_UI_FORBIDDEN: never show DPAPI's own credential-prompt UI —
/// this tool runs non-interactively (e.g. over SSH-adjacent tooling) and a
/// silently-blocking UI prompt would just hang it.
const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

/// Encrypts `plaintext` with DPAPI, scoped to the current Windows user (no
/// CRYPTPROTECT_LOCAL_MACHINE flag) so the blob is only decryptable by the
/// same OS account that created it.
pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut input = plaintext.to_vec();
    let blob_in = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_mut_ptr(),
    };
    let mut blob_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        CryptProtectData(
            &blob_in,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut blob_out,
        )
        .map_err(|e| AppError::Credential(format!("CryptProtectData failed: {e}")))?;
    }

    Ok(take_blob(blob_out))
}

/// Reverses `encrypt`. DPAPI itself authenticates the blob, so a tampered or
/// foreign-user blob fails here with an error rather than silently decrypting
/// garbage.
pub fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>> {
    let mut input = ciphertext.to_vec();
    let blob_in = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_mut_ptr(),
    };
    let mut blob_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        CryptUnprotectData(
            &blob_in,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut blob_out,
        )
        .map_err(|e| AppError::Credential(format!("CryptUnprotectData failed: {e}")))?;
    }

    Ok(take_blob(blob_out))
}

/// Copies the DPAPI output blob into an owned Vec and frees the LocalAlloc'd
/// buffer DPAPI handed back (per the CryptProtectData/CryptUnprotectData
/// contract: the caller owns pbData and must LocalFree it).
fn take_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(HLOCAL(blob.pbData as *mut c_void));
    }
    bytes
}
