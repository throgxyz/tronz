//! Web3 Secret Storage V3 keystore — encrypt and decrypt TRON private keys.
//!
//! Compatible with the format used by go-ethereum, TronLink, and gotron-sdk.
//! The `address` field stores the TRON base58check address (e.g. `TXyz…`)
//! rather than an Ethereum 20-byte hex address.
//!
//! # Format
//!
//! ```json
//! {
//!   "address": "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH",
//!   "crypto": {
//!     "cipher":       "aes-128-ctr",
//!     "ciphertext":   "<hex>",
//!     "cipherparams": { "iv": "<hex>" },
//!     "kdf":          "scrypt",
//!     "kdfparams":    { "n": 262144, "r": 8, "p": 1, "dklen": 32, "salt": "<hex>" },
//!     "mac":          "<hex>"
//!   },
//!   "id":      "<uuid-v4>",
//!   "version": 3
//! }
//! ```
//!
//! KDF: scrypt (N=262144, r=8, p=1).
//! Cipher: AES-128-CTR.
//! MAC: keccak256(derivedKey[16..32] ‖ ciphertext).

use std::path::{Path, PathBuf};

use aes::cipher::{KeyIvInit, StreamCipher};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use thiserror::Error;
use uuid::Uuid;

use crate::SignerError;

// ─── Scrypt parameters ────────────────────────────────────────────────────────

/// Standard (production) scrypt parameters.  N = 2^18 = 262 144.
const LOG_N: u8 = 18;
const R: u32 = 8;
const P: u32 = 1;
const DK_LEN: usize = 32;

// ─── AES-128-CTR type alias ───────────────────────────────────────────────────

type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;

// ─── JSON structures ──────────────────────────────────────────────────────────

/// Top-level keystore file structure (Web3 Secret Storage V3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreFile {
    /// TRON address in base58check format (e.g. `TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH`).
    pub address: String,
    /// Cryptographic parameters.
    pub crypto: CryptoJson,
    /// Random UUID v4 that uniquely identifies this keystore.
    pub id: String,
    /// Always 3.
    pub version: u8,
}

/// `crypto` sub-object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoJson {
    /// Symmetric cipher name — always `"aes-128-ctr"`.
    pub cipher: String,
    /// Hex-encoded ciphertext (32 bytes = 64 hex chars).
    pub ciphertext: String,
    /// Cipher-specific parameters.
    pub cipherparams: CipherparamsJson,
    /// Key-derivation function name — always `"scrypt"`.
    pub kdf: String,
    /// KDF-specific parameters.
    pub kdfparams: KdfparamsJson,
    /// Hex-encoded keccak256(derivedKey[16..32] ‖ ciphertext).
    pub mac: String,
}

/// `cipherparams` sub-object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherparamsJson {
    /// Hex-encoded 16-byte initialisation vector.
    pub iv: String,
}

/// `kdfparams` sub-object (scrypt-specific fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfparamsJson {
    /// CPU/memory cost parameter N (must be a power of two, e.g. 262144).
    pub n: u64,
    /// Block size parameter r.
    pub r: u32,
    /// Parallelisation parameter p.
    pub p: u32,
    /// Derived-key length in bytes (always 32).
    pub dklen: u32,
    /// Hex-encoded 32-byte random salt.
    pub salt: String,
}

// ─── KeystoreError ────────────────────────────────────────────────────────────

