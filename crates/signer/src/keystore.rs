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
    /// Key-derivation function name: `"scrypt"` (written) or `"pbkdf2"` (read).
    pub kdf: String,
    /// KDF-specific parameters.
    pub kdfparams: KdfparamsType,
    /// Hex-encoded keccak256(derivedKey[16..32] ‖ ciphertext).
    pub mac: String,
}

/// `cipherparams` sub-object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherparamsJson {
    /// Hex-encoded 16-byte initialisation vector.
    pub iv: String,
}

/// `kdfparams` sub-object. Its shape depends on the sibling `kdf` field.
///
/// tronz always writes [`Scrypt`](Self::Scrypt), but reads either so that
/// keystores produced by wallets that default to PBKDF2 can be imported.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KdfparamsType {
    /// PBKDF2 parameters (read-only support).
    Pbkdf2 {
        /// Iteration count.
        c: u32,
        /// Derived-key length in bytes (always 32).
        dklen: u32,
        /// Pseudo-random function — only `"hmac-sha256"` is supported.
        prf: String,
        /// Hex-encoded random salt.
        salt: String,
    },
    /// Scrypt parameters.
    Scrypt {
        /// CPU/memory cost parameter N (must be a power of two, e.g. 262144).
        n: u64,
        /// Block size parameter r.
        r: u32,
        /// Parallelisation parameter p.
        p: u32,
        /// Derived-key length in bytes (always 32).
        dklen: u32,
        /// Hex-encoded 32-byte random salt.
        salt: String,
    },
}

impl KdfparamsType {
    /// The declared derived-key length, common to both KDFs.
    pub const fn dklen(&self) -> u32 {
        match self {
            Self::Pbkdf2 { dklen, .. } | Self::Scrypt { dklen, .. } => *dklen,
        }
    }

    /// The hex-encoded salt, common to both KDFs.
    pub fn salt(&self) -> &str {
        match self {
            Self::Pbkdf2 { salt, .. } | Self::Scrypt { salt, .. } => salt,
        }
    }
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
    /// The keystore uses a PBKDF2 pseudo-random function tronz does not support.
    #[error("unsupported PBKDF2 prf: {0}")]
    UnsupportedPrf(String),
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
    // ── Parse hex fields ──────────────────────────────────────────────────────
    let iv_bytes = hex::decode(&ks.crypto.cipherparams.iv)?;
    let iv: [u8; 16] =
        iv_bytes.try_into().map_err(|_| KeystoreError::InvalidField("iv must be 16 bytes"))?;
    let mut ciphertext = hex::decode(&ks.crypto.ciphertext)?;
    let stored_mac = hex::decode(&ks.crypto.mac)?;

    // ── Derive key ────────────────────────────────────────────────────────────
    let derived_key = derive_key(&ks.crypto.kdf, &ks.crypto.kdfparams, password)?;

