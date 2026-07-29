//! [`ProviderBuilder`] and the [`FilledProvider`] it produces.
//!
//! Mirrors alloy's `ProviderBuilder` + `JoinFill` pattern.

use std::{fmt, sync::Arc, time::Duration};

use async_trait::async_trait;
use tronz_primitives::{Address, Trx};
use tronz_signer::{TronNetworkWallet, TronSigner, TronWallet};

use crate::{
    error::{Error, Result},
    fillers::{
        EnergyFiller, FeeLimitFiller, HasSigner, Identity, JoinFill, TaposFiller, TxFiller,
        WalletFiller,
    },
    layers::{ProviderLayer, Stack},
    provider::{ContractReadProvider, PendingTransaction, RootProvider, TronProvider},
    transport::{
        TronTransport,
        grpc::{GrpcMiddleware, GrpcTransport, GrpcTransportConfig, RetryConfig},
    },
    types::{ContractType, RawTransaction, SignedTransaction, TransactionRequest},
};

/// Accumulates fillers and finally binds a transport to produce a
/// [`FilledProvider`].
///
/// Transport tuning (`connect_timeout` / `request_timeout` / `retry`) is stored
/// as `Option`s; `None` defers to [`GrpcTransportConfig`] defaults.
pub struct ProviderBuilder<F, L = Identity> {
    filler: F,
    layer: L,
    api_key: Option<String>,
    connect_timeout: Option<Duration>,
    request_timeout: Option<Duration>,
    retry: Option<RetryConfig>,
    endpoints: Vec<String>,
    middleware: Vec<Arc<dyn GrpcMiddleware>>,
}

impl<F: fmt::Debug, L: fmt::Debug> fmt::Debug for ProviderBuilder<F, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderBuilder")
            .field("filler", &self.filler)
            .field("layer", &self.layer)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("connect_timeout", &self.connect_timeout)
            .field("request_timeout", &self.request_timeout)
            .field("retry", &self.retry)
            .field("endpoints", &self.endpoints)
            .field("middleware", &self.middleware.len())
            .finish()
    }
}

impl ProviderBuilder<JoinFill<Identity, EnergyFiller>> {
    /// Start with the recommended filler chain.
    pub fn new() -> Self {
        ProviderBuilder::default().with_recommended_fillers()
    }
}

impl<L> ProviderBuilder<JoinFill<Identity, EnergyFiller>, L> {
    /// Start again with no fillers at all, keeping the transport settings and any
    /// layers already added.
    ///
    /// The same chain [`default`](Default::default) begins with, reachable without
    /// having to know that the two constructors differ.
    pub fn disable_recommended_fillers(self) -> ProviderBuilder<Identity, L> {
        let Self {
            layer,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
            middleware,
            ..
        } = self;
        ProviderBuilder {
            filler: Identity,
            layer,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
            middleware,
        }
    }
}

impl Default for ProviderBuilder<Identity> {
    fn default() -> Self {
        Self {
            filler: Identity,
            layer: Identity,
            api_key: None,
            connect_timeout: None,
            request_timeout: None,
            retry: None,
            endpoints: Vec::new(),
            middleware: Vec::new(),
        }
    }
}

impl<F: TxFiller, L> ProviderBuilder<F, L> {
    /// Optionally attach a TronGrid API key.
    ///
    /// Accepts `None` (no-op) or `Some(key)`, so you can pass an
    /// `Option<String>` directly without a `match`:
    ///
    /// ```no_run
    /// use tronz_provider::{ProviderBuilder, transport::grpc::TRONGRID_MAINNET};
    /// # async fn run() -> tronz_provider::Result<()> {
    /// let api_key: Option<String> = std::env::var("TRON_API_KEY").ok();
    /// let provider =
    ///     ProviderBuilder::new().maybe_api_key(api_key).connect_grpc(TRONGRID_MAINNET).await?;
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
    pub fn with_endpoints<I, S>(mut self, endpoints: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.endpoints = endpoints.into_iter().map(Into::into).collect();
        self
    }

