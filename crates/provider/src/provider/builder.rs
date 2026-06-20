//! [`ProviderBuilder`] and the [`FilledProvider`] it produces.
//!
//! Mirrors alloy's `ProviderBuilder` + `JoinFill` pattern (see `DESIGN.md` §5.3).

use std::time::Duration;

use tronz_primitives::{Address, Trx};
use tronz_signer::TronSigner;

use crate::{
    error::{Error, Result},
    fillers::{FeeLimitFiller, HasSigner, Identity, JoinFill, SignerFiller, TaposFiller, TxFiller},
    provider::{PendingTransaction, RootProvider, TronProvider},
    transport::{
        TronTransport,
        grpc::{GrpcTransport, GrpcTransportConfig, RetryConfig},
    },
    types::{ContractType, RawTransaction, SignedTransaction, TransactionRequest},
};

/// Accumulates fillers and finally binds a transport to produce a
/// [`FilledProvider`].
///
/// Transport tuning (`connect_timeout` / `request_timeout` / `retry`) is stored
/// as `Option`s; `None` defers to [`GrpcTransportConfig`] defaults.
pub struct ProviderBuilder<F> {
    filler: F,
    api_key: Option<String>,
    connect_timeout: Option<Duration>,
    request_timeout: Option<Duration>,
    retry: Option<RetryConfig>,
    endpoints: Vec<String>,
}

impl ProviderBuilder<Identity> {
    /// Start with no fillers.
    pub fn new() -> Self {
        Self {
            filler: Identity,
            api_key: None,
            connect_timeout: None,
            request_timeout: None,
            retry: None,
            endpoints: Vec::new(),
        }
    }
}

impl Default for ProviderBuilder<Identity> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: TxFiller> ProviderBuilder<F> {
    /// Optionally attach a TronGrid API key.
    ///
    /// Accepts `None` (no-op) or `Some(key)`, so you can pass an
    /// `Option<String>` directly without a `match`:
    ///
    /// ```no_run
    /// use tronz_provider::{ProviderBuilder, transport::grpc::TRONGRID_MAINNET};
    /// # async fn run() -> tronz_provider::Result<()> {
    /// let api_key: Option<String> = std::env::var("TRON_API_KEY").ok();
    /// let provider = ProviderBuilder::new()
    ///     .maybe_api_key(api_key)
    ///     .on_grpc(TRONGRID_MAINNET)
    ///     .await?;
    /// # Ok(()) }
    /// ```
    pub fn maybe_api_key(mut self, key: Option<impl Into<String>>) -> Self {
        self.api_key = key.map(|k| k.into());
        self
    }

    /// Override the connect (handshake) timeout. Default: 10 s.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Override the per-call request timeout (applied to every RPC). Default: 30 s.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    /// Override the retry policy. Default: [`RetryConfig::default`].
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Add equivalent node endpoints for client-side failover / load balancing.
    ///
    /// These join the `uri` passed to [`on_grpc`](Self::on_grpc); with two or
    /// more total endpoints the channel load-balances and fails over across
    /// them (see [`GrpcTransportConfig::endpoints`]).
    pub fn with_endpoints(mut self, endpoints: Vec<String>) -> Self {
        self.endpoints = endpoints;
        self
    }

    /// Add both the TAPOS filler and a 20 TRX default fee-limit filler in one
    /// call — the most common setup for a read/write provider.
    ///
    /// Equivalent to `.with_tapos().with_fee_limit(Trx::from_sun_unchecked(20_000_000))`.
    pub fn with_recommended_fillers(
        self,
    ) -> ProviderBuilder<JoinFill<JoinFill<F, TaposFiller>, FeeLimitFiller>> {
        self.with_tapos()
            .with_fee_limit(Trx::from_sun_unchecked(20_000_000))
    }

    /// Add the TAPOS filler (required before broadcasting client-built txs).
    pub fn with_tapos(self) -> ProviderBuilder<JoinFill<F, TaposFiller>> {
        // Destructure so adding a transport-config field later is a compile
        // error here, not a silently dropped setting.
        let Self {
            filler,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        } = self;
        ProviderBuilder {
            filler: JoinFill::new(filler, TaposFiller::new()),
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        }
    }

    /// Add a default `fee_limit` for contract operations.
    pub fn with_fee_limit(self, limit: Trx) -> ProviderBuilder<JoinFill<F, FeeLimitFiller>> {
        let Self {
            filler,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        } = self;
        ProviderBuilder {
            filler: JoinFill::new(filler, FeeLimitFiller::new(limit)),
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        }
    }

