//! In-memory local key signer backed by a `k256` signing key.

use core::str::FromStr;

use async_trait::async_trait;
use k256::ecdsa::SigningKey;
use tronz_primitives::{Address, B256, RecoverableSignature};

use crate::{
    error::SignerError,
    signer::{TronSigner, TronSignerSync},
};

/// A signer that holds a secp256k1 private key in memory.
#[derive(Clone)]
pub struct LocalSigner {
    key: SigningKey,
    address: Address,
}

impl LocalSigner {
    /// Build from a 32-byte private key.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, SignerError> {
        let key = SigningKey::from_bytes(bytes.into())
            .map_err(|e| SignerError::InvalidKey(e.to_string()))?;
        let address = Address::from_public_key(key.verifying_key());
        Ok(Self { key, address })
    }

    /// Build from a 32-byte private key slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, SignerError> {
        let arr: [u8; 32] = bytes.try_into().map_err(|_| {
            SignerError::InvalidKey(format!("expected 32 bytes, got {}", bytes.len()))
        })?;
        Self::from_bytes(&arr)
    }

    /// Build from a hex-encoded private key (with or without `0x`).
    pub fn from_hex(s: &str) -> Result<Self, SignerError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        Self::from_slice(&hex::decode(s)?)
    }

    /// Build from an existing `k256` signing key.
    pub fn from_signing_key(key: SigningKey) -> Self {
        let address = Address::from_public_key(key.verifying_key());
        Self { key, address }
    }

    /// Generate a signer from a new random private key.
    ///
    /// Requires the `rand` feature.
    #[cfg(feature = "rand")]
    pub fn random() -> Self {
        Self::random_with(&mut rand::rng())
    }

    /// Generate a signer from a new random private key using `rng`.
    ///
    /// Requires the `rand` feature.
    #[cfg(feature = "rand")]
    pub fn random_with<R: rand::Rng + rand::CryptoRng>(rng: &mut R) -> Self {
        // `rand` and the `rand_core` that `k256` re-exports are different major
        // versions, so draw the scalar here and resample the vanishingly
        // unlikely values that fall outside the curve order.
        loop {
            let bytes: [u8; 32] = rng.random();
            if let Ok(signer) = Self::from_bytes(&bytes) {
                return signer;
            }
        }
    }

    /// The TRON address that corresponds to this signer's key.
    pub const fn address(&self) -> Address {
        self.address
    }

    /// The underlying `k256` signing key.
    pub const fn signing_key(&self) -> &SigningKey {
        &self.key
    }

    /// The private key as raw bytes.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.key.to_bytes().into()
    }

    /// Encrypt this signer's private key as a Web3 Secret Storage V3 keystore
    /// file and write it to `dir`.
    ///
    /// The file is named `<uuid>.json` and uses standard scrypt parameters
    /// (N = 262 144). Returns the path of the written file.
    ///
    /// Requires the `keystore` feature.
    #[cfg(feature = "keystore")]
    pub fn encrypt_keystore<P: AsRef<std::path::Path>>(
        &self,
        dir: P,
        password: &str,
    ) -> Result<std::path::PathBuf, crate::SignerError> {
        let key_bytes: [u8; 32] = self.key.to_bytes().into();
        crate::keystore::encrypt_to_file(
            &key_bytes,
            &self.address.to_string(),
            password,
            dir,
            &mut rand::rng(),
        )
    }

    /// Load and decrypt a keystore file created by [`Self::encrypt_keystore`].
    ///
    /// Returns [`crate::keystore::KeystoreError::InvalidPassword`] if the
    /// password is wrong.
    ///
    /// Requires the `keystore` feature.
    #[cfg(feature = "keystore")]
    pub fn decrypt_keystore<P: AsRef<std::path::Path>>(
        path: P,
        password: &str,
    ) -> Result<Self, crate::SignerError> {
        let key_bytes = crate::keystore::decrypt_from_file(path, password)?;
        Self::from_bytes(&key_bytes)
    }
}

impl core::fmt::Debug for LocalSigner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print the private key.
        f.debug_struct("LocalSigner").field("address", &self.address).finish_non_exhaustive()
    }
}

impl From<SigningKey> for LocalSigner {
    fn from(key: SigningKey) -> Self {
        Self::from_signing_key(key)
    }
}

impl FromStr for LocalSigner {
    type Err = SignerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl PartialEq for LocalSigner {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address && self.key.to_bytes() == other.key.to_bytes()
    }
}

impl Eq for LocalSigner {}