    // ── Verify MAC: keccak256(derivedKey[16..32] || ciphertext) ───────────────
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

/// Stretch `password` into the 32-byte key that guards the MAC and the AES key.
fn derive_key(
    kdf: &str,
    params: &KdfparamsType,
    password: &str,
) -> Result<[u8; DK_LEN], SignerError> {
    // The MAC and cipher keys are carved out of a 32-byte derived key, so any
    // other length would slice out of bounds.
    if params.dklen() as usize != DK_LEN {
        return Err(KeystoreError::InvalidField("dklen must be 32").into());
    }
    let salt = hex::decode(params.salt())?;
    let mut derived_key = [0u8; DK_LEN];

    match (kdf, params) {
        ("scrypt", KdfparamsType::Scrypt { n, r, p, .. }) => {
            if *n == 0 || n & (n - 1) != 0 {
                return Err(KeystoreError::InvalidScryptN(*n).into());
            }
            let params = scrypt::Params::new(n.trailing_zeros() as u8, *r, *p)
                .map_err(|e| KeystoreError::Scrypt(e.to_string()))?;
            scrypt::scrypt(password.as_bytes(), &salt, &params, &mut derived_key)
                .map_err(|e| KeystoreError::Scrypt(e.to_string()))?;
        }
        ("pbkdf2", KdfparamsType::Pbkdf2 { c, prf, .. }) => {
            if prf != "hmac-sha256" {
                return Err(KeystoreError::UnsupportedPrf(prf.clone()).into());
            }
            if *c == 0 {
                return Err(KeystoreError::InvalidField("pbkdf2 c must be non-zero").into());
            }
            pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), &salt, *c, &mut derived_key);
        }
        ("scrypt" | "pbkdf2", _) => {
            return Err(KeystoreError::InvalidField("kdfparams do not match kdf").into());
        }
        (other, _) => return Err(KeystoreError::UnsupportedKdf(other.to_string()).into()),
    }

    Ok(derived_key)
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
    let mac = Keccak256::new().chain_update(&derived_key[16..]).chain_update(ciphertext).finalize();

    Ok(KeystoreFile {
        address: address.to_string(),
        crypto: CryptoJson {
            cipher: "aes-128-ctr".into(),
            ciphertext: hex::encode(ciphertext),
            cipherparams: CipherparamsJson { iv: hex::encode(iv) },
            kdf: "scrypt".into(),
            kdfparams: KdfparamsType::Scrypt {
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
    use super::*;
    use crate::LocalSigner;

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
        encrypt_inner(key, addr, password, &mut rand::rng(), TEST_LOG_N, TEST_R, TEST_P).unwrap()
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
        assert!(ks.address.starts_with('T'), "TRON address must start with T");
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
        assert_ne!(ks1.crypto.kdfparams.salt(), ks2.crypto.kdfparams.salt());
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
        ks.crypto.kdf = "argon2id".into();
        let err = decrypt(&ks, "pw").unwrap_err();
        assert!(err.to_string().contains("KDF"), "got: {err}");
    }

    #[test]
    fn rejects_kdf_params_mismatch() {
        let key = test_key();
        let mut ks = encrypt_light(&key, ADDR, "pw");
        // Scrypt params under a pbkdf2 label.
        ks.crypto.kdf = "pbkdf2".into();
        let err = decrypt(&ks, "pw").unwrap_err();
        assert!(err.to_string().contains("kdfparams do not match"), "got: {err}");
    }

    #[test]
    fn rejects_dklen_other_than_32() {
        let key = test_key();
        // A dklen between 10 and 15 used to slice past the end of the derived
        // key and panic; anything but 32 must be a clean error.
        for dklen in [0u32, 8, 12, 15, 31, 33, u32::MAX] {
            let mut ks = encrypt_light(&key, ADDR, "pw");
            let KdfparamsType::Scrypt { n, r, p, salt, .. } = ks.crypto.kdfparams.clone() else {
                unreachable!("encrypt always writes scrypt params")
            };
            ks.crypto.kdfparams = KdfparamsType::Scrypt { n, r, p, dklen, salt };
            let err = decrypt(&ks, "pw").unwrap_err();
            assert!(err.to_string().contains("dklen"), "dklen={dklen}, got: {err}");
        }
    }

    // ── PBKDF2 import ──────────────────────────────────────────────────────────

    /// Golden vector from the Web3 Secret Storage V3 test suite, re-addressed to
    /// TRON. Password is `testpassword`.
    const PBKDF2_JSON: &str = r#"{
        "address": "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH",
        "crypto": {
            "cipher": "aes-128-ctr",
            "ciphertext": "5318b4d5bcd28de64ee5559e671353e16f075ecae9f99c7a79a38af5f869aa46",
            "cipherparams": { "iv": "6087dab2f9fdbbfaddc31a909735c1e6" },
            "kdf": "pbkdf2",
            "kdfparams": {
                "c": 262144,
                "dklen": 32,
                "prf": "hmac-sha256",
                "salt": "ae3cd4e7013836a3df6bd7241b12db061dbe2c6785853cce422d148a624ce0bd"
            },
            "mac": "517ead924a9d0dc3124507e3393d175ce3ff7c1e96529c6c555ce9e51205e9b2"
        },
        "id": "3198bc9c-6672-5ab3-d995-4942343ae5b6",
        "version": 3
    }"#;

    #[test]
    fn decrypts_pbkdf2_keystore() {
        let ks: KeystoreFile = serde_json::from_str(PBKDF2_JSON).unwrap();
        let key = decrypt(&ks, "testpassword").unwrap();
        assert_eq!(
            hex::encode(key),
            "7a28b5ba57c53603b0b07b56bba752f7784bf506fa95edc395f5cf6c7514fe9d"
        );
    }

    #[test]
    fn rejects_unsupported_pbkdf2_prf() {
        let mut ks: KeystoreFile = serde_json::from_str(PBKDF2_JSON).unwrap();
        let KdfparamsType::Pbkdf2 { c, dklen, salt, .. } = ks.crypto.kdfparams.clone() else {
            unreachable!("fixture uses pbkdf2 params")
        };
        ks.crypto.kdfparams = KdfparamsType::Pbkdf2 { c, dklen, prf: "hmac-sha512".into(), salt };
        let err = decrypt(&ks, "testpassword").unwrap_err();
        assert!(err.to_string().contains("prf"), "got: {err}");
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