    /// Attach a signer so `.send()` operations work.
    pub fn with_signer<S: TronSigner>(
        self,
        signer: S,
    ) -> ProviderBuilder<JoinFill<F, SignerFiller<S>>> {
        let Self {
            filler,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        } = self;
        ProviderBuilder {
            filler: JoinFill::new(filler, SignerFiller::new(signer)),
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        }
    }

    /// Connect to a TRON gRPC node, applying any API key set via
    /// [`maybe_api_key`](Self::maybe_api_key).
    ///
    /// `uri` examples:
    /// - `"https://grpc.trongrid.io:443"` (TronGrid mainnet, TLS)
    /// - `"http://127.0.0.1:50051"` (local node, plain HTTP/2)
    pub async fn on_grpc(self, uri: impl AsRef<str>) -> Result<FilledProvider<GrpcTransport, F>> {
        let mut cfg = GrpcTransportConfig {
            api_key: self.api_key,
            endpoints: self.endpoints,
            ..Default::default()
        };
        if let Some(t) = self.connect_timeout {
            cfg.connect_timeout = t;
        }
        if let Some(t) = self.request_timeout {
            cfg.request_timeout = t;
        }
        if let Some(r) = self.retry {
            cfg.retry = r;
        }
        let transport = GrpcTransport::connect_with_config(uri, cfg)
            .await
            .map_err(Error::Transport)?;
        Ok(FilledProvider::new(
            RootProvider::new(transport),
            self.filler,
        ))
    }

    /// Connect with an explicit TronGrid API key.
    ///
    /// Equivalent to `.maybe_api_key(Some(key)).on_grpc(uri)`.
    pub async fn on_grpc_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<GrpcTransport, F>> {
        self.maybe_api_key(Some(api_key)).on_grpc(uri).await
    }

    /// Alias for [`on_grpc`](Self::on_grpc).
    pub async fn connect(self, uri: impl AsRef<str>) -> Result<FilledProvider<GrpcTransport, F>> {
        self.on_grpc(uri).await
    }

    /// Alias for [`on_grpc_with_key`](Self::on_grpc_with_key).
    pub async fn connect_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<GrpcTransport, F>> {
        self.on_grpc_with_key(uri, api_key).await
    }
}

/// A provider that automatically applies filler `F` before every send.
#[derive(Clone)]
pub struct FilledProvider<T: TronTransport, F: TxFiller> {
    inner: RootProvider<T>,
    filler: F,
}

impl<T: TronTransport, F: TxFiller> FilledProvider<T, F> {
    /// Construct from a root provider and a filler.
    pub fn new(inner: RootProvider<T>, filler: F) -> Self {
        Self { inner, filler }
    }

    /// Borrow the underlying root provider.
    pub fn root(&self) -> &RootProvider<T> {
        &self.inner
    }

    /// Borrow the filler chain.
    pub fn filler(&self) -> &F {
        &self.filler
    }
}

impl<T: TronTransport, F: TxFiller + HasSigner + 'static> crate::provider::private::Sealed
    for FilledProvider<T, F>
{
}

impl<T: TronTransport, F: TxFiller + HasSigner + 'static> TronProvider for FilledProvider<T, F> {
    type Transport = T;

    fn transport(&self) -> &T {
        self.inner.transport()
    }

    fn signer_address(&self) -> Option<Address> {
        self.filler.signer_address()
    }

    // ── send_transaction ─────────────────────────────────────────────────────

    async fn send_transaction(&self, req: TransactionRequest) -> Result<PendingTransaction<Self>> {
        // Build (fill TAPOS / fee-limit + node round-trip), then sign & broadcast.
        let raw = self.build_transaction(req).await?;

        let sig = self
            .filler
            .sign(raw.tx_id())
            .await
            .ok_or(Error::no_signer())?
            .map_err(Error::local_usage)?;

        let tx_id = raw.tx_id();
        let signed = SignedTransaction {
            raw,
            signatures: vec![sig],
        };
        self.inner
            .transport()
            .broadcast_transaction(&signed)
            .await
            .map_err(|e| Error::from(e.into()))?;

        Ok(PendingTransaction::new(self.clone(), tx_id))
    }

    // `broadcast` uses the `TronProvider` trait default implementation.
}