impl TronSignerSync for LocalSigner {
    fn sign_hash_sync(&self, hash: &B256) -> Result<RecoverableSignature, SignerError> {
        let (sig, recid) = self.key.sign_prehash_recoverable(hash.as_slice())?;
        Ok(RecoverableSignature::from_signature(&sig, recid))
    }
}

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
impl TronSigner for LocalSigner {
    fn address(&self) -> Address {
        self.address
    }

    async fn sign_hash(&self, hash: &B256) -> Result<RecoverableSignature, SignerError> {
        self.sign_hash_sync(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Throwaway test key (do not use anywhere real).
    const KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    #[test]
    fn derives_stable_address() {
        let signer = LocalSigner::from_hex(KEY).unwrap();
        // Address must be deterministic for a given key and well-formed.
        let again = LocalSigner::from_hex(KEY).unwrap();
        assert_eq!(signer.address(), again.address());
        assert_eq!(signer.address().as_bytes()[0], 0x41);
    }

    #[tokio::test]
    async fn signs_hash() {
        let signer = LocalSigner::from_hex(KEY).unwrap();
        let sig = signer.sign_hash(&B256::repeat_byte(0xab)).await.unwrap();
        assert_eq!(sig.to_bytes().len(), 65);
        assert!(sig.v() == 0 || sig.v() == 1);
        // Round-trips back into k256 components.
        assert!(sig.split().is_ok());
    }

    #[test]
    fn rejects_bad_key() {
        assert!(LocalSigner::from_hex("zz").is_err());
        assert!(LocalSigner::from_hex("01").is_err());
        assert!(LocalSigner::from_slice(&[1u8; 31]).is_err());
    }

    #[test]
    fn standard_conversions() {
        let signer = LocalSigner::from_hex(KEY).unwrap();

        assert_eq!(KEY.parse::<LocalSigner>().unwrap(), signer);
        assert_eq!(format!("0x{KEY}").parse::<LocalSigner>().unwrap(), signer);
        assert_eq!(LocalSigner::from_slice(&signer.to_bytes()).unwrap(), signer);
        assert_eq!(LocalSigner::from(signer.signing_key().clone()), signer);
        assert_ne!(LocalSigner::from_hex(&"02".repeat(32)).unwrap(), signer);
    }

    #[cfg(feature = "rand")]
    #[test]
    fn random_keys_are_distinct() {
        let a = LocalSigner::random();
        let b = LocalSigner::random();
        assert_ne!(a, b);
        assert_ne!(a.address(), b.address());
        assert_eq!(a.address().as_bytes()[0], 0x41);
    }

    #[test]
    fn sync_signing_needs_no_runtime() {
        let signer = LocalSigner::from_hex(KEY).unwrap();

        let sig = signer.sign_message_sync(b"hello world").unwrap();
        assert!(tronz_primitives::verify_message(b"hello world", &sig, signer.address()));
        assert!(matches!(sig.to_bytes()[64], 0 | 1));
    }

    #[tokio::test]
    async fn sync_and_async_signing_agree() {
        let signer = LocalSigner::from_hex(KEY).unwrap();
        let hash = B256::repeat_byte(0xab);

        assert_eq!(signer.sign_hash_sync(&hash).unwrap(), signer.sign_hash(&hash).await.unwrap());
        assert_eq!(
            signer.sign_message_sync(b"hello world").unwrap(),
            signer.sign_message(b"hello world").await.unwrap()
        );
    }

    #[tokio::test]
    async fn sign_message_round_trips() {
        use tronz_primitives::{recover_message_address, verify_message};

        let signer = LocalSigner::from_hex(KEY).unwrap();
        let sig = signer.sign_message(b"hello world").await.unwrap();
        assert_eq!(recover_message_address(b"hello world", &sig).unwrap(), signer.address());
        assert!(verify_message(b"hello world", &sig, signer.address()));
        assert!(matches!(sig.to_bytes()[64], 0 | 1));
    }

    #[tokio::test]
    async fn sign_message_matches_tronweb_bytes() {
        // TronWeb `signMessageV2("hello world")` with `KEY`, byte-for-byte.
        const TRONWEB_SIG: &str = "0dc0b53d525e0103a6013061cf18e60cf158809149f2b8994a545af65a7004cb1eeaff560e801ab51b28df5d42549aa024c2aa7e9d34de1e01294b9afb5e6c7e1c";

        let signer = LocalSigner::from_hex(KEY).unwrap();
        let sig = signer.sign_message(b"hello world").await.unwrap();
        assert_eq!(hex::encode(sig.to_legacy_bytes()), TRONWEB_SIG);
    }
}