    /// Observe or pace every call the transport makes.
    ///
    /// The seam nothing above it routes around: a
    /// [`ProviderLayer`](crate::ProviderLayer) only sees the methods it overrides,
    /// while middleware also sees a [`PendingTransaction`]'s polling and an event
    /// watcher's. Install more than one and they nest, first added outermost.
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use tronz_provider::{ProviderBuilder, transport::grpc::{GrpcMiddleware, TRONGRID_MAINNET}};
    /// # async fn run(limiter: Arc<dyn GrpcMiddleware>) -> tronz_provider::Result<()> {
    /// let provider = ProviderBuilder::new()
    ///     .with_middleware(limiter)
    ///     .connect_grpc(TRONGRID_MAINNET)
    ///     .await?;
    /// # let _ = provider;
    /// # Ok(()) }
    /// ```
    pub fn with_middleware(mut self, middleware: Arc<dyn GrpcMiddleware>) -> Self {
        self.middleware.push(middleware);
        self
    }

    /// Add the recommended filler chain.
    ///
    /// This installs an [`EnergyFiller`], which sizes each contract call's
    /// `fee_limit` from what the call actually costs. All supported transaction
    /// builders ask the node to construct the transaction, so TAPOS is already
    /// filled by the node. Use [`with_tapos`] explicitly only when overriding TAPOS
    /// for a locally referenced block.
    ///
    /// [`with_tapos`]: Self::with_tapos
    pub fn with_recommended_fillers(self) -> ProviderBuilder<JoinFill<F, EnergyFiller>, L> {
        self.with_energy(EnergyFiller::new())
    }

    /// Add a filler to the chain, to run after the ones already there.
    ///
    /// Every filler this builder installs goes through here, including one written
    /// downstream:
    ///
    /// ```no_run
    /// # use tronz_provider::{
    /// #     ProviderBuilder, Result, TronProvider, fillers::TxFiller,
    /// #     transport::grpc::TRONGRID_MAINNET, types::TransactionRequest,
    /// # };
    /// # #[derive(Clone, Debug)]
    /// # struct Deadline;
    /// # impl TxFiller for Deadline {
    /// #     fn fill(
    /// #         &self,
    /// #         tx: TransactionRequest,
    /// #         _: &impl TronProvider,
    /// #     ) -> impl std::future::Future<Output = Result<TransactionRequest>> + Send {
    /// #         async move { Ok(tx) }
    /// #     }
    /// # }
    /// # async fn run() -> Result<()> {
    /// let provider = ProviderBuilder::new().filler(Deadline).connect_grpc(TRONGRID_MAINNET).await?;
    /// # let _ = provider;
    /// # Ok(()) }
    /// ```
    pub fn filler<F2>(self, filler: F2) -> ProviderBuilder<JoinFill<F, F2>, L> {
        let Self {
            filler: inner,
            layer,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
            middleware,
        } = self;
        ProviderBuilder {
            filler: JoinFill::new(inner, filler),
            layer,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
            middleware,
        }
    }

    /// Wrap the provider in a [`ProviderLayer`], so a whole stack can be described
    /// in one place rather than assembled after connecting.
    ///
    /// Layers go on inside the fillers, as they do in alloy: a layer sees a
    /// transaction with its fields already filled in, and sees the RPCs the fillers
    /// make. Added first is outermost.
    ///
    /// A layer only sees the methods it overrides — what it leaves alone reaches the
    /// node through [`root`](TronProvider::root), as does a
    /// [`PendingTransaction`]'s polling. For a seam nothing routes around, use
    /// [`with_middleware`](Self::with_middleware).
    ///
    /// ```no_run
    /// # use tronz_provider::{ProviderBuilder, layers::LoggingLayer, transport::grpc::TRONGRID_MAINNET};
    /// # async fn run() -> tronz_provider::Result<()> {
    /// let provider = ProviderBuilder::new()
    ///     .layer(LoggingLayer)
    ///     .connect_grpc(TRONGRID_MAINNET)
    ///     .await?;
    /// # let _ = provider;
    /// # Ok(()) }
    /// ```
    pub fn layer<L2>(self, layer: L2) -> ProviderBuilder<F, Stack<L2, L>> {
        let Self {
            filler,
            layer: current,
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
            middleware,
        } = self;
        ProviderBuilder {
            filler,
            layer: Stack::new(layer, current),
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
            middleware,
        }
    }

