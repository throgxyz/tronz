//! In-memory local key signer backed by a `k256` signing key.

use core::future::Future;

use k256::ecdsa::SigningKey;
use tronz_primitives::{Address, B256, RecoverableSignature};

use crate::{error::SignerError, signer::TronSigner};

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

    /// Build from a hex-encoded private key (with or without `0x`).
    pub fn from_hex(s: &str) -> Result<Self, SignerError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s)?;
        let arr: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            SignerError::InvalidKey(format!("expected 32 bytes, got {}", bytes.len()))
        })?;
        Self::from_bytes(&arr)
    }

    /// The underlying `k256` signing key.
    pub fn signing_key(&self) -> &SigningKey {
        &self.key
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

impl TronSigner for LocalSigner {
    fn address(&self) -> Address {
        self.address
    }

    fn sign_hash(
        &self,
        hash: B256,
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
        // Signing is CPU-only here, but the signature is async to match the
        // trait (future signers may hit a network/HSM).
        let key = self.key.clone();
        async move {
            let (sig, recid) = key.sign_prehash_recoverable(hash.as_slice())?;
            Ok(RecoverableSignature::from_signature(&sig, recid))
        }
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
        let sig = signer.sign_hash(B256::repeat_byte(0xab)).await.unwrap();
        assert_eq!(sig.to_bytes().len(), 65);
        assert!(sig.v() == 0 || sig.v() == 1);
        // Round-trips back into k256 components.
        assert!(sig.split().is_ok());
    }

    #[test]
    fn rejects_bad_key() {
        assert!(LocalSigner::from_hex("zz").is_err());
        assert!(LocalSigner::from_hex("01").is_err());
    }
}