impl<T: TronTransport, F: TxFiller + HasSigner + 'static> FilledProvider<T, F> {
    /// Run all fillers and build the node-side transaction **without signing or
    /// broadcasting it**.
    ///
    /// Returns the unsigned [`RawTransaction`]. Sign it once (single-sig) or
    /// collect multiple signatures (multisig) and submit via
    /// [`TronProvider::broadcast`]. For the common single-signer case prefer
    /// [`TronProvider::send_transaction`], which fills, signs, and broadcasts in
    /// one step.
    pub async fn build_transaction(&self, req: TransactionRequest) -> Result<RawTransaction> {
        // ── 1. Fill (sync then async) ────────────────────────────────────────
        let filler = self.filler.clone();
        let mut req = req;
        filler.fill_sync(&mut req);
        let mut req = filler.fill(req, self).await?;
        filler.fill_sync(&mut req); // second sync pass after async fill

        // ── 2. Route contract → transport build call ─────────────────────────
        let contract = req
            .contract
            .take()
            .ok_or(Error::missing_field("contract"))?;
        let transport = self.inner.transport();

        let raw_result = match contract {
            ContractType::Transfer(c) => transport.transfer_trx(c).await,
            ContractType::TriggerSmartContract(c) => transport.trigger_smart_contract(c).await,
            ContractType::FreezeBalanceV1(c) => transport.freeze_balance_v1(c).await,
            ContractType::UnfreezeBalanceV1(c) => transport.unfreeze_balance_v1(c).await,
            ContractType::FreezeBalanceV2(c) => transport.freeze_balance_v2(c).await,
            ContractType::UnfreezeBalanceV2(c) => transport.unfreeze_balance_v2(c).await,
            ContractType::DelegateResource(c) => transport.delegate_resource(c).await,
            ContractType::UnDelegateResource(c) => transport.undelegate_resource(c).await,
            ContractType::WithdrawExpireUnfreeze(c) => transport.withdraw_expire_unfreeze(c).await,
            ContractType::CancelAllUnfreezeV2(c) => transport.cancel_all_unfreeze_v2(c).await,
            ContractType::WithdrawBalance(c) => transport.withdraw_balance(c).await,
            ContractType::AccountPermissionUpdate(c) => {
                transport.account_permission_update(c).await
            }
            ContractType::CreateSmartContract(c) => transport.create_smart_contract(c).await,
            ContractType::AssetIssue(c) => transport.create_asset_issue(c).await,
            ContractType::TransferAsset(c) => transport.transfer_asset(c).await,
            ContractType::ParticipateAssetIssue(c) => transport.participate_asset_issue(c).await,
            ContractType::UnfreezeAsset(c) => transport.unfreeze_asset(c).await,
            ContractType::UpdateAsset(c) => transport.update_asset(c).await,
            ContractType::CreateAccount(c) => transport.create_account(c).await,
            ContractType::VoteWitness(c) => transport.vote_witness_account(c).await,
            ContractType::UpdateAccount(c) => transport.update_account(c).await,
            ContractType::ProposalCreate(c) => transport.proposal_create(c).await,
            ContractType::ProposalApprove(c) => transport.proposal_approve(c).await,
            ContractType::ProposalDelete(c) => transport.proposal_delete(c).await,
            ContractType::CreateWitness(c) => transport.create_witness(c).await,
            ContractType::UpdateWitness(c) => transport.update_witness(c).await,
            ContractType::UpdateBrokerage(c) => transport.update_brokerage(c).await,
            ContractType::SetAccountId(c) => transport.set_account_id(c).await,
            ContractType::ClearContractAbi(c) => transport.clear_contract_abi(c).await,
            ContractType::UpdateSetting(c) => transport.update_setting(c).await,
            ContractType::UpdateEnergyLimit(c) => transport.update_energy_limit(c).await,
            ContractType::ExchangeCreate(c) => transport.exchange_create(c).await,
            ContractType::ExchangeInject(c) => transport.exchange_inject(c).await,
            ContractType::ExchangeWithdraw(c) => transport.exchange_withdraw(c).await,
            ContractType::ExchangeTransaction(c) => transport.exchange_transaction(c).await,
            ContractType::MarketSellAsset(c) => transport.market_sell_asset(c).await,
            ContractType::MarketCancelOrder(c) => transport.market_cancel_order(c).await,
        };
        let mut raw = raw_result.map_err(|e| Error::from(e.into()))?;

        // ── 3. Apply fee_limit / memo / permission_id ────────────────────────
        raw.apply_request_fields(
            req.fee_limit.map(|t| t.as_sun()),
            req.memo.as_deref(),
            req.permission_id,
        )
        .map_err(Error::Transport)?;

        Ok(raw)
    }
}
