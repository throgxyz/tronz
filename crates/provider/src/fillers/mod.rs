//! Transaction fillers — composable units that populate a
//! [`TransactionRequest`] before signing.
//!
//! Modeled on alloy's `TxFiller` / `JoinFill` pattern.

#![allow(clippy::manual_async_fn, reason = "explicit RPIT preserves the required Send bound")]

use core::future::Future;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
    time::{Duration, Instant},
};

use tokio::sync::Mutex;
use tronz_primitives::{Address, B256, RecoverableSignature, Trx};
use tronz_signer::{SignerError, TronNetworkWallet};

use crate::{
    error::Result,
    provider::TronProvider,
    types::{BlockInfo, TransactionRequest},
};

/// A composable transaction filler.
///
/// Each filler decides for itself whether it has anything to do, by inspecting
/// the request it is handed. There is no separate readiness query: unlike
/// alloy's `FillerControlFlow`, TRON has no filler whose work unlocks another's,
/// so the provider drives the chain in a single pass rather than a loop.
pub trait TxFiller: Clone + Send + Sync {
    /// Fill fields that are available synchronously (no network).
    fn fill_sync(&self, _tx: &mut TransactionRequest) {}

    /// Fill fields that require a network round-trip.
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

impl TxFiller for Identity {}

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