    /// Size `fee_limit` from an estimate rather than a constant.
    ///
    /// Configure the estimate through [`EnergyFiller`]'s own setters.
    pub fn with_energy(
        self,
        filler: EnergyFiller,
    ) -> ProviderBuilder<JoinFill<F, EnergyFiller>, L> {
        self.filler(filler)
    }

    /// Add the TAPOS filler (required before broadcasting client-built txs).
    pub fn with_tapos(self) -> ProviderBuilder<JoinFill<F, TaposFiller>, L> {
        self.filler(TaposFiller::new())
    }

    /// Add a default `fee_limit` for contract operations.
    pub fn with_fee_limit(self, limit: Trx) -> ProviderBuilder<JoinFill<F, FeeLimitFiller>, L> {
        self.filler(FeeLimitFiller::new(limit))
    }

    /// Add a wallet so `.send()` operations work.
    ///
    /// Accepts any [`TronNetworkWallet`] implementation.
    ///
    /// ```no_run
    /// # use tronz_provider::{ProviderBuilder, transport::grpc::TRONGRID_MAINNET};
    /// # use tronz_signer::{LocalSigner, TronWallet};
    /// # async fn run(key_a: &str, key_b: &str) -> tronz_provider::Result<()> {
    /// let mut wallet = TronWallet::new(LocalSigner::from_hex(key_a).unwrap());
    /// wallet.register_signer(LocalSigner::from_hex(key_b).unwrap());
    ///
    /// let provider = ProviderBuilder::new().wallet(wallet).connect_grpc(TRONGRID_MAINNET).await?;
    /// # Ok(()) }
    /// ```
    pub fn wallet<W: TronNetworkWallet>(
        self,
        wallet: W,
    ) -> ProviderBuilder<JoinFill<F, WalletFiller<W>>, L> {
        self.wallet_filler(WalletFiller::new(wallet))
    }

    /// Attach a wallet that refuses to substitute its default credential when it
    /// holds no key for a transaction's owner.
    ///
    /// [`wallet`](Self::wallet) falls back, because a TRON account can authorize
    /// another account's key through an active permission. Use this instead when
    /// a missing key means a bug rather than a delegation.
    pub fn strict_wallet<W: TronNetworkWallet>(
        self,
        wallet: W,
    ) -> ProviderBuilder<JoinFill<F, WalletFiller<W>>, L> {
        self.wallet_filler(WalletFiller::new(wallet).strict())
    }

    fn wallet_filler<W: TronNetworkWallet>(
        self,
        wallet: WalletFiller<W>,
    ) -> ProviderBuilder<JoinFill<F, WalletFiller<W>>, L> {
        self.filler(wallet)
    }

    /// Attach a single signer so `.send()` operations work.
    ///
    /// The signer is moved into a cloneable [`TronWallet`] owned by the
    /// provider. The signer itself does not need to implement [`Clone`].
    pub fn with_signer<S>(
        self,
        signer: S,
    ) -> ProviderBuilder<JoinFill<F, WalletFiller<TronWallet>>, L>
    where
        S: TronSigner + Send + Sync + 'static,
    {
        self.wallet(TronWallet::new(signer))
    }
}

