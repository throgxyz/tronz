// `SdkError` variants are inherently large; boxing every call site would hurt ergonomics.
#![allow(clippy::result_large_err)]

use core::future::Future;

use aws_sdk_kms::{
    Client,
    error::SdkError,
    operation::{
        get_public_key::{GetPublicKeyError, GetPublicKeyOutput},
        sign::{SignError, SignOutput},
    },
    primitives::Blob,
    types::{MessageType, SigningAlgorithmSpec},
};
use k256::ecdsa::{self, VerifyingKey};
use tracing::{debug, instrument};
use tronz_primitives::{Address, B256, RecoverableSignature};
use tronz_signer::{SignerError, TronSigner};

/// Errors produced by [`AwsSigner`].
#[derive(Debug, thiserror::Error)]
pub enum AwsSignerError {
    /// AWS KMS returned an error during `Sign`.
    #[error(transparent)]
    Sign(#[from] SdkError<SignError>),
    /// AWS KMS returned an error during `GetPublicKey`.
    #[error(transparent)]
    GetPublicKey(#[from] SdkError<GetPublicKeyError>),
    /// Failed to parse a k256 ECDSA signature or key.
    #[error(transparent)]
    K256(#[from] ecdsa::Error),
    /// Failed to parse the DER-encoded SubjectPublicKeyInfo returned by KMS.
    #[error(transparent)]
    Spki(#[from] spki::Error),
    /// KMS response did not contain a public key.
    #[error("public key not found in KMS response")]
    PublicKeyNotFound,
    /// KMS response did not contain a signature.
    #[error("signature not found in KMS response")]
    SignatureNotFound,
    /// Neither recovery parity (0 or 1) produced the expected public key.
    #[error("failed to recover parity from KMS signature — key may not be secp256k1")]
    SignatureRecoveryFailed,
}

impl From<AwsSignerError> for SignerError {
    fn from(e: AwsSignerError) -> Self {
        SignerError::Other(Box::new(e))
    }
}

/// A [`TronSigner`] backed by an AWS KMS secp256k1 key.
///
/// The private key never leaves the AWS HSM. The public key is fetched once on
/// construction and cached; signing is delegated to the KMS `Sign` API.
#[derive(Clone)]
pub struct AwsSigner {
    kms: Client,
    key_id: String,
    pubkey: VerifyingKey,
    address: Address,
}

impl core::fmt::Debug for AwsSigner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AwsSigner")
            .field("key_id", &self.key_id)
            .field("pubkey", &hex::encode(self.pubkey.to_sec1_bytes()))
            .field("address", &self.address)
            .finish()
    }
}

impl AwsSigner {
    /// Create a new signer from an existing KMS [`Client`] and key ID.
    ///
    /// Calls `GetPublicKey` once to derive and cache the TRON address.
    /// The key must be an `ECC_SECG_P256K1` asymmetric signing key.
    #[instrument(skip(kms), err)]
    pub async fn new(kms: Client, key_id: String) -> Result<Self, AwsSignerError> {
        let resp = request_get_pubkey(&kms, key_id.clone()).await?;
        let pubkey = decode_pubkey(resp)?;
        let address = Address::from_public_key(&pubkey);
        debug!(%address, "AWS KMS signer ready");
        Ok(Self { kms, key_id, pubkey, address })
    }

    /// The KMS key ID used by this signer.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Return the cached secp256k1 public key.
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.pubkey
    }

    /// Fetch the public key for a given key ID from KMS.
    pub async fn get_pubkey_for_key(&self, key_id: String) -> Result<VerifyingKey, AwsSignerError> {
        request_get_pubkey(&self.kms, key_id).await.and_then(decode_pubkey)
    }

    /// Fetch the public key for this signer's key ID from KMS.
    pub async fn get_pubkey(&self) -> Result<VerifyingKey, AwsSignerError> {
        self.get_pubkey_for_key(self.key_id.clone()).await
    }

    #[instrument(err, skip(hash), fields(hash = %hash))]
    async fn sign_hash_inner(&self, hash: B256) -> Result<RecoverableSignature, AwsSignerError> {
        let sig = request_sign_digest(&self.kms, self.key_id.clone(), &hash)
            .await
            .and_then(decode_signature)?;
        sig_from_digest_bytes_trial_recovery(sig, &hash, &self.pubkey)
    }
}

impl TronSigner for AwsSigner {
    fn address(&self) -> Address {
        self.address
    }

