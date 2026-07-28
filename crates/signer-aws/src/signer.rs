#![allow(clippy::result_large_err)]

use async_trait::async_trait;
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

/// Errors thrown by [`AwsSigner`].
#[derive(Debug, thiserror::Error)]
pub enum AwsSignerError {
    /// Thrown when the AWS KMS API returns a signing error.
    #[error(transparent)]
    Sign(#[from] SdkError<SignError>),
    /// Thrown when the AWS KMS API returns an error.
    #[error(transparent)]
    GetPublicKey(#[from] SdkError<GetPublicKeyError>),
    /// [`ecdsa`] error.
    #[error(transparent)]
    K256(#[from] ecdsa::Error),
    /// [`spki`] error.
    #[error(transparent)]
    Spki(#[from] spki::Error),
    /// Thrown when the AWS KMS API returns a response without a public key.
    #[error("public key not found in KMS response")]
    PublicKeyNotFound,
    /// Thrown when the AWS KMS API returns a response without a signature.
    #[error("signature not found in KMS response")]
    SignatureNotFound,
    /// Failed to recover signature parity for the given digest and public key.
    #[error("failed to recover parity from KMS signature — key may not be secp256k1")]
    SignatureRecoveryFailed,
}

impl From<AwsSignerError> for SignerError {
    fn from(e: AwsSignerError) -> Self {
        SignerError::Other(Box::new(e))
    }
}

/// Amazon Web Services Key Management Service (AWS KMS) TRON signer.
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
    /// Instantiate a new signer from an existing [`Client`] and key ID.
    ///
    /// Retrieves the public key from AWS and calculates the TRON address.
    #[instrument(skip(kms), err)]
    pub async fn new(kms: Client, key_id: String) -> Result<Self, AwsSignerError> {
        let resp = request_get_pubkey(&kms, key_id.clone()).await?;
        let pubkey = decode_pubkey(resp)?;
        let address = Address::from_public_key(&pubkey);
        debug!(%address, "AWS KMS signer ready");
        Ok(Self { kms, key_id, pubkey, address })
    }

    /// Returns the KMS key ID used by this signer.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Returns the cached public key.
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.pubkey
    }

    /// Fetch the pubkey associated with a key ID.
    pub async fn get_pubkey_for_key(&self, key_id: String) -> Result<VerifyingKey, AwsSignerError> {
        request_get_pubkey(&self.kms, key_id).await.and_then(decode_pubkey)
    }

    /// Fetch the pubkey associated with this signer's key ID.
    pub async fn get_pubkey(&self) -> Result<VerifyingKey, AwsSignerError> {
        self.get_pubkey_for_key(self.key_id.clone()).await
    }

    /// Sign a digest with the key associated with a key ID.
    pub async fn sign_digest_with_key(
        &self,
        key_id: String,
        digest: &B256,
    ) -> Result<ecdsa::Signature, AwsSignerError> {
        request_sign_digest(&self.kms, key_id, digest).await.and_then(decode_signature)
    }

    /// Sign a digest with this signer's key.
    pub async fn sign_digest(&self, digest: &B256) -> Result<ecdsa::Signature, AwsSignerError> {
        self.sign_digest_with_key(self.key_id.clone(), digest).await
    }

    #[instrument(err, skip(hash), fields(hash = %hash))]
    async fn sign_hash_inner(&self, hash: B256) -> Result<RecoverableSignature, AwsSignerError> {
        let sig = self.sign_digest(&hash).await?;
        sig_from_digest_bytes_trial_recovery(sig, &hash, &self.pubkey)
    }
}

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
impl TronSigner for AwsSigner {
    fn address(&self) -> Address {
        self.address
    }

    async fn sign_hash(&self, hash: &B256) -> Result<RecoverableSignature, SignerError> {
        self.sign_hash_inner(*hash).await.map_err(SignerError::from)
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

/// Decode an AWS KMS Pubkey response.
fn decode_pubkey(resp: GetPublicKeyOutput) -> Result<VerifyingKey, AwsSignerError> {
    let raw = resp.public_key.as_ref().ok_or(AwsSignerError::PublicKeyNotFound)?;
    let spki = spki::SubjectPublicKeyInfoRef::try_from(raw.as_ref())?;
    Ok(VerifyingKey::from_sec1_bytes(spki.subject_public_key.raw_bytes())?)
}

/// Decode an AWS KMS Signature response.
fn decode_signature(resp: SignOutput) -> Result<ecdsa::Signature, AwsSignerError> {
    let raw = resp.signature.as_ref().ok_or(AwsSignerError::SignatureNotFound)?;
    let sig = ecdsa::Signature::from_der(raw.as_ref())?;
    Ok(sig.normalize_s().unwrap_or(sig))
}

/// Recover an rsig from a signature under a known key by trial/error.
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

/// Makes a trial recovery to check whether an rsig corresponds to a known [`VerifyingKey`].
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
        let sig = TronSigner::sign_hash(&signer, &hash).await.unwrap();
        assert_eq!(sig.recover_address_from_prehash(hash).unwrap(), signer.address());

        let message = b"hello from AWS KMS";
        let sig = signer.sign_message(message).await.unwrap();
        assert!(tronz_primitives::verify_message(message, &sig, signer.address()));
    }
}