impl<F: TxFiller, L: ProviderLayer<RootProvider>> ProviderBuilder<F, L> {
    /// Connect to a TRON gRPC node, applying any API key set via
    /// [`maybe_api_key`](Self::maybe_api_key).
    ///
    /// `uri` examples:
    /// - `"https://grpc.trongrid.io:443"` (TronGrid mainnet, TLS)
    /// - `"http://127.0.0.1:50051"` (local node, plain HTTP/2)
    pub async fn connect_grpc(
        self,
        uri: impl AsRef<str>,
    ) -> Result<FilledProvider<F, L::Provider>> {
        let mut cfg = GrpcTransportConfig {
            api_key: self.api_key,
            endpoints: self.endpoints,
            middleware: self.middleware,
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
        let transport =
            GrpcTransport::connect_with_config(uri, cfg).await.map_err(Error::Transport)?;
        let root = RootProvider::new(transport);
        Ok(FilledProvider::new(self.layer.layer(root), self.filler))
    }

    /// Connect with an explicit TronGrid API key.
    ///
    /// Equivalent to `.maybe_api_key(Some(key)).connect_grpc(uri)`.
    pub async fn connect_grpc_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<F, L::Provider>> {
        self.maybe_api_key(Some(api_key)).connect_grpc(uri).await
    }

    /// Alias for [`connect_grpc`](Self::connect_grpc).
    pub async fn connect(self, uri: impl AsRef<str>) -> Result<FilledProvider<F, L::Provider>> {
        self.connect_grpc(uri).await
    }

    /// Build over a transport that already exists, instead of dialing one.
    ///
    /// This is the way in for a `MockTransport` under test, or for any transport
    /// built by hand. The connection settings on this builder
    /// (`api_key`, the timeouts, `retry`, `endpoints`) all configure a gRPC dial,
    /// so they are ignored here — `transport` is already connected, and carries
    /// whatever configuration it was built with.
    ///
    /// ```no_run
    /// # use tronz_provider::{ProviderBuilder, ReadProvider};
    /// # use tronz_provider::transport::grpc::{GrpcTransport, TRONGRID_MAINNET};
    /// # async fn run() -> tronz_provider::Result<()> {
    /// let transport = GrpcTransport::connect(TRONGRID_MAINNET).await?;
    /// let provider: ReadProvider = ProviderBuilder::new().connect_transport(transport);
    /// # let _ = provider;
    /// # Ok(()) }
    /// ```
    pub fn connect_transport(
        self,
        transport: impl TronTransport,
    ) -> FilledProvider<F, L::Provider> {
        let root = RootProvider::new(transport);
        FilledProvider::new(self.layer.layer(root), self.filler)
    }

    /// Alias for [`connect_grpc_with_key`](Self::connect_grpc_with_key).
    pub async fn connect_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<F, L::Provider>> {
        self.connect_grpc_with_key(uri, api_key).await
    }

    /// Deprecated alias for [`connect_grpc`](Self::connect_grpc).
    #[deprecated(note = "use `connect_grpc` instead")]
    pub async fn on_grpc(self, uri: impl AsRef<str>) -> Result<FilledProvider<F, L::Provider>> {
        self.connect_grpc(uri).await
    }

    /// Deprecated alias for [`connect_grpc_with_key`](Self::connect_grpc_with_key).
    #[deprecated(note = "use `connect_grpc_with_key` instead")]
    pub async fn on_grpc_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<F, L::Provider>> {
        self.connect_grpc_with_key(uri, api_key).await
    }
}

/// A provider that automatically applies filler `F` before every send.
///
/// `P` is whatever it was built over: a [`RootProvider`], or that wrapped in the
/// [`ProviderLayer`](crate::ProviderLayer)s a
/// [`ProviderBuilder::layer`] installed. Fillers sit outside layers, so a layer sees
/// transactions with their fields already filled in.
#[derive(Clone)]
pub struct FilledProvider<F: TxFiller, P = RootProvider> {
    inner: P,
    filler: F,
}

impl<F: TxFiller, P: TronProvider> FilledProvider<F, P> {
    /// Construct from an inner provider and a filler.
    pub fn new(inner: P, filler: F) -> Self {
        Self { inner, filler }
    }