    fn sign_hash(
        &self,
        hash: B256,
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
        let this = self.clone();
        async move { this.sign_hash_inner(hash).await.map_err(SignerError::from) }
    }
}

#[instrument(skip(kms), err)]
async fn request_get_pubkey(
    kms: &Client,
    key_id: String,
) -> Result<GetPublicKeyOutput, AwsSignerError> {
    kms.get_public_key().key_id(key_id).send().await.map_err(Into::into)
}

#[instrument(skip(kms, digest), fields(digest = %digest), err)]
async fn request_sign_digest(
    kms: &Client,
    key_id: String,
    digest: &B256,
) -> Result<SignOutput, AwsSignerError> {
    kms.sign()
        .key_id(key_id)
        .message(Blob::new(digest.as_slice()))
        .message_type(MessageType::Digest)
        .signing_algorithm(SigningAlgorithmSpec::EcdsaSha256)
        .send()
        .await
        .map_err(Into::into)
}

/// Decode a KMS `GetPublicKey` response (DER SubjectPublicKeyInfo) into a [`VerifyingKey`].
fn decode_pubkey(resp: GetPublicKeyOutput) -> Result<VerifyingKey, AwsSignerError> {
    let raw = resp.public_key.as_ref().ok_or(AwsSignerError::PublicKeyNotFound)?;
    let spki = spki::SubjectPublicKeyInfoRef::try_from(raw.as_ref())?;
    Ok(VerifyingKey::from_sec1_bytes(spki.subject_public_key.raw_bytes())?)
}

/// Decode a KMS `Sign` response and normalize `s` to low-S (required by TRON, as in EIP-2).
fn decode_signature(resp: SignOutput) -> Result<ecdsa::Signature, AwsSignerError> {
    let raw = resp.signature.as_ref().ok_or(AwsSignerError::SignatureNotFound)?;
    let sig = ecdsa::Signature::from_der(raw.as_ref())?;
    Ok(sig.normalize_s().unwrap_or(sig))
}

/// Recover the parity of a KMS signature by trial: KMS omits it, so try `v = 0`
/// then `v = 1` and keep the one that recovers `pubkey`.
fn sig_from_digest_bytes_trial_recovery(
    sig: ecdsa::Signature,
    hash: &B256,
    pubkey: &VerifyingKey,
) -> Result<RecoverableSignature, AwsSignerError> {
    let recid = ecdsa::RecoveryId::new(false, false);
    let candidate = RecoverableSignature::from_signature(&sig, recid);
    if check_candidate(&candidate, hash, pubkey) {
        return Ok(candidate);
    }

    let recid = ecdsa::RecoveryId::new(true, false);
    let candidate = RecoverableSignature::from_signature(&sig, recid);
    if check_candidate(&candidate, hash, pubkey) {
        return Ok(candidate);
    }

    Err(AwsSignerError::SignatureRecoveryFailed)
}

/// Whether `sig` over `hash` recovers to `expected`.
fn check_candidate(sig: &RecoverableSignature, hash: &B256, expected: &VerifyingKey) -> bool {
    sig.split()
        .ok()
        .and_then(|(s, recid)| VerifyingKey::recover_from_prehash(hash.as_slice(), &s, recid).ok())
        .map(|recovered| &recovered == expected)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live test — requires `AWS_KEY_ID` env var and valid AWS credentials.
    /// Run with: `AWS_KEY_ID=<id> cargo test -p tronz-signer-aws -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_sign_and_verify() {
        use aws_config::BehaviorVersion;

        let key_id = std::env::var("AWS_KEY_ID").expect("AWS_KEY_ID must be set");
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        let signer = AwsSigner::new(client, key_id).await.unwrap();
        println!("address: {}", signer.address());

        let hash = B256::repeat_byte(0xab);
        let sig = TronSigner::sign_hash(&signer, hash).await.unwrap();
        assert!(sig.v() == 0 || sig.v() == 1);
        assert_eq!(sig.to_bytes().len(), 65);
    }
}
