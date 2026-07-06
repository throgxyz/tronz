//! Transaction fillers — composable units that populate a
//! [`TransactionRequest`] before signing.
//!
//! Modeled on alloy's `TxFiller` / `JoinFill` pattern (see `DESIGN.md` §5.3).

use core::future::Future;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tronz_primitives::{Address, B256, RecoverableSignature, Trx};
use tronz_signer::{SignerError, TronSigner};

use crate::{
    error::Result,
    provider::TronProvider,
    types::{BlockInfo, TransactionRequest},
};

/// Whether a filler still has work to do for a given request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillerStatus {
    /// All of this filler's fields are already present.
    Ready,
    /// This filler has fields left to fill (sync or async).
    NeedsWork,
    /// This filler is a no-op.
    Finished,
}

/// A composable transaction filler.
pub trait TxFiller: Clone + Send + Sync {
    /// Report whether this filler needs to act on `tx`.
    fn status(&self, _tx: &TransactionRequest) -> FillerStatus {
        FillerStatus::Ready
    }

    /// Fill fields that are available synchronously (no network).
    fn fill_sync(&self, _tx: &mut TransactionRequest) {}

    /// Fill fields that require a network round-trip.
    ///
    /// The explicit `+ Send` bound is required so filler futures can run on a
    /// multi-threaded executor, hence the manual `impl Future` form.
    #[allow(clippy::manual_async_fn)]
    fn fill(
        &self,
        tx: TransactionRequest,
        _provider: &impl TronProvider,
    ) -> impl Future<Output = Result<TransactionRequest>> + Send {
        async move { Ok(tx) }
    }
}

/// The empty filler. Does nothing; the identity element for [`JoinFill`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Identity;

impl TxFiller for Identity {
    fn status(&self, _tx: &TransactionRequest) -> FillerStatus {
        FillerStatus::Finished
    }
}

/// Zero-cost combinator that runs `left` then `right`.
#[derive(Clone, Copy, Debug)]
pub struct JoinFill<L, R> {
    /// The first filler to run.
    pub left: L,
    /// The second filler to run.
    pub right: R,
}

impl<L, R> JoinFill<L, R> {
    /// Combine two fillers.
    pub fn new(left: L, right: R) -> Self {
        Self { left, right }
    }
}

impl<L: TxFiller, R: TxFiller> TxFiller for JoinFill<L, R> {
    fn fill_sync(&self, tx: &mut TransactionRequest) {
        self.left.fill_sync(tx);
        self.right.fill_sync(tx);
    }

    #[allow(clippy::manual_async_fn)]
    fn fill(
        &self,
        tx: TransactionRequest,
        provider: &impl TronProvider,
    ) -> impl Future<Output = Result<TransactionRequest>> + Send {
        async move {
            let mut tx = self.left.fill(tx, provider).await?;
            self.left.fill_sync(&mut tx);
            let mut tx = self.right.fill(tx, provider).await?;
            self.right.fill_sync(&mut tx);
            Ok(tx)
        }
    }
}

/// Fills TAPOS fields (`ref_block_*`, `expiration`, `timestamp`) from the
/// latest block. Required before broadcasting client-built transactions.
///
/// The most-recently-fetched block is cached for [`block_ttl`] (default 3 s,
/// matching TRON's block interval) so that bursts of transactions share a
/// single `get_now_block` round-trip.  All clones of the same filler share
/// the same cache via an inner [`Arc`].
///
/// [`block_ttl`]: TaposFiller::with_block_ttl
#[derive(Clone, Debug)]
pub struct TaposFiller {
    expiry: Duration,
    block_ttl: Duration,
    cached: Arc<Mutex<Option<(BlockInfo, Instant)>>>,
}

impl TaposFiller {
    /// Default 5-minute expiry and 3-second block cache TTL.
    pub fn new() -> Self {
        Self {
            expiry: Duration::from_secs(300),
            block_ttl: Duration::from_secs(3),
            cached: Arc::new(Mutex::new(None)),
        }
    }

    /// Override the transaction expiry window.
    pub fn with_expiry(expiry: Duration) -> Self {
        Self {
            expiry,
            ..Self::new()
        }
    }

    /// Override how long a fetched block is reused before the next
    /// `get_now_block` call.  Set to `Duration::ZERO` to disable caching.
    pub fn with_block_ttl(mut self, ttl: Duration) -> Self {
        self.block_ttl = ttl;
        self
    }
}

impl Default for TaposFiller {
    fn default() -> Self {
        Self::new()
    }
}

impl TxFiller for TaposFiller {
    fn status(&self, tx: &TransactionRequest) -> FillerStatus {
        if tx.ref_block_bytes.is_none() {
            FillerStatus::NeedsWork
        } else {
            FillerStatus::Ready
        }
    }

