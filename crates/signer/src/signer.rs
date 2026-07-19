//! The [`TronSigner`] trait and the [`NoSigner`] placeholder.

use core::future::Future;

use tronz_primitives::{Address, B256, RecoverableSignature};

use crate::error::SignerError;

/// A signer capable of producing recoverable secp256k1 signatures over a
/// transaction hash.
///
/// Uses RPITIT (`-> impl Future`) rather than `async_trait` for zero-cost,
/// allocation-free async. See `DESIGN.md` §10 / OQ-1 for object-safety notes.
pub trait TronSigner: Clone + Send + Sync + 'static {
    /// The TRON address that corresponds to this signer's key.
    fn address(&self) -> Address;

    /// Sign a 32-byte transaction hash (`tx_id`), returning a recoverable
    /// signature in TRON's `r || s || v` form.
    fn sign_hash(
        &self,
        hash: B256,
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send;

    /// Sign a plaintext message, TronWeb `signMessageV2`-compatible.
    ///
    /// The returned `v` is `0`/`1`; use `to_legacy_bytes` for TronWeb's `27`/`28`.
    ///
    /// ```
    /// # use tronz_signer::{LocalSigner, TronSigner};
    /// # use tronz_primitives::verify_message;
    /// # async fn example(signer: LocalSigner) -> Result<(), Box<dyn std::error::Error>> {
    /// let sig = signer.sign_message(b"hello world").await?;
    /// assert!(verify_message(b"hello world", &sig, signer.address()));
    /// let _tronweb_sig = sig.to_legacy_bytes();
    /// # Ok(())
    /// # }
    /// ```
    fn sign_message(
        &self,
        message: &[u8],
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
        self.sign_hash(tronz_primitives::hash_message(message))
    }
}

/// Placeholder signer type for providers built without a signer attached.
///
/// It deliberately does **not** implement [`TronSigner`]; its only purpose is
/// to be the default type parameter so that read-only providers compile while
/// `.send()`-style operations remain unavailable until a real signer is added.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSigner;