    /// Borrow the underlying root provider.
    pub fn root(&self) -> &RootProvider {
        self.inner.root()
    }

    /// Borrow the provider this one was built over — the layers, if any were
    /// installed, and otherwise the root.
    pub const fn inner(&self) -> &P {
        &self.inner
    }

    /// Borrow the filler chain.
    pub const fn filler(&self) -> &F {
        &self.filler
    }
}

impl<F: TxFiller + HasSigner + 'static, P: TronProvider> ContractReadProvider
    for FilledProvider<F, P>
{
    fn inner_read(&self) -> Option<&dyn ContractReadProvider> {
        Some(&self.inner)
    }

    fn default_caller(&self) -> Option<Address> {
        self.filler.signer_address()
    }
}

#[async_trait]
impl<F: TxFiller + HasSigner + 'static, P: TronProvider> TronProvider for FilledProvider<F, P> {
    fn root(&self) -> &RootProvider {
        self.inner.root()
    }

    fn inner(&self) -> Option<&dyn TronProvider> {
        Some(&self.inner)
    }

    fn signer_address(&self) -> Option<Address> {
        self.filler.signer_address()
    }

    async fn send_transaction(&self, req: TransactionRequest) -> Result<PendingTransaction> {
        let key = match req.contract.as_ref().map(ContractType::owner_address) {
            Some(owner) if owner == Address::ZERO => {
                return Err(Error::missing_field("owner_address"));
            }
            key => key,
        };

        let raw = self.build_transaction(req).await?;

        let sig = self
            .filler
            .sign_with(key, raw.tx_id())
            .await
            .ok_or(Error::no_signer())?
            .map_err(Error::local_usage)?;

        let signed = SignedTransaction { raw, signatures: vec![sig] };

        self.inner.broadcast(signed).await
    }

    /// Runs the configured fillers, then asks the provider below to build.
    async fn build_transaction(&self, req: TransactionRequest) -> Result<RawTransaction> {
        let filler = self.filler.clone();
        let mut req = req;
        filler.fill_sync(&mut req);
        let mut req = filler.fill(req, self).await?;
        filler.fill_sync(&mut req); // second sync pass after async fill

        self.inner.build_transaction(req).await
    }
}

#[cfg(test)]
mod tests {
    use core::future::Future;
    use std::sync::{Arc, Mutex};

    use prost::Message as _;
    use tronz_primitives::{Address, B256, Bytes, RecoverableSignature};
    use tronz_signer::{LocalSigner, SignerError, TronSigner, TronWallet};

    use super::*;
    use crate::{
        fillers::WalletFiller,
        transport::mock::MockTransport,
        types::{ContractType, TransferContract, TriggerSmartContract},
    };

    const KEY_A: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const KEY_B: &str = "0000000000000000000000000000000000000000000000000000000000000002";

    #[tokio::test]
    async fn recommended_fillers_do_not_fetch_tapos() {
        let transport = MockTransport::new();
        transport.push_ok("estimate_energy", 1_000i64);
        transport.push_ok("get_energy_prices", "0:420".to_owned());
        let provider = RootProvider::new(transport);
        let builder = ProviderBuilder::new();
        let address = Address::from_evm_bytes([1; 20]);
        let request = TransactionRequest {
            contract: Some(ContractType::TriggerSmartContract(TriggerSmartContract {
                owner_address: address,
                contract_address: address,
                call_value: Trx::ZERO,
                data: Bytes::new(),
                call_token_value: Trx::ZERO,
                token_id: 0,
            })),
            ..Default::default()
        };

        let mut filled = builder.filler.fill(request, &provider).await.unwrap();
        builder.filler.fill_sync(&mut filled);
        assert_eq!(filled.fee_limit, Some(Trx::from_sun_unchecked(504_000)));
        assert!(filled.ref_block_bytes.is_none());
    }