    fn fill(
        &self,
        tx: TransactionRequest,
        provider: &impl TronProvider,
    ) -> impl Future<Output = Result<TransactionRequest>> + Send {
        let expiry = self.expiry;
        let block_ttl = self.block_ttl;
        let cached = Arc::clone(&self.cached);
        async move {
            // Skip if TAPOS was already filled server-side (e.g. trigger calls).
            if tx.ref_block_bytes.is_some() {
                return Ok(tx);
            }

            // Return the cached block if it is still fresh.  The lock is held
            // only while reading the Option — never across an await point.
            let cached_block = cached
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|(b, t)| (t.elapsed() < block_ttl).then(|| b.clone()));

            let block = match cached_block {
                Some(b) => b,
                None => {
                    let b = provider.get_now_block().await?;
                    *cached.lock().unwrap() = Some((b.clone(), Instant::now()));
                    b
                }
            };

            let mut tx = tx;
            tx.ref_block_bytes = Some(block.ref_block_bytes());
            tx.ref_block_hash = Some(block.ref_block_hash());
            // Use the block's own timestamp as the baseline so that clock skew
            // between the client and the node cannot produce an already-expired
            // transaction.
            let base_ms = block.timestamp;
            tx.timestamp = Some(base_ms);
            tx.expiration = Some(base_ms + expiry.as_secs() as i64 * 1_000);
            Ok(tx)
        }
    }
}

/// Sets a default `fee_limit` for contract operations that require one.
#[derive(Clone, Copy, Debug)]
pub struct FeeLimitFiller {
    default: Trx,
}

impl FeeLimitFiller {
    /// Use `default` as the fee limit when none is set on a contract operation.
    pub fn new(default: Trx) -> Self {
        Self { default }
    }
}

impl TxFiller for FeeLimitFiller {
    fn fill_sync(&self, tx: &mut TransactionRequest) {
        if tx.fee_limit.is_none() && tx.contract_needs_fee_limit() {
            tx.fee_limit = Some(self.default);
        }
    }
}

/// Carries the signer for a provider. Signing itself happens in the provider's
/// `send_transaction`, after filling completes; this filler is a no-op marker.
#[derive(Clone, Copy, Debug)]
pub struct SignerFiller<S> {
    signer: S,
}

impl<S: TronSigner> SignerFiller<S> {
    /// Wrap a signer.
    pub fn new(signer: S) -> Self {
        Self { signer }
    }

    /// Borrow the wrapped signer.
    pub fn signer(&self) -> &S {
        &self.signer
    }
}

impl<S: TronSigner> TxFiller for SignerFiller<S> {
    // Intentionally a no-op: signing is performed by the provider once the
    // request is fully filled.
}

// ── HasSigner ─────────────────────────────────────────────────────────────────

/// Implemented by filler chains that (may) carry a signer.
///
/// All filler types implement this trait. Non-signer fillers return `None` from
/// both methods; [`SignerFiller`] returns the wrapped signer's address and signs.
/// [`JoinFill`] prefers the right branch, then falls back to the left.
///
/// This allows [`FilledProvider`](crate::provider::FilledProvider) to locate the
/// signer at runtime without knowing the exact filler chain type.
pub trait HasSigner {
    /// The TRON address of the attached signer, if any.
    fn signer_address(&self) -> Option<Address>;

    /// Sign `hash` with the attached signer.  Returns `None` when no signer is
    /// present in this filler chain.
    fn sign(
        &self,
        hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send;
}

impl HasSigner for Identity {
    fn signer_address(&self) -> Option<Address> {
        None
    }

    // Returns a trivially Send future; `async fn` syntax would require the
    // trait to use `async fn` too, which conflicts with the explicit `+ Send`
    // bound we need for multi-threaded executor compatibility.
    #[allow(clippy::manual_async_fn)]
    fn sign(
        &self,
        _hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        async { None }
    }
}

impl HasSigner for TaposFiller {
    fn signer_address(&self) -> Option<Address> {
        None
    }

    #[allow(clippy::manual_async_fn)]
    fn sign(
        &self,
        _hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        async { None }
    }
}

impl HasSigner for FeeLimitFiller {
    fn signer_address(&self) -> Option<Address> {
        None
    }

    #[allow(clippy::manual_async_fn)]
    fn sign(
        &self,
        _hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        async { None }
    }
}

impl<S: TronSigner> HasSigner for SignerFiller<S> {
    fn signer_address(&self) -> Option<Address> {
        Some(self.signer.address())
    }

    fn sign(
        &self,
        hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        let signer = self.signer.clone();
        async move { Some(signer.sign_hash(hash).await) }
    }
}

impl<L: HasSigner + Clone + Send, R: HasSigner + Clone + Send> HasSigner for JoinFill<L, R> {
    fn signer_address(&self) -> Option<Address> {
        self.right
            .signer_address()
            .or_else(|| self.left.signer_address())
    }

