//! [`ProviderBuilder`] and the [`FilledProvider`] it produces.
//!
//! Mirrors alloy's `ProviderBuilder` + `JoinFill` pattern (see `DESIGN.md` §5.3).

use tronz_primitives::{Address, Trx};
use tronz_signer::TronSigner;

use crate::{
    error::{Error, Result},
    fillers::{FeeLimitFiller, HasSigner, Identity, JoinFill, SignerFiller, TaposFiller, TxFiller},
    provider::{PendingTransaction, RootProvider, TronProvider},
    transport::{TronTransport, grpc::GrpcTransport},
    types::{ContractType, SignedTransaction, TransactionRequest},
};

/// Accumulates fillers and finally binds a transport to produce a
/// [`FilledProvider`].
pub struct ProviderBuilder<F> {
    filler: F,
    api_key: Option<String>,
}

impl ProviderBuilder<Identity> {
    /// Start with no fillers.
    pub fn new() -> Self {
        Self {
            filler: Identity,
            api_key: None,
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
        ProviderBuilder {
            filler: JoinFill::new(self.filler, TaposFiller::new()),
            api_key: self.api_key,
        }
    }

    /// Add a default `fee_limit` for contract operations.
    pub fn with_fee_limit(self, limit: Trx) -> ProviderBuilder<JoinFill<F, FeeLimitFiller>> {
        ProviderBuilder {
            filler: JoinFill::new(self.filler, FeeLimitFiller::new(limit)),
            api_key: self.api_key,
        }
    }

    /// Attach a signer so `.send()` operations work.
    pub fn with_signer<S: TronSigner>(
        self,
        signer: S,
    ) -> ProviderBuilder<JoinFill<F, SignerFiller<S>>> {
        ProviderBuilder {
            filler: JoinFill::new(self.filler, SignerFiller::new(signer)),
            api_key: self.api_key,
        }
    }

    /// Connect to a TRON gRPC node, applying any API key set via
    /// [`maybe_api_key`](Self::maybe_api_key).
    ///
    /// `uri` examples:
    /// - `"https://grpc.trongrid.io:443"` (TronGrid mainnet, TLS)
    /// - `"http://127.0.0.1:50051"` (local node, plain HTTP/2)
    pub async fn on_grpc(self, uri: impl AsRef<str>) -> Result<FilledProvider<GrpcTransport, F>> {
        let mut transport = GrpcTransport::connect(uri)
            .await
            .map_err(Error::Transport)?;
        if let Some(key) = self.api_key {
            transport = transport.with_api_key(key);
        }
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
        // ── 1. Fill (sync then async) ────────────────────────────────────────
        let filler = self.filler.clone();
        let mut req = req;
        filler.fill_sync(&mut req);
        let mut req = filler.fill(req, self).await?;
        filler.fill_sync(&mut req); // second sync pass after async fill

        // ── 2. Route contract → transport call ───────────────────────────────
        let contract = req
            .contract
            .take()
            .ok_or(Error::missing_field("contract"))?;
        let transport = self.inner.transport();

        let mut raw = match contract {
            ContractType::Transfer(c) => transport
                .transfer_trx(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::TriggerSmartContract(c) => transport
                .trigger_smart_contract(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::FreezeBalanceV1(c) => transport
                .freeze_balance_v1(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UnfreezeBalanceV1(c) => transport
                .unfreeze_balance_v1(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::FreezeBalanceV2(c) => transport
                .freeze_balance_v2(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UnfreezeBalanceV2(c) => transport
                .unfreeze_balance_v2(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::DelegateResource(c) => transport
                .delegate_resource(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UnDelegateResource(c) => transport
                .undelegate_resource(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::WithdrawExpireUnfreeze(c) => transport
                .withdraw_expire_unfreeze(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::CancelAllUnfreezeV2(c) => transport
                .cancel_all_unfreeze_v2(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::WithdrawBalance(c) => transport
                .withdraw_balance(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::AccountPermissionUpdate(c) => transport
                .account_permission_update(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::CreateSmartContract(c) => transport
                .create_smart_contract(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::AssetIssue(c) => transport
                .create_asset_issue(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::TransferAsset(c) => transport
                .transfer_asset(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::ParticipateAssetIssue(c) => transport
                .participate_asset_issue(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UnfreezeAsset(c) => transport
                .unfreeze_asset(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UpdateAsset(c) => transport
                .update_asset(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::CreateAccount(c) => transport
                .create_account(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::VoteWitness(c) => transport
                .vote_witness_account(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UpdateAccount(c) => transport
                .update_account(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::ProposalCreate(c) => transport
                .proposal_create(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::ProposalApprove(c) => transport
                .proposal_approve(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::ProposalDelete(c) => transport
                .proposal_delete(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::CreateWitness(c) => transport
                .create_witness(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UpdateWitness(c) => transport
                .update_witness(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UpdateBrokerage(c) => transport
                .update_brokerage(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::SetAccountId(c) => transport
                .set_account_id(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::ClearContractAbi(c) => transport
                .clear_contract_abi(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UpdateSetting(c) => transport
                .update_setting(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
            ContractType::UpdateEnergyLimit(c) => transport
                .update_energy_limit(c)
                .await
                .map_err(|e| Error::Transport(e.into()))?,
        };

        // ── 3. Apply fee_limit / memo / permission_id ────────────────────────
        raw.apply_request_fields(
            req.fee_limit.map(|t| t.as_sun()),
            req.memo.as_deref(),
            req.permission_id,
        )
        .map_err(Error::Transport)?;

        // ── 4. Sign ──────────────────────────────────────────────────────────
        let sig = self
            .filler
            .sign(raw.tx_id())
            .await
            .ok_or(Error::no_signer())?
            .map_err(Error::local_usage)?;

        // ── 5. Broadcast ─────────────────────────────────────────────────────
        let tx_id = raw.tx_id();
        let signed = SignedTransaction {
            raw,
            signatures: vec![sig],
        };
        transport
            .broadcast_transaction(&signed)
            .await
            .map_err(|e| Error::Transport(e.into()))?;

        Ok(PendingTransaction::new(self.clone(), tx_id))
    }

    async fn broadcast(&self, tx: SignedTransaction) -> Result<PendingTransaction<Self>> {
        let tx_id = tx.raw.tx_id();
        self.inner
            .transport()
            .broadcast_transaction(&tx)
            .await
            .map_err(|e| Error::Transport(e.into()))?;
        Ok(PendingTransaction::new(self.clone(), tx_id))
    }
}