    #[test]
    fn signer_ownership_moves_into_cloneable_wallet_filler() {
        let signer: Box<dyn TronSigner + Send + Sync> =
            Box::new(LocalSigner::from_hex(KEY_A).unwrap());
        let expected = signer.address();
        let builder = ProviderBuilder::default().with_signer(signer);
        assert_eq!(builder.filler.signer_address(), Some(expected));

        let wallet = TronWallet::new(LocalSigner::from_hex(KEY_A).unwrap());
        let builder = ProviderBuilder::default().wallet(wallet);
        assert_eq!(builder.filler.signer_address(), Some(expected));
    }

    #[derive(Clone, Debug)]
    struct RecordingWallet {
        inner: TronWallet,
        keys: Arc<Mutex<Vec<Address>>>,
    }

    impl TronNetworkWallet for RecordingWallet {
        fn default_signer_address(&self) -> Address {
            self.inner.default_signer_address()
        }

        fn has_signer_for(&self, address: &Address) -> bool {
            self.inner.has_signer_for(address)
        }

        fn signer_addresses(&self) -> impl Iterator<Item = Address> {
            self.inner.signer_addresses()
        }

        fn sign_hash_with(
            &self,
            key: Address,
            hash: &B256,
        ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
            self.keys.lock().unwrap().push(key);
            self.inner.sign_hash_with(key, hash)
        }

        fn sign_message_with(
            &self,
            key: Address,
            message: &[u8],
        ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
            self.keys.lock().unwrap().push(key);
            self.inner.sign_message_with(key, message)
        }
    }

    type RecordingProvider = FilledProvider<WalletFiller<RecordingWallet>>;