    fn sign(
        &self,
        hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        let left = self.left.clone();
        let right = self.right.clone();
        async move {
            if let Some(result) = right.sign(hash).await {
                Some(result)
            } else {
                left.sign(hash).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tronz_primitives::B256;

    use super::*;
    use crate::{
        provider::RootProvider,
        transport::mock::MockTransport,
        types::{BlockInfo, ContractType, TransferContract, TriggerSmartContract},
    };

    fn addr(b: u8) -> Address {
        Address::from_evm_bytes({
            let mut a = [0u8; 20];
            a[19] = b;
            a
        })
    }

    fn block(num: i64, ts: i64) -> BlockInfo {
        BlockInfo {
            number: num,
            hash: B256::ZERO,
            timestamp: ts,
        }
    }

    fn mock_provider() -> RootProvider<MockTransport> {
        RootProvider::new(MockTransport::new())
    }

    // ── TaposFiller ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn tapos_filler_fills_from_block() {
        let provider = mock_provider();
        provider
            .transport()
            .push_ok("get_now_block", block(0x0011_2233_4455_6677, 1_000_000));

        let filler = TaposFiller::new();
        let tx = TransactionRequest::default();
        assert_eq!(filler.status(&tx), FillerStatus::NeedsWork);

        let filled = filler.fill(tx, &provider).await.unwrap();
        assert_eq!(filled.ref_block_bytes, Some([0x66, 0x77]));
        assert_eq!(filled.timestamp, Some(1_000_000));
        assert_eq!(filled.expiration, Some(1_000_000 + 300_000)); // default 300 s
    }

    #[tokio::test]
    async fn tapos_filler_skips_already_filled() {
        // No response queued — if fill() called get_now_block, MockTransport would panic.
        let provider = mock_provider();

        let filler = TaposFiller::new();
        let mut tx = TransactionRequest::default();
        tx.ref_block_bytes = Some([0xaa, 0xbb]);
        assert_eq!(filler.status(&tx), FillerStatus::Ready);

        let filled = filler.fill(tx, &provider).await.unwrap();
        assert_eq!(filled.ref_block_bytes, Some([0xaa, 0xbb]));
    }

    #[tokio::test]
    async fn tapos_filler_reuses_cached_block() {
        let provider = mock_provider();
        // Only one response queued — second fill() must hit the cache.
        provider
            .transport()
            .push_ok("get_now_block", block(0x0011_2233_4455_6677, 2_000_000));

        let filler = TaposFiller::new(); // default TTL = 3 s
        let filled1 = filler
            .fill(TransactionRequest::default(), &provider)
            .await
            .unwrap();
        // Second call: MockTransport would panic if it tried to pop a second response.
        let filled2 = filler
            .fill(TransactionRequest::default(), &provider)
            .await
            .unwrap();

        assert_eq!(filled1.ref_block_bytes, filled2.ref_block_bytes);
        assert_eq!(filled1.timestamp, filled2.timestamp);
    }

    #[tokio::test]
    async fn tapos_filler_cache_shared_across_clones() {
        let provider = mock_provider();
        // One response — the clone must share the cache and not re-fetch.
        provider
            .transport()
            .push_ok("get_now_block", block(0x0011_2233_4455_6677, 3_000_000));

        let filler = TaposFiller::new();
        let clone = filler.clone();
        filler
            .fill(TransactionRequest::default(), &provider)
            .await
            .unwrap();
        // Clone shares the Arc — no second push_ok needed.
        clone
            .fill(TransactionRequest::default(), &provider)
            .await
            .unwrap();
    }

    // ── FeeLimitFiller ───────────────────────────────────────────────────────

    #[test]
    fn fee_limit_filler_sets_limit_for_trigger() {
        let limit = Trx::from_sun_unchecked(10_000_000);
        let filler = FeeLimitFiller::new(limit);
        let mut tx = TransactionRequest::default().with_contract(
            ContractType::TriggerSmartContract(TriggerSmartContract {
                owner_address: addr(1),
                contract_address: addr(2),
                call_value: Trx::ZERO,
                data: Default::default(),
                call_token_value: Trx::ZERO,
                token_id: 0,
            }),
        );
        assert!(tx.fee_limit.is_none());
        filler.fill_sync(&mut tx);
        assert_eq!(tx.fee_limit, Some(limit));
    }

    #[test]
    fn fee_limit_filler_skips_when_already_set() {
        let existing = Trx::from_sun_unchecked(5_000_000);
        let filler = FeeLimitFiller::new(Trx::from_sun_unchecked(10_000_000));
        let mut tx = TransactionRequest::default()
            .with_contract(ContractType::TriggerSmartContract(TriggerSmartContract {
                owner_address: addr(1),
                contract_address: addr(2),
                call_value: Trx::ZERO,
                data: Default::default(),
                call_token_value: Trx::ZERO,
                token_id: 0,
            }))
            .with_fee_limit(existing);
        filler.fill_sync(&mut tx);
        assert_eq!(tx.fee_limit, Some(existing));
    }

    #[test]
    fn fee_limit_filler_skips_non_contract_tx() {
        let filler = FeeLimitFiller::new(Trx::from_sun_unchecked(10_000_000));
        let mut tx =
            TransactionRequest::default().with_contract(ContractType::Transfer(TransferContract {
                owner_address: addr(1),
                to_address: addr(2),
                amount: Trx::from_sun_unchecked(1),
            }));
        filler.fill_sync(&mut tx);
        assert!(tx.fee_limit.is_none());
    }
}