/// Errors specific to keystore operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum KeystoreError {
    /// Wrong password — MAC verification failed.
    #[error("invalid password or corrupted keystore")]
    InvalidPassword,
    /// The keystore uses an algorithm tronz does not support.
    #[error("unsupported cipher: {0}")]
    UnsupportedCipher(String),
    /// The keystore uses a KDF tronz does not support.
    #[error("unsupported KDF: {0}")]
    UnsupportedKdf(String),
    /// A required field has the wrong length or format.
    #[error("invalid keystore field: {0}")]
    InvalidField(&'static str),
    /// The scrypt cost parameter N is not a power of two.
    #[error("scrypt N must be a power of two, got {0}")]
    InvalidScryptN(u64),
    /// Scrypt internal error.
    #[error("scrypt error: {0}")]
    Scrypt(String),
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Encrypt `key_bytes` with `password` and write the keystore JSON to `dir`.
///
/// The file is named `<uuid>.json`. Returns the full path of the written file.
pub(crate) fn encrypt_to_file<P: AsRef<Path>>(
    key_bytes: &[u8; 32],
    address: &str,
    password: &str,
    dir: P,
    rng: &mut impl Rng,
) -> Result<PathBuf, SignerError> {
    let ks = encrypt_inner(key_bytes, address, password, rng, LOG_N, R, P)?;
    let path = dir.as_ref().join(format!("{}.json", ks.id));
    let json = serde_json::to_vec_pretty(&ks).map_err(SignerError::Json)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Deserialize a keystore file from `path` and decrypt it with `password`.
pub(crate) fn decrypt_from_file<P: AsRef<Path>>(
    path: P,
    password: &str,
) -> Result<[u8; 32], SignerError> {
    let contents = std::fs::read_to_string(path)?;
    let ks: KeystoreFile = serde_json::from_str(&contents).map_err(SignerError::Json)?;
    decrypt(&ks, password)
}

/// Encrypt a 32-byte private key into a [`KeystoreFile`] (in memory, no I/O).
///
/// Uses the standard scrypt parameters (N = 2^18).
pub fn encrypt(
    key_bytes: &[u8; 32],
    address: &str,
    password: &str,
    rng: &mut impl Rng,
) -> Result<KeystoreFile, SignerError> {
    encrypt_inner(key_bytes, address, password, rng, LOG_N, R, P)
}

/// Decrypt a [`KeystoreFile`] with the given `password`.
///
/// Returns the raw 32-byte private key on success, or
/// [`KeystoreError::InvalidPassword`] if the MAC does not match.
pub fn decrypt(ks: &KeystoreFile, password: &str) -> Result<[u8; 32], SignerError> {
    // ── Validate algorithm ────────────────────────────────────────────────────
    if ks.crypto.cipher != "aes-128-ctr" {
        return Err(KeystoreError::UnsupportedCipher(ks.crypto.cipher.clone()).into());
    }
    if ks.crypto.kdf != "scrypt" {
        return Err(KeystoreError::UnsupportedKdf(ks.crypto.kdf.clone()).into());
    }

    let kp = &ks.crypto.kdfparams;

    // ── Parse hex fields ──────────────────────────────────────────────────────
    let salt = hex::decode(&kp.salt)?;
    let iv_bytes = hex::decode(&ks.crypto.cipherparams.iv)?;
    let iv: [u8; 16] = iv_bytes
        .try_into()
        .map_err(|_| KeystoreError::InvalidField("iv must be 16 bytes"))?;
    let mut ciphertext = hex::decode(&ks.crypto.ciphertext)?;
    let stored_mac = hex::decode(&ks.crypto.mac)?;

    // ── Derive key via scrypt ─────────────────────────────────────────────────
    let n = kp.n;
    if n == 0 || n & (n - 1) != 0 {
        return Err(KeystoreError::InvalidScryptN(n).into());
    }
    let log_n = n.trailing_zeros() as u8;
    let params =
        scrypt::Params::new(log_n, kp.r, kp.p).map_err(|e| KeystoreError::Scrypt(e.to_string()))?;
    let mut derived_key = vec![0u8; kp.dklen as usize];
    scrypt::scrypt(password.as_bytes(), &salt, &params, &mut derived_key)
        .map_err(|e| KeystoreError::Scrypt(e.to_string()))?;

    // ── Verify MAC: keccak256(derivedKey[16..] || ciphertext) ─────────────────
    let computed_mac = Keccak256::new()
        .chain_update(&derived_key[16..])
        .chain_update(ciphertext.as_slice())
        .finalize();
    if computed_mac[..] != stored_mac[..] {
        return Err(KeystoreError::InvalidPassword.into());
    }

    // ── Decrypt AES-128-CTR ───────────────────────────────────────────────────
    let mut cipher = Aes128Ctr::new_from_slices(&derived_key[..16], &iv)
        .map_err(|_| KeystoreError::InvalidField("AES key/IV length error"))?;
    cipher.apply_keystream(&mut ciphertext);

    ciphertext
        .try_into()
        .map_err(|_| KeystoreError::InvalidField("ciphertext must be 32 bytes").into())
}

// ─── Internal encrypt implementation ─────────────────────────────────────────

fn encrypt_inner(
    key_bytes: &[u8; 32],
    address: &str,
    password: &str,
    rng: &mut impl Rng,
    log_n: u8,
    r: u32,
    p: u32,
) -> Result<KeystoreFile, SignerError> {
    // ── Generate random salt and IV ───────────────────────────────────────────
    let mut salt = [0u8; 32];
    let mut iv = [0u8; 16];
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(&mut iv);

    // ── Derive key via scrypt ─────────────────────────────────────────────────
    let params =
        scrypt::Params::new(log_n, r, p).map_err(|e| KeystoreError::Scrypt(e.to_string()))?;
    let mut derived_key = [0u8; DK_LEN];
    scrypt::scrypt(password.as_bytes(), &salt, &params, &mut derived_key)
        .map_err(|e| KeystoreError::Scrypt(e.to_string()))?;

    // ── Encrypt: AES-128-CTR (key = derivedKey[0..16]) ───────────────────────
    let mut ciphertext = *key_bytes;
    let mut cipher = Aes128Ctr::new_from_slices(&derived_key[..16], &iv)
        .map_err(|_| KeystoreError::InvalidField("AES key/IV length error"))?;
    cipher.apply_keystream(&mut ciphertext);

    // ── MAC: keccak256(derivedKey[16..32] || ciphertext) ─────────────────────
    let mac = Keccak256::new()
        .chain_update(&derived_key[16..])
        .chain_update(ciphertext)
        .finalize();

    Ok(KeystoreFile {
        address: address.to_string(),
        crypto: CryptoJson {
            cipher: "aes-128-ctr".into(),
            ciphertext: hex::encode(ciphertext),
            cipherparams: CipherparamsJson {
                iv: hex::encode(iv),
            },
            kdf: "scrypt".into(),
            kdfparams: KdfparamsJson {
                n: 1u64 << log_n,
                r,
                p,
                dklen: DK_LEN as u32,
                salt: hex::encode(salt),
            },
            mac: hex::encode(mac),
        },
        id: Uuid::new_v4().to_string(),
        version: 3,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rand::rngs::OsRng;

    use super::*;
    use crate::{LocalSigner, TronSigner};

    // Light scrypt params for fast tests (N=2^12=4096).
    const TEST_LOG_N: u8 = 12;
    const TEST_R: u32 = 8;
    const TEST_P: u32 = 6;

    const KEY_HEX: &str = "b5a4cea271ff424d7c31dc12a3e43e401df7a40d7412a15750f3f0b6b5449a28";
    const ADDR: &str = "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH";

    fn test_key() -> [u8; 32] {
        let b = hex::decode(KEY_HEX).unwrap();
        b.try_into().unwrap()
    }

    fn encrypt_light(key: &[u8; 32], addr: &str, password: &str) -> KeystoreFile {
        encrypt_inner(key, addr, password, &mut OsRng, TEST_LOG_N, TEST_R, TEST_P).unwrap()
    }

    // ── Round-trip ─────────────────────────────────────────────────────────────

    #[test]
    fn round_trip() {
        let key = test_key();
        let ks = encrypt_light(&key, ADDR, "correct-password");
        let recovered = decrypt(&ks, "correct-password").unwrap();
        assert_eq!(recovered, key);
    }

    #[test]
    fn wrong_password_returns_error() {
        let key = test_key();
        let ks = encrypt_light(&key, ADDR, "correct-password");
        let err = decrypt(&ks, "wrong-password").unwrap_err();
        assert!(err.to_string().contains("invalid password"), "got: {err}");
    }

    #[test]
    fn address_stored_as_tron_base58() {
        let key = test_key();
        let ks = encrypt_light(&key, ADDR, "pw");
        assert_eq!(ks.address, ADDR);
        assert!(
            ks.address.starts_with('T'),
            "TRON address must start with T"
        );
    }

    #[test]
    fn version_is_3() {
        let key = test_key();
        let ks = encrypt_light(&key, ADDR, "pw");
        assert_eq!(ks.version, 3);
    }

    #[test]
    fn id_is_valid_uuid() {
        let key = test_key();
        let ks = encrypt_light(&key, ADDR, "pw");
        // UUID v4: 8-4-4-4-12 hex chars separated by hyphens, 36 chars total.
        assert_eq!(ks.id.len(), 36);
        assert_eq!(ks.id.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn two_encryptions_differ() {
        let key = test_key();
        let ks1 = encrypt_light(&key, ADDR, "pw");
        let ks2 = encrypt_light(&key, ADDR, "pw");
        // Different random salt/IV → different ciphertext and MAC.
        assert_ne!(ks1.crypto.ciphertext, ks2.crypto.ciphertext);
        assert_ne!(ks1.crypto.kdfparams.salt, ks2.crypto.kdfparams.salt);
        assert_ne!(ks1.id, ks2.id);
    }

    #[test]
    fn json_round_trip() {
        let key = test_key();
        let ks = encrypt_light(&key, ADDR, "pw");
        // Serialize → deserialize → decrypt.
        let json = serde_json::to_string(&ks).unwrap();
        let ks2: KeystoreFile = serde_json::from_str(&json).unwrap();
        let recovered = decrypt(&ks2, "pw").unwrap();
        assert_eq!(recovered, key);
    }

    #[test]
    fn rejects_unsupported_cipher() {
        let key = test_key();
        let mut ks = encrypt_light(&key, ADDR, "pw");
        ks.crypto.cipher = "aes-256-gcm".into();
        let err = decrypt(&ks, "pw").unwrap_err();
        assert!(err.to_string().contains("cipher"), "got: {err}");
    }

    #[test]
    fn rejects_unsupported_kdf() {
        let key = test_key();
        let mut ks = encrypt_light(&key, ADDR, "pw");
        ks.crypto.kdf = "pbkdf2".into();
        let err = decrypt(&ks, "pw").unwrap_err();
        assert!(err.to_string().contains("KDF"), "got: {err}");
    }

    #[test]
    fn file_round_trip() {
        let key = test_key();
        let dir = tempfile::tempdir().unwrap();
        // Use LocalSigner helpers.
        let signer = LocalSigner::from_bytes(&key).unwrap();
        let path = signer.encrypt_keystore(dir.path(), "my-password").unwrap();
        assert!(path.exists());
        let recovered = LocalSigner::decrypt_keystore(&path, "my-password").unwrap();
        assert_eq!(signer.address(), recovered.address());
    }
}