    fn recording_provider() -> (RecordingProvider, Arc<Mutex<Vec<Address>>>) {
        let mut inner = TronWallet::new(LocalSigner::from_hex(KEY_A).unwrap());
        inner.register_signer(LocalSigner::from_hex(KEY_B).unwrap());

        let keys = Arc::new(Mutex::new(Vec::new()));
        let wallet = RecordingWallet { inner, keys: Arc::clone(&keys) };

        let transport = MockTransport::new();
        transport.push_ok("transfer_trx", node_built_transfer());
        transport.push_ok("broadcast_transaction", ());

        let provider = FilledProvider::new(RootProvider::new(transport), WalletFiller::new(wallet));
        (provider, keys)
    }
    fn node_built_transfer() -> RawTransaction {
        let tx = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw {
                contract: vec![crate::proto::transaction::Contract::default()],
                ..Default::default()
            }),
            ..Default::default()
        };
        RawTransaction::from_node_encoded(tx.encode_to_vec(), &[]).unwrap()
    }

    fn transfer_from(owner: Address) -> TransactionRequest {
        TransactionRequest::default().with_contract(ContractType::Transfer(TransferContract {
            owner_address: owner,
            to_address: Address::from_evm_bytes([9; 20]),
            amount: Trx::from_sun_unchecked(1),
        }))
    }
    #[tokio::test]
    async fn erasing_a_provider_keeps_its_filler_chain() {
        let secondary = LocalSigner::from_hex(KEY_B).unwrap().address();
        let (provider, keys) = recording_provider();

        provider.erased().send_transaction(transfer_from(secondary)).await.unwrap();

        assert_eq!(*keys.lock().unwrap(), vec![secondary]);
    }

    #[tokio::test]
    async fn send_transaction_signs_with_the_credential_named_by_the_owner() {
        let secondary = LocalSigner::from_hex(KEY_B).unwrap().address();
        let (provider, keys) = recording_provider();

        provider.send_transaction(transfer_from(secondary)).await.unwrap();

        assert_eq!(*keys.lock().unwrap(), vec![secondary]);
    }

    #[tokio::test]
    async fn send_transaction_signs_for_an_unheld_owner_with_the_default_credential() {
        let default = LocalSigner::from_hex(KEY_A).unwrap().address();
        let multisig_owner = Address::from_evm_bytes([7; 20]);
        let (provider, keys) = recording_provider();

        provider.send_transaction(transfer_from(multisig_owner)).await.unwrap();

        assert_eq!(*keys.lock().unwrap(), vec![default]);
    }

    #[tokio::test]
    async fn active_permission_still_prefers_the_owner_key_when_the_wallet_holds_it() {
        let owner = LocalSigner::from_hex(KEY_B).unwrap().address();
        let (provider, keys) = recording_provider();

        provider.send_transaction(transfer_from(owner).with_permission_id(2)).await.unwrap();

        assert_eq!(*keys.lock().unwrap(), vec![owner]);
    }

    #[tokio::test]
    async fn send_transaction_errors_when_the_wallet_holds_no_credentials() {
        let transport = MockTransport::new();
        transport.push_ok("transfer_trx", node_built_transfer());
        let provider = FilledProvider::new(
            RootProvider::new(transport),
            WalletFiller::new(TronWallet::default()),
        );

        let owner = LocalSigner::from_hex(KEY_A).unwrap().address();
        let Err(err) = provider.send_transaction(transfer_from(owner)).await else {
            panic!("signing without a credential should fail");
        };
        assert!(err.to_string().contains("missing signing credential"));
    }

    #[tokio::test]
    async fn send_transaction_rejects_a_request_with_no_owner() {
        let provider = FilledProvider::new(
            RootProvider::new(MockTransport::new()),
            WalletFiller::new(TronWallet::new(LocalSigner::from_hex(KEY_A).unwrap())),
        );

        let Err(err) = provider.send_transaction(transfer_from(Address::ZERO)).await else {
            panic!("a zero owner should be rejected");
        };
        assert!(err.to_string().contains("owner_address"));
    }
    #[tokio::test]
    async fn a_custom_filler_can_be_installed() {
        #[derive(Clone, Debug)]
        struct Memo;

        impl TxFiller for Memo {
            async fn fill(
                &self,
                tx: TransactionRequest,
                _: &impl TronProvider,
            ) -> Result<TransactionRequest> {
                Ok(tx.with_fee_limit(Trx::from_sun_unchecked(7)))
            }
        }

        impl HasSigner for Memo {}

        let builder = ProviderBuilder::default().filler(Memo);
        let provider = RootProvider::new(MockTransport::new());
        let filled = builder.filler.fill(TransactionRequest::default(), &provider).await.unwrap();

        assert_eq!(filled.fee_limit, Some(Trx::from_sun_unchecked(7)));
    }

    #[test]
    fn recommended_fillers_can_be_turned_back_off() {
        let builder = ProviderBuilder::new()
            .with_request_timeout(Duration::from_secs(9))
            .layer(crate::layers::LoggingLayer)
            .disable_recommended_fillers();
        assert_eq!(builder.request_timeout, Some(Duration::from_secs(9)));
        let _: &Stack<crate::layers::LoggingLayer, Identity> = &builder.layer;
    }

    #[test]
    fn middleware_reaches_the_transport_config() {
        use crate::transport::grpc::{GrpcCall, GrpcMiddleware};

        struct Nothing;

        #[async_trait]
        impl GrpcMiddleware for Nothing {
            async fn before(
                &self,
                _: GrpcCall<'_>,
            ) -> std::result::Result<(), crate::TransportErrorKind> {
                Ok(())
            }
        }

        let builder = ProviderBuilder::new().with_middleware(Arc::new(Nothing));

        assert_eq!(builder.middleware.len(), 1);
    }
    #[derive(Clone, Default)]
    struct Recorder {
        seen: Arc<Mutex<Vec<&'static str>>>,
    }

    #[derive(Clone)]
    struct Recording<P> {
        inner: P,
        seen: Arc<Mutex<Vec<&'static str>>>,
    }

    impl<P: TronProvider> ProviderLayer<P> for Recorder {
        type Provider = Recording<P>;

        fn layer(&self, inner: P) -> Recording<P> {
            Recording { inner, seen: self.seen.clone() }
        }
    }

    impl<P: TronProvider> ContractReadProvider for Recording<P> {
        fn inner_read(&self) -> Option<&dyn ContractReadProvider> {
            Some(&self.inner)
        }
    }
    #[async_trait]
    impl<P: TronProvider> TronProvider for Recording<P> {
        fn root(&self) -> &RootProvider {
            self.inner.root()
        }

        fn inner(&self) -> Option<&dyn TronProvider> {
            Some(&self.inner)
        }

        async fn get_now_block(&self) -> Result<crate::types::BlockInfo> {
            self.seen.lock().unwrap().push("get_now_block");
            self.inner.get_now_block().await
        }

        async fn build_transaction(&self, req: TransactionRequest) -> Result<RawTransaction> {
            self.seen.lock().unwrap().push("build_transaction");
            self.inner.build_transaction(req).await
        }

        async fn broadcast(&self, tx: SignedTransaction) -> Result<PendingTransaction> {
            self.seen.lock().unwrap().push("broadcast");
            self.inner.broadcast(tx).await
        }
    }

    type LayeredProvider =
        FilledProvider<JoinFill<Identity, WalletFiller<TronWallet>>, Recording<RootProvider>>;

    fn layered_provider(transport: MockTransport) -> (LayeredProvider, Recorder) {
        let recorder = Recorder::default();
        let wallet = TronWallet::new(LocalSigner::from_hex(KEY_A).unwrap());
        let provider = ProviderBuilder::default()
            .layer(recorder.clone())
            .wallet(wallet)
            .connect_transport(transport);

        (provider, recorder)
    }
    #[tokio::test]
    async fn a_layer_sees_an_ordinary_read() {
        let transport = MockTransport::new();
        transport.push_ok("get_now_block", crate::types::BlockInfo::new(1, B256::ZERO, 0));
        let (provider, recorder) = layered_provider(transport);

        let _ = provider.get_now_block().await.unwrap();

        assert_eq!(*recorder.seen.lock().unwrap(), vec!["get_now_block"]);
    }
    #[tokio::test]
    async fn a_layer_sees_the_build_and_the_broadcast() {
        let transport = MockTransport::new();
        transport.push_ok("transfer_trx", node_built_transfer());
        transport.push_ok("broadcast_transaction", ());
        let (provider, recorder) = layered_provider(transport);

        let owner = LocalSigner::from_hex(KEY_A).unwrap().address();
        provider.send_transaction(transfer_from(owner)).await.unwrap();
        assert_eq!(*recorder.seen.lock().unwrap(), vec!["build_transaction", "broadcast"]);
    }
    #[tokio::test]
    async fn a_layer_sees_a_send_made_through_an_operation_builder() {
        let transport = MockTransport::new();
        transport.push_ok("transfer_trx", node_built_transfer());
        transport.push_ok("broadcast_transaction", ());
        let (provider, recorder) = layered_provider(transport);

        let owner = LocalSigner::from_hex(KEY_A).unwrap().address();
        provider
            .send_trx()
            .from(owner)
            .to(Address::from_evm_bytes([9; 20]))
            .amount(Trx::from_sun_unchecked(1))
            .send()
            .await
            .unwrap();

        assert_eq!(*recorder.seen.lock().unwrap(), vec!["build_transaction", "broadcast"]);
    }
    #[tokio::test]
    async fn layers_stack_in_the_order_they_were_added() {
        let transport = MockTransport::new();
        transport.push_ok("get_now_block", crate::types::BlockInfo::new(1, B256::ZERO, 0));
        let outer = Recorder::default();
        let inner = Recorder::default();

        let provider = ProviderBuilder::default()
            .layer(outer.clone())
            .layer(inner.clone())
            .connect_transport(transport);

        let _ = provider.get_now_block().await.unwrap();

        assert_eq!(*outer.seen.lock().unwrap(), vec!["get_now_block"]);
        assert_eq!(*inner.seen.lock().unwrap(), vec!["get_now_block"]);
    }
}