    fn fill(
        &self,
        tx: TransactionRequest,
        provider: &impl TronProvider,
    ) -> impl Future<Output = Result<TransactionRequest>> + Send {
        // Only the async half is chained here. The provider brackets the whole
        // chain with `fill_sync`, so repeating it per side would just run every
        // sync filler several times over.
        async move {
            let tx = self.left.fill(tx, provider).await?;
            self.right.fill(tx, provider).await
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
        Self { expiry, ..Self::new() }
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

            // Keep the async mutex through a cache miss so concurrent callers
            // coalesce into one get_now_block request instead of stampeding the
            // node when the TTL expires.
            let mut cache = cached.lock().await;
            let block = if let Some((block, fetched_at)) = cache.as_ref() {
                if fetched_at.elapsed() < block_ttl {
                    block.clone()
                } else {
                    let block = provider.get_now_block().await?;
                    *cache = Some((block.clone(), Instant::now()));
                    block
                }
            } else {
                let block = provider.get_now_block().await?;
                *cache = Some((block.clone(), Instant::now()));
                block
            };
            drop(cache);

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

/// What an [`EnergyFiller`] does when it cannot find out what a call needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnEstimateFailure {
    /// Fail the send.
    ///
    /// The default. A limit picked without knowing the price is a guess either way:
    /// too low and the call runs out of energy part-way, reverting while the energy
    /// it burned is still gone, and too high and the limit has stopped capping
    /// anything. Better to say the price is unknown than to send on a guess.
    #[default]
    Fail,

    /// Send with [`EnergyFiller::with_fallback`]'s limit.
    ///
    /// For a caller who would rather a transaction go out on an old-fashioned flat
    /// limit than not go out at all.
    UseFallback,
}

/// Sizes `fee_limit` from what a contract call would actually cost.
///
/// `fee_limit` caps the TRX a call may burn for energy. Too low and the call runs
/// out mid-execution, reverting while the energy it did use is still burned; too
/// high and the cap stops protecting the sender. Neither is a number that can be
/// picked once, so this filler asks the chain what the call needs, adds a margin,
/// and clamps the result.
///
/// It defers to a `fee_limit` already on the request, and leaves alone any operation
/// that does not need one.
///
/// # Where the estimate comes from
///
/// The node's own `EstimateEnergy` is asked first. Nodes must opt into that one, so
/// where it is switched off the filler runs the call read-only instead and takes the
/// energy that consumed — less exact, but available everywhere. A refusal is
/// remembered, so a node that does not offer it is only asked once.
///
/// # When the chain cannot answer
///
/// A deployment cannot be estimated at all — there is no contract to call yet — so it
/// is funded with [`with_fallback`](Self::with_fallback).
///
/// Anything else is a failure rather than a known limit: a call that reverts
/// read-only, a node that cannot be reached, an unreadable price schedule. By default
/// these fail the send, so a transaction is never broadcast on a guessed limit.
/// [`on_estimate_failure`](Self::on_estimate_failure) trades that for the fallback.
///
/// The energy price is cached for [`with_price_ttl`](Self::with_price_ttl), since it
/// moves through governance rather than per block.
#[derive(Clone, Debug)]
pub struct EnergyFiller {
    margin_percent: u32,
    min: Trx,
    max: Trx,
    fallback: Trx,
    on_failure: OnEstimateFailure,
    price_ttl: Duration,
    price: Arc<Mutex<Option<(i64, Instant)>>>,
    /// Set once a node turns out not to offer `EstimateEnergy`.
    node_will_not_estimate: Arc<AtomicBool>,
}

impl Default for EnergyFiller {
    fn default() -> Self {
        Self::new()
    }
}

impl EnergyFiller {
    /// A filler with a 20% margin, no floor, a 1000 TRX ceiling, a 20 TRX fallback,
    /// and a 5-minute price cache.
    pub fn new() -> Self {
        Self {
            margin_percent: 20,
            min: Trx::ZERO,
            max: Trx::from_sun_unchecked(1_000_000_000),
            fallback: Trx::from_sun_unchecked(20_000_000),
            on_failure: OnEstimateFailure::Fail,
            price_ttl: Duration::from_secs(300),
            price: Arc::new(Mutex::new(None)),
            node_will_not_estimate: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Add `percent` on top of the estimate, covering a call that costs more than
    /// the estimate said. Zero funds the estimate exactly.
    pub const fn with_margin(mut self, percent: u32) -> Self {
        self.margin_percent = percent;
        self
    }

    /// Keep the limit inside `min ..= max`, whatever the estimate comes to.
    ///
    /// # Panics
    ///
    /// If `min` is above `max`. Use [`try_with_bounds`](Self::try_with_bounds) for
    /// bounds that come from somewhere other than the source, such as a config file.
    pub fn with_bounds(self, min: Trx, max: Trx) -> Self {
        self.try_with_bounds(min, max).expect("fee limit bounds are inverted")
    }

    /// [`with_bounds`](Self::with_bounds), reporting inverted bounds rather than
    /// panicking on them.
    pub fn try_with_bounds(mut self, min: Trx, max: Trx) -> Result<Self> {
        if min > max {
            return Err(crate::Error::local_usage_str(&format!(
                "fee limit bounds are inverted: {min} is above {max}"
            )));
        }

        self.min = min;
        self.max = max;
        Ok(self)
    }

    /// The limit for an operation the chain cannot price, and for a failed estimate
    /// under [`OnEstimateFailure::UseFallback`].
    pub const fn with_fallback(mut self, fallback: Trx) -> Self {
        self.fallback = fallback;
        self
    }

    /// Whether a failed estimate fails the send, or falls back to a flat limit.
    pub const fn on_estimate_failure(mut self, on_failure: OnEstimateFailure) -> Self {
        self.on_failure = on_failure;
        self
    }

    /// How long a fetched energy price is reused. [`Duration::ZERO`] fetches it for
    /// every call.
    pub const fn with_price_ttl(mut self, ttl: Duration) -> Self {
        self.price_ttl = ttl;
        self
    }

    /// Energy price in sun, from the cache or the node.
    async fn energy_price(&self, provider: &impl TronProvider) -> Result<i64> {
        let mut cached = self.price.lock().await;
        if let Some((price, fetched)) = *cached
            && fetched.elapsed() < self.price_ttl
        {
            return Ok(price);
        }

        let price = provider.get_energy_price().await?;
        *cached = Some((price, Instant::now()));
        Ok(price)
    }

    /// Energy the call needs, from whichever estimator the node offers.
    async fn energy_needed(
        &self,
        call: &crate::types::TriggerSmartContract,
        provider: &impl TronProvider,
    ) -> Result<i64> {
        if !self.node_will_not_estimate.load(AtomicOrdering::Relaxed) {
            match provider.estimate_energy(call.clone()).await {
                Ok(energy) => return Ok(energy),
                // Only a node that will never answer is written off. A timeout or a
                // rate limit says nothing about the endpoint, and giving up on it for
                // the life of the provider would cost every later call its estimate.
                Err(err) if is_unsupported(&err) => {
                    self.node_will_not_estimate.store(true, AtomicOrdering::Relaxed);
                }
                // Worth a second chance through the read-only call, which may be all
                // that is wrong.
                Err(_) => {}
            }
        }

        let result = provider.call_contract(call.clone()).await?;

        // The energy burned up to a revert is not what a working call would cost, and
        // a call that reverts read-only is unlikely to be worth funding.
        if let Some(reason) = result.revert_reason {
            return Err(crate::Error::NodeError(format!(
                "the call reverts read-only, so its energy cannot be estimated: {reason}"
            )));
        }

        Ok(result.energy_used)
    }

    /// What the call would cost.
    async fn estimate(
        &self,
        call: &crate::types::TriggerSmartContract,
        provider: &impl TronProvider,
    ) -> Result<Trx> {
        let energy = self.energy_needed(call, provider).await?;
        let price = self.energy_price(provider).await?;

        // Checked, because energy and price both come from the node: no product of
        // theirs and the margin should be able to wrap around. Clamped before
        // narrowing, so the bounds hold whatever it came to.
        let scale = 100 + i128::from(self.margin_percent);
        let sun = (energy >= 0 && price >= 0)
            .then(|| i128::from(energy).checked_mul(i128::from(price))?.checked_mul(scale))
            .flatten()
            .ok_or_else(|| {
                crate::Error::transport(crate::TransportErrorKind::Malformed(format!(
                    "node priced the call at {energy} energy times {price} sun"
                )))
            })?;

        let clamped =
            (sun / 100).clamp(i128::from(self.min.as_sun()), i128::from(self.max.as_sun()));

        Trx::from_sun(clamped as i64).map_err(crate::Error::local_usage)
    }
}

/// Whether the node said it does not serve the call, rather than that the call failed.
fn is_unsupported(err: &crate::Error) -> bool {
    matches!(err.as_transport_err(), Some(crate::TransportErrorKind::Unsupported(_)))
}

impl TxFiller for EnergyFiller {
    fn fill(
        &self,
        tx: TransactionRequest,
        provider: &impl TronProvider,
    ) -> impl Future<Output = Result<TransactionRequest>> + Send {
        async move {
            let mut tx = tx;
            if tx.fee_limit.is_some() || !tx.contract_needs_fee_limit() {
                return Ok(tx);
            }

            // Anything but a call to an existing contract — a deployment, say — the
            // chain cannot price at all.
            let Some(crate::types::ContractType::TriggerSmartContract(call)) = tx.contract.as_ref()
            else {
                tx.fee_limit = Some(self.fallback);
                return Ok(tx);
            };

            let limit = match self.estimate(call, provider).await {
                Ok(limit) => limit,
                Err(err) => match self.on_failure {
                    OnEstimateFailure::Fail => return Err(err),
                    OnEstimateFailure::UseFallback => self.fallback,
                },
            };

            tx.fee_limit = Some(limit);
            Ok(tx)
        }
    }
}

/// Carries a wallet for local transaction signing.
#[derive(Clone, Debug)]
pub struct WalletFiller<W> {
    wallet: W,
    strict: bool,
}

impl<W> WalletFiller<W> {
    /// Wrap a wallet, falling back to its default credential when the wallet
    /// holds no key for a transaction's owner.
    pub const fn new(wallet: W) -> Self {
        Self { wallet, strict: false }
    }

    /// Require the owner's own key, instead of falling back to the default
    /// credential when the wallet does not hold it.
    ///
    /// The fallback exists because a TRON account can authorize another
    /// account's key through an active permission, so signing with the default
    /// credential is often correct. Turn it off when every owner you send for is
    /// supposed to be in the wallet and a miss means a bug.
    pub const fn strict(mut self) -> Self {
        self.strict = true;
        self
    }

    /// Whether the owner's own key is required. See [`strict`](Self::strict).
    pub const fn is_strict(&self) -> bool {
        self.strict
    }

    /// Borrow the wrapped wallet.
    pub const fn wallet(&self) -> &W {
        &self.wallet
    }
}

impl<W> AsRef<W> for WalletFiller<W> {
    fn as_ref(&self) -> &W {
        &self.wallet
    }
}

impl<W> AsMut<W> for WalletFiller<W> {
    fn as_mut(&mut self) -> &mut W {
        &mut self.wallet
    }
}

impl<W: TronNetworkWallet + Clone> TxFiller for WalletFiller<W> {
    // Intentionally a no-op. On TRON the node assembles the transaction, so the
    // bytes to sign do not exist until after the request has been sent; there is
    // nothing for a request-phase filler to sign. The provider reaches the
    // wallet through [`HasSigner`] once it holds the built transaction.
}

/// Provides signing access through a filler chain.
pub trait HasSigner {
    /// The default signing address of the attached wallet, if any.
    ///
    /// Defaults to `None`, which is the answer for any filler that does not carry a
    /// wallet — most of them.
    fn signer_address(&self) -> Option<Address> {
        None
    }

    /// Sign with `key` when available, otherwise with the default credential.
    ///
    /// Returns `None` when the chain contains no wallet, as it does by default.
    fn sign_with(
        &self,
        key: Option<Address>,
        hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        let _ = (key, hash);
        async { None }
    }

    /// Sign `hash` with the wallet's default credential.
    fn sign(
        &self,
        hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        self.sign_with(None, hash)
    }
}

impl HasSigner for Identity {}

impl HasSigner for TaposFiller {}

impl HasSigner for EnergyFiller {}

impl HasSigner for FeeLimitFiller {}

impl<W: TronNetworkWallet + Clone> HasSigner for WalletFiller<W> {
    fn signer_address(&self) -> Option<Address> {
        Some(self.wallet.default_signer_address())
    }

    fn sign_with(
        &self,
        key: Option<Address>,
        hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        let wallet = self.wallet.clone();
        let strict = self.strict;
        async move {
            // In strict mode the requested key is used as-is, so an unheld owner
            // surfaces as the wallet's own "missing credential" error.
            let key = match key {
                Some(owner) if strict || wallet.has_signer_for(&owner) => owner,
                _ => wallet.default_signer_address(),
            };
            Some(wallet.sign_hash_with(key, &hash).await)
        }
    }
}

impl<L: HasSigner + Clone + Send, R: HasSigner + Clone + Send> HasSigner for JoinFill<L, R> {
    fn signer_address(&self) -> Option<Address> {
        self.right.signer_address().or_else(|| self.left.signer_address())
    }

    fn sign_with(
        &self,
        key: Option<Address>,
        hash: B256,
    ) -> impl Future<Output = Option<Result<RecoverableSignature, SignerError>>> + Send {
        let left = self.left.clone();
        let right = self.right.clone();
        async move {
            if let Some(result) = right.sign_with(key, hash).await {
                Some(result)
            } else {
                left.sign_with(key, hash).await
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
        types::{
            BlockInfo, ConstantCallResult, ContractType, TransferContract, TriggerSmartContract,
        },
    };

    fn addr(b: u8) -> Address {
        Address::from_evm_bytes({
            let mut a = [0u8; 20];
            a[19] = b;
            a
        })
    }

    fn block(num: i64, ts: i64) -> BlockInfo {
        BlockInfo::new(num, B256::ZERO, ts)
    }
    fn mock_provider() -> (RootProvider, MockTransport) {
        let mock = MockTransport::new();
        (RootProvider::new(mock.clone()), mock)
    }

    #[tokio::test]
    async fn tapos_filler_fills_from_block() {
        let (provider, mock) = mock_provider();
        mock.push_ok("get_now_block", block(0x0011_2233_4455_6677, 1_000_000));

        let filler = TaposFiller::new();
        let tx = TransactionRequest::default();
        let filled = filler.fill(tx, &provider).await.unwrap();
        assert_eq!(filled.ref_block_bytes, Some([0x66, 0x77]));
        assert_eq!(filled.timestamp, Some(1_000_000));
        assert_eq!(filled.expiration, Some(1_000_000 + 300_000)); // default 300 s
    }

    #[tokio::test]
    async fn tapos_filler_skips_already_filled() {
        let (provider, _mock) = mock_provider();

        let filler = TaposFiller::new();
        let tx = TransactionRequest { ref_block_bytes: Some([0xaa, 0xbb]), ..Default::default() };
        let filled = filler.fill(tx, &provider).await.unwrap();
        assert_eq!(filled.ref_block_bytes, Some([0xaa, 0xbb]));
    }

    #[tokio::test]
    async fn tapos_filler_reuses_cached_block() {
        let (provider, mock) = mock_provider();
        mock.push_ok("get_now_block", block(0x0011_2233_4455_6677, 2_000_000));

        let filler = TaposFiller::new(); // default TTL = 3 s
        let filled1 = filler.fill(TransactionRequest::default(), &provider).await.unwrap();
        let filled2 = filler.fill(TransactionRequest::default(), &provider).await.unwrap();

        assert_eq!(filled1.ref_block_bytes, filled2.ref_block_bytes);
        assert_eq!(filled1.timestamp, filled2.timestamp);
    }

    #[tokio::test]
    async fn tapos_filler_cache_shared_across_clones() {
        let (provider, mock) = mock_provider();
        mock.push_ok("get_now_block", block(0x0011_2233_4455_6677, 3_000_000));

        let filler = TaposFiller::new();
        let clone = filler.clone();
        filler.fill(TransactionRequest::default(), &provider).await.unwrap();
        clone.fill(TransactionRequest::default(), &provider).await.unwrap();
    }

    #[tokio::test]
    async fn tapos_filler_coalesces_concurrent_cache_misses() {
        let (provider, mock) = mock_provider();
        mock.push_ok("get_now_block", block(7, 4_000_000));

        let filler = TaposFiller::new();
        let clone = filler.clone();
        let (first, second) = tokio::join!(
            filler.fill(TransactionRequest::default(), &provider),
            clone.fill(TransactionRequest::default(), &provider),
        );

        assert_eq!(first.unwrap().timestamp, Some(4_000_000));
        assert_eq!(second.unwrap().timestamp, Some(4_000_000));
    }

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

    #[test]
    fn wallet_filler_exposes_address() {
        use tronz_signer::{LocalSigner, TronWallet};
        let signer = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let expected = signer.address();
        let filler = WalletFiller::new(TronWallet::new(signer));
        assert_eq!(filler.signer_address(), Some(expected));
        assert_eq!(filler.wallet().default_signer_address(), expected);
    }

    #[tokio::test]
    async fn wallet_filler_signs_with_the_credential_named_by_key() {
        use tronz_signer::{LocalSigner, TronWallet};
        let default = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let secondary = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000002",
        )
        .unwrap();

        let mut wallet = TronWallet::new(default.clone());
        wallet.register_signer(secondary.clone());
        let filler = WalletFiller::new(wallet);

        let hash = B256::repeat_byte(9);
        let by_default = filler.sign_with(None, hash).await.unwrap().unwrap();
        let by_key = filler.sign_with(Some(secondary.address()), hash).await.unwrap().unwrap();

        assert_eq!(by_default.recover_address_from_prehash(hash).unwrap(), default.address());
        assert_eq!(by_key.recover_address_from_prehash(hash).unwrap(), secondary.address());
    }

    #[tokio::test]
    async fn wallet_filler_falls_back_to_the_default_credential_for_an_unheld_owner() {
        use tronz_signer::{LocalSigner, TronWallet};
        let signer = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let multisig_owner = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000002",
        )
        .unwrap()
        .address();
        let filler = WalletFiller::new(TronWallet::new(signer.clone()));

        let hash = B256::repeat_byte(9);
        let sig = filler.sign_with(Some(multisig_owner), hash).await.unwrap().unwrap();
        assert_eq!(sig.recover_address_from_prehash(hash).unwrap(), signer.address());
    }

    #[tokio::test]
    async fn a_strict_wallet_filler_refuses_to_substitute_for_an_unheld_owner() {
        use tronz_signer::{LocalSigner, TronWallet};
        let signer = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let unheld = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000002",
        )
        .unwrap()
        .address();
        let filler = WalletFiller::new(TronWallet::new(signer.clone())).strict();
        assert!(filler.is_strict());

        let err = filler.sign_with(Some(unheld), B256::repeat_byte(9)).await.unwrap().unwrap_err();
        assert!(err.to_string().contains("missing signing credential"));
        let hash = B256::repeat_byte(9);
        let sig = filler.sign_with(None, hash).await.unwrap().unwrap();
        assert_eq!(sig.recover_address_from_prehash(hash).unwrap(), signer.address());
    }

    #[tokio::test]
    async fn wallet_filler_errors_when_the_wallet_holds_no_credentials() {
        use tronz_signer::TronWallet;
        let filler = WalletFiller::new(TronWallet::default());

        let result = filler.sign_with(None, B256::ZERO).await.unwrap();
        assert!(result.unwrap_err().to_string().contains("missing signing credential"));
    }

    #[test]
    fn join_fill_prefers_right_signer() {
        use tronz_signer::{LocalSigner, TronWallet};
        let signer = LocalSigner::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let expected = signer.address();
        let join = JoinFill::new(TaposFiller::new(), WalletFiller::new(TronWallet::new(signer)));
        assert_eq!(join.signer_address(), Some(expected));
    }

    #[test]
    fn join_fill_no_signer_when_both_none() {
        let join = JoinFill::new(TaposFiller::new(), FeeLimitFiller::new(Trx::ZERO));
        assert_eq!(join.signer_address(), None);
    }
    async fn run_pipeline(
        filler: &impl TxFiller,
        mut tx: TransactionRequest,
        provider: &impl TronProvider,
    ) -> TransactionRequest {
        filler.fill_sync(&mut tx);
        let mut tx = filler.fill(tx, provider).await.unwrap();
        filler.fill_sync(&mut tx);
        tx
    }

    #[tokio::test]
    async fn the_fill_pipeline_runs_tapos_and_fee_limit() {
        let (provider, mock) = mock_provider();
        mock.push_ok("get_now_block", block(0x0011_2233_4455_6677, 1_000_000));

        let limit = Trx::from_sun_unchecked(10_000_000);
        let join = JoinFill::new(TaposFiller::new(), FeeLimitFiller::new(limit));
        let tx = TransactionRequest::default().with_contract(ContractType::TriggerSmartContract(
            TriggerSmartContract {
                owner_address: addr(1),
                contract_address: addr(2),
                call_value: Trx::ZERO,
                data: Default::default(),
                call_token_value: Trx::ZERO,
                token_id: 0,
            },
        ));
        let filled = run_pipeline(&join, tx, &provider).await;
        assert_eq!(filled.ref_block_bytes, Some([0x66, 0x77]));
        assert_eq!(filled.fee_limit, Some(limit));
    }
    #[derive(Clone, Default)]
    struct CountingFiller(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl CountingFiller {
        fn count(&self) -> usize {
            self.0.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl TxFiller for CountingFiller {
        fn fill_sync(&self, _tx: &mut TransactionRequest) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    #[tokio::test]
    async fn a_sync_filler_runs_once_per_pass_regardless_of_join_depth() {
        let (provider, _mock) = mock_provider();
        let (outer, inner) = (CountingFiller::default(), CountingFiller::default());
        let join = JoinFill::new(JoinFill::new(Identity, inner.clone()), outer.clone());

        run_pipeline(&join, TransactionRequest::default(), &provider).await;
        assert_eq!((outer.count(), inner.count()), (2, 2));
    }
    fn unsupported() -> crate::TransportErrorKind {
        crate::TransportErrorKind::Unsupported("estimate energy is not enabled".into())
    }

    fn trigger_request() -> TransactionRequest {
        TransactionRequest::default().with_contract(ContractType::TriggerSmartContract(
            TriggerSmartContract {
                owner_address: addr(1),
                contract_address: addr(2),
                call_value: Trx::ZERO,
                data: Default::default(),
                call_token_value: Trx::ZERO,
                token_id: 0,
            },
        ))
    }
    fn estimating(energy: i64) -> (RootProvider, MockTransport) {
        let (provider, mock) = mock_provider();
        mock.push_ok("estimate_energy", energy);
        mock.push_ok("get_energy_prices", "0:100,1613044800000:420".to_owned());
        (provider, mock)
    }
    fn constant_only(energy_used: i64) -> (RootProvider, MockTransport) {
        let (provider, mock) = mock_provider();
        mock.push_err::<i64>("estimate_energy", unsupported());
        mock.push_ok(
            "trigger_constant_contract",
            ConstantCallResult { output: Default::default(), energy_used, revert_reason: None },
        );
        mock.push_ok("get_energy_prices", "0:100,1613044800000:420".to_owned());
        (provider, mock)
    }

    #[tokio::test]
    async fn energy_filler_prices_the_estimate_the_node_gives() {
        let (provider, _mock) = estimating(30_000);

        let filled =
            EnergyFiller::new().with_margin(0).fill(trigger_request(), &provider).await.unwrap();
        assert_eq!(filled.fee_limit, Some(Trx::from_sun_unchecked(12_600_000)));
    }

    #[tokio::test]
    async fn energy_filler_falls_back_to_a_read_only_call_where_the_node_will_not_estimate() {
        let (provider, _mock) = constant_only(30_000);

        let filled =
            EnergyFiller::new().with_margin(0).fill(trigger_request(), &provider).await.unwrap();

        assert_eq!(filled.fee_limit, Some(Trx::from_sun_unchecked(12_600_000)));
    }

    #[tokio::test]
    async fn energy_filler_asks_an_unwilling_node_only_once() {
        let (provider, mock) = constant_only(1_000);
        mock.push_ok(
            "trigger_constant_contract",
            ConstantCallResult {
                output: Default::default(),
                energy_used: 1_000,
                revert_reason: None,
            },
        );

        let filler = EnergyFiller::new().with_margin(0);
        let first = filler.fill(trigger_request(), &provider).await.unwrap();
        let second = filler.fill(trigger_request(), &provider).await.unwrap();

        assert_eq!(first.fee_limit, second.fee_limit);
        assert_eq!(first.fee_limit, Some(Trx::from_sun_unchecked(420_000)));
    }

    #[tokio::test]
    async fn energy_filler_applies_the_margin_and_the_ceiling() {
        let (provider, _mock) = estimating(30_000);

        let filled = EnergyFiller::new()
            .with_margin(100)
            .with_bounds(Trx::ZERO, Trx::from_sun_unchecked(20_000_000))
            .fill(trigger_request(), &provider)
            .await
            .unwrap();
        assert_eq!(filled.fee_limit, Some(Trx::from_sun_unchecked(20_000_000)));
    }

    #[tokio::test]
    async fn energy_filler_applies_the_floor() {
        let (provider, _mock) = estimating(10);
        let floor = Trx::from_sun_unchecked(1_000_000);

        let filled = EnergyFiller::new()
            .with_bounds(floor, Trx::from_sun_unchecked(20_000_000))
            .fill(trigger_request(), &provider)
            .await
            .unwrap();

        assert_eq!(filled.fee_limit, Some(floor));
    }

    #[test]
    #[should_panic(expected = "bounds are inverted")]
    fn energy_filler_rejects_inverted_bounds() {
        let _ = EnergyFiller::new()
            .with_bounds(Trx::from_sun_unchecked(100), Trx::from_sun_unchecked(10));
    }

    #[tokio::test]
    async fn energy_filler_survives_a_margin_that_would_overflow() {
        let (provider, _mock) = estimating(i64::MAX / 1_000);

        let filled = EnergyFiller::new()
            .with_margin(u32::MAX)
            .fill(trigger_request(), &provider)
            .await
            .unwrap();
        assert_eq!(filled.fee_limit, Some(Trx::from_sun_unchecked(1_000_000_000)));
    }
    #[tokio::test]
    async fn energy_filler_keeps_asking_a_node_that_merely_failed() {
        let (provider, mock) = mock_provider();
        mock.push_err::<i64>(
            "estimate_energy",
            crate::TransportErrorKind::Malformed("timed out".into()),
        );
        mock.push_ok(
            "trigger_constant_contract",
            ConstantCallResult {
                output: Default::default(),
                energy_used: 1_000,
                revert_reason: None,
            },
        );
        mock.push_ok("get_energy_prices", "0:100,1613044800000:420".to_owned());
        mock.push_ok("estimate_energy", 2_000i64);

        let filler = EnergyFiller::new().with_margin(0);
        let first = filler.fill(trigger_request(), &provider).await.unwrap();
        let second = filler.fill(trigger_request(), &provider).await.unwrap();

        assert_eq!(first.fee_limit, Some(Trx::from_sun_unchecked(420_000)));
        assert_eq!(second.fee_limit, Some(Trx::from_sun_unchecked(840_000)));
    }
    #[tokio::test]
    async fn energy_filler_rejects_numbers_no_chain_would_produce() {
        let (provider, mock) = mock_provider();
        mock.push_ok("estimate_energy", i64::MAX);
        mock.push_ok("get_energy_prices", format!("0:{}", i64::MAX));

        let err = EnergyFiller::new()
            .with_margin(u32::MAX)
            .fill(trigger_request(), &provider)
            .await
            .expect_err("no limit can be read out of those numbers");

        assert!(err.to_string().contains("priced the call at"), "{err}");
    }

    #[tokio::test]
    async fn energy_filler_leaves_an_explicit_limit_alone() {
        let (provider, _mock) = mock_provider();
        let asked = Trx::from_sun_unchecked(5_000_000);

        let filled = EnergyFiller::new()
            .fill(trigger_request().with_fee_limit(asked), &provider)
            .await
            .unwrap();
        assert_eq!(filled.fee_limit, Some(asked));
    }
    #[tokio::test]
    async fn energy_filler_refuses_to_price_a_call_that_reverts() {
        let (provider, mock) = mock_provider();
        mock.push_err::<i64>("estimate_energy", unsupported());
        mock.push_ok(
            "trigger_constant_contract",
            ConstantCallResult {
                output: Default::default(),
                energy_used: 900,
                revert_reason: Some("insufficient balance".to_owned()),
            },
        );

        let err = EnergyFiller::new()
            .fill(trigger_request(), &provider)
            .await
            .expect_err("a reverting call has no meaningful limit");

        assert!(err.to_string().contains("insufficient balance"), "{err}");
    }

    #[tokio::test]
    async fn energy_filler_reports_a_node_that_no_estimator_reached() {
        let (provider, mock) = mock_provider();
        mock.push_err::<i64>("estimate_energy", unsupported());
        mock.push_err::<ConstantCallResult>(
            "trigger_constant_contract",
            crate::TransportErrorKind::Malformed("no constant call either".into()),
        );

        let err = EnergyFiller::new()
            .fill(trigger_request(), &provider)
            .await
            .expect_err("a transaction should not go out on a guess");

        assert!(err.to_string().contains("no constant call either"), "{err}");
    }
    #[tokio::test]
    async fn energy_filler_uses_the_fallback_when_told_to_tolerate_failure() {
        let (provider, mock) = mock_provider();
        mock.push_err::<i64>("estimate_energy", unsupported());
        mock.push_err::<ConstantCallResult>(
            "trigger_constant_contract",
            crate::TransportErrorKind::Malformed("no constant call either".into()),
        );

        let fallback = Trx::from_sun_unchecked(9_000_000);
        let filled = EnergyFiller::new()
            .with_fallback(fallback)
            .on_estimate_failure(OnEstimateFailure::UseFallback)
            .fill(trigger_request(), &provider)
            .await
            .unwrap();

        assert_eq!(filled.fee_limit, Some(fallback));
    }
    #[tokio::test]
    async fn energy_filler_funds_a_deploy_it_cannot_price() {
        let (provider, _mock) = mock_provider();
        let fallback = Trx::from_sun_unchecked(11_000_000);
        let tx = TransactionRequest::default().with_contract(ContractType::CreateSmartContract(
            crate::types::CreateSmartContract {
                owner_address: addr(1),
                bytecode: Default::default(),
                abi: Default::default(),
                call_value: Trx::ZERO,
                consume_user_resource_percent: 100,
                origin_energy_limit: 0,
                name: String::new(),
            },
        ));

        let filled = EnergyFiller::new().with_fallback(fallback).fill(tx, &provider).await.unwrap();

        assert_eq!(filled.fee_limit, Some(fallback));
    }

    #[tokio::test]
    async fn inverted_bounds_can_be_reported_instead_of_panicking() {
        let err = EnergyFiller::new()
            .try_with_bounds(Trx::from_sun_unchecked(100), Trx::from_sun_unchecked(10))
            .expect_err("inverted");

        assert!(err.to_string().contains("inverted"), "{err}");
    }

    #[tokio::test]
    async fn energy_filler_leaves_operations_that_need_no_limit_alone() {
        let (provider, _mock) = mock_provider();
        let tx =
            TransactionRequest::default().with_contract(ContractType::Transfer(TransferContract {
                owner_address: addr(1),
                to_address: addr(2),
                amount: Trx::from_sun_unchecked(1),
            }));

        let filled = EnergyFiller::new().fill(tx, &provider).await.unwrap();

        assert_eq!(filled.fee_limit, None);
    }

    #[tokio::test]
    async fn energy_filler_refetches_the_price_once_its_ttl_is_up() {
        let (provider, mock) = estimating(1_000);
        mock.push_ok("estimate_energy", 1_000i64);
        mock.push_ok("get_energy_prices", "0:100,1613044800000:1000".to_owned());

        let filler = EnergyFiller::new().with_margin(0).with_price_ttl(Duration::ZERO);
        let first = filler.fill(trigger_request(), &provider).await.unwrap();
        let second = filler.fill(trigger_request(), &provider).await.unwrap();

        assert_eq!(first.fee_limit, Some(Trx::from_sun_unchecked(420_000)));
        assert_eq!(second.fee_limit, Some(Trx::from_sun_unchecked(1_000_000)));
    }

    #[tokio::test]
    async fn energy_filler_reuses_a_price_inside_its_ttl() {
        let (provider, mock) = estimating(1_000);
        mock.push_ok("estimate_energy", 1_000i64);

        let filler = EnergyFiller::new().with_margin(0);
        let first = filler.fill(trigger_request(), &provider).await.unwrap();
        let second = filler.fill(trigger_request(), &provider).await.unwrap();

        assert_eq!(first.fee_limit, second.fee_limit);
    }
}
