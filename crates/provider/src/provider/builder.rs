//! [`ProviderBuilder`] and the [`FilledProvider`] it produces.
//!
//! Mirrors alloy's `ProviderBuilder` + `JoinFill` pattern.

use std::time::Duration;

use tronz_primitives::{Address, Trx, TxId};
use tronz_signer::{TronNetworkWallet, TronSigner, TronWallet};

use crate::{
    error::{Error, Result},
    fillers::{FeeLimitFiller, HasSigner, Identity, JoinFill, TaposFiller, TxFiller, WalletFiller},
    provider::{ContractReadProvider, PendingTransaction, RootProvider, TronProvider},
    transport::{
        TronTransport,
        grpc::{GrpcTransport, GrpcTransportConfig, RetryConfig},
    },
    types::{
        ConstantCallResult, ContractType, RawTransaction, SignedTransaction, TransactionInfo,
        TransactionRequest, TriggerSmartContract,
    },
};

/// Accumulates fillers and finally binds a transport to produce a
/// [`FilledProvider`].
///
/// Transport tuning (`connect_timeout` / `request_timeout` / `retry`) is stored
/// as `Option`s; `None` defers to [`GrpcTransportConfig`] defaults.
#[derive(Debug)]
pub struct ProviderBuilder<F> {
    filler: F,
    api_key: Option<String>,
    connect_timeout: Option<Duration>,
    request_timeout: Option<Duration>,
    retry: Option<RetryConfig>,
    endpoints: Vec<String>,
}

impl ProviderBuilder<JoinFill<Identity, FeeLimitFiller>> {
    /// Start with the recommended filler chain.
    pub fn new() -> Self {
        ProviderBuilder::default().with_recommended_fillers()
    }
}

impl Default for ProviderBuilder<Identity> {
    fn default() -> Self {
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

    /// Add the recommended filler chain.
    ///
    /// This currently installs a 20 TRX default fee limit. All supported
    /// transaction builders ask the node to construct the transaction, so TAPOS
    /// is already filled by the node. Use [`with_tapos`] explicitly only when
    /// overriding TAPOS for a locally referenced block.
    ///
    /// [`with_tapos`]: Self::with_tapos
    pub fn with_recommended_fillers(self) -> ProviderBuilder<JoinFill<F, FeeLimitFiller>> {
        self.with_fee_limit(Trx::from_sun_unchecked(20_000_000))
    }

    /// Add the TAPOS filler (required before broadcasting client-built txs).
    pub fn with_tapos(self) -> ProviderBuilder<JoinFill<F, TaposFiller>> {
        // Destructure so adding a transport-config field later is a compile
        // error here, not a silently dropped setting.
        let Self { filler, api_key, connect_timeout, request_timeout, retry, endpoints } = self;
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
        let Self { filler, api_key, connect_timeout, request_timeout, retry, endpoints } = self;
        ProviderBuilder {
            filler: JoinFill::new(filler, FeeLimitFiller::new(limit)),
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        }
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
    ) -> ProviderBuilder<JoinFill<F, WalletFiller<W>>> {
        let Self { filler, api_key, connect_timeout, request_timeout, retry, endpoints } = self;
        ProviderBuilder {
            filler: JoinFill::new(filler, WalletFiller::new(wallet)),
            api_key,
            connect_timeout,
            request_timeout,
            retry,
            endpoints,
        }
    }

    /// Attach a single signer so `.send()` operations work.
    ///
    /// The signer is moved into a cloneable [`TronWallet`] owned by the
    /// provider. The signer itself does not need to implement [`Clone`].
    pub fn with_signer<S>(self, signer: S) -> ProviderBuilder<JoinFill<F, WalletFiller<TronWallet>>>
    where
        S: TronSigner + Send + Sync + 'static,
    {
        self.wallet(TronWallet::new(signer))
    }

    /// Connect to a TRON gRPC node, applying any API key set via
    /// [`maybe_api_key`](Self::maybe_api_key).
    ///
    /// `uri` examples:
    /// - `"https://grpc.trongrid.io:443"` (TronGrid mainnet, TLS)
    /// - `"http://127.0.0.1:50051"` (local node, plain HTTP/2)
    pub async fn connect_grpc(
        self,
        uri: impl AsRef<str>,
    ) -> Result<FilledProvider<GrpcTransport, F>> {
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
        let transport =
            GrpcTransport::connect_with_config(uri, cfg).await.map_err(Error::Transport)?;
        Ok(FilledProvider::new(RootProvider::new(transport), self.filler))
    }

    /// Connect with an explicit TronGrid API key.
    ///
    /// Equivalent to `.maybe_api_key(Some(key)).connect_grpc(uri)`.
    pub async fn connect_grpc_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<GrpcTransport, F>> {
        self.maybe_api_key(Some(api_key)).connect_grpc(uri).await
    }

    /// Alias for [`connect_grpc`](Self::connect_grpc).
    pub async fn connect(self, uri: impl AsRef<str>) -> Result<FilledProvider<GrpcTransport, F>> {
        self.connect_grpc(uri).await
    }

    /// Alias for [`connect_grpc_with_key`](Self::connect_grpc_with_key).
    pub async fn connect_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<GrpcTransport, F>> {
        self.connect_grpc_with_key(uri, api_key).await
    }

    /// Deprecated alias for [`connect_grpc`](Self::connect_grpc).
    #[deprecated(note = "use `connect_grpc` instead")]
    pub async fn on_grpc(self, uri: impl AsRef<str>) -> Result<FilledProvider<GrpcTransport, F>> {
        self.connect_grpc(uri).await
    }

    /// Deprecated alias for [`connect_grpc_with_key`](Self::connect_grpc_with_key).
    #[deprecated(note = "use `connect_grpc_with_key` instead")]
    pub async fn on_grpc_with_key(
        self,
        uri: impl AsRef<str>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<GrpcTransport, F>> {
        self.connect_grpc_with_key(uri, api_key).await
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
impl<T: TronTransport, F: TxFiller + HasSigner + 'static>
    crate::provider::private::ContractReadSealed for FilledProvider<T, F>
{
}

impl<T: TronTransport, F: TxFiller + HasSigner + 'static> ContractReadProvider
    for FilledProvider<T, F>
{
    fn default_caller(&self) -> Option<Address> {
        self.filler.signer_address()
    }

    async fn call_contract(&self, params: TriggerSmartContract) -> Result<ConstantCallResult> {
        self.inner.call_contract(params).await
    }

    async fn estimate_contract_energy(&self, params: TriggerSmartContract) -> Result<i64> {
        self.inner.estimate_contract_energy(params).await
    }

    async fn transaction_info(&self, tx_id: TxId) -> Result<Option<TransactionInfo>> {
        self.inner.transaction_info(tx_id).await
    }

    async fn transaction_infos_by_block(&self, block_num: i64) -> Result<Vec<TransactionInfo>> {
        self.inner.transaction_infos_by_block(block_num).await
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
        let key = req
            .contract
            .as_ref()
            .map(ContractType::owner_address)
            .filter(|owner| *owner != Address::ZERO);

        let raw = self.build_transaction(req).await?;

        let sig = self
            .filler
            .sign_with(key, raw.tx_id())
            .await
            .ok_or(Error::no_signer())?
            .map_err(Error::local_usage)?;

        let tx_id = raw.tx_id();
        let signed = SignedTransaction { raw, signatures: vec![sig] };
        self.inner.transport().broadcast_transaction(&signed).await.map_err(Error::transport)?;

        Ok(PendingTransaction::new(self.clone(), tx_id))
    }

    /// Runs the configured fillers before building the transaction.
    async fn build_transaction(&self, req: TransactionRequest) -> Result<RawTransaction> {
        let filler = self.filler.clone();
        let mut req = req;
        filler.fill_sync(&mut req);
        let mut req = filler.fill(req, self).await?;
        filler.fill_sync(&mut req); // second sync pass after async fill

        crate::provider::build_via_transport(self.inner.transport(), req).await
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
        // No get_now_block response is queued. The mock would panic if the
        // recommended chain still contained TaposFiller.
        let provider = RootProvider::new(MockTransport::new());
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

        assert_eq!(filled.fee_limit, Some(Trx::from_sun_unchecked(20_000_000)));
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

    type RecordingProvider = FilledProvider<MockTransport, WalletFiller<RecordingWallet>>;

    fn recording_provider() -> (RecordingProvider, Arc<Mutex<Vec<Address>>>) {
        let mut inner = TronWallet::new(LocalSigner::from_hex(KEY_A).unwrap());
        inner.register_signer(LocalSigner::from_hex(KEY_B).unwrap());

        let keys = Arc::new(Mutex::new(Vec::new()));
        let wallet = RecordingWallet { inner, keys: Arc::clone(&keys) };

        let transport = MockTransport::new();
        let tx = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw::default()),
            ..Default::default()
        };
        transport.push_ok(
            "transfer_trx",
            RawTransaction::from_proto_extention(vec![0; 32], tx.encode_to_vec(), 0, 0).unwrap(),
        );
        transport.push_ok("broadcast_transaction", ());

        let provider = FilledProvider::new(RootProvider::new(transport), WalletFiller::new(wallet));
        (provider, keys)
    }

    fn transfer_from(owner: Address) -> TransactionRequest {
        TransactionRequest::default().with_contract(ContractType::Transfer(TransferContract {
            owner_address: owner,
            to_address: Address::from_evm_bytes([9; 20]),
            amount: Trx::from_sun_unchecked(1),
        }))
    }

    #[tokio::test]
    async fn send_transaction_signs_with_the_credential_named_by_the_owner() {
        let secondary = LocalSigner::from_hex(KEY_B).unwrap().address();
        let (provider, keys) = recording_provider();

        provider.send_transaction(transfer_from(secondary)).await.unwrap();

        assert_eq!(*keys.lock().unwrap(), vec![secondary]);
    }

    #[tokio::test]
    async fn send_transaction_falls_back_to_the_default_credential() {
        let default = LocalSigner::from_hex(KEY_A).unwrap().address();
        let (provider, keys) = recording_provider();

        provider.send_transaction(transfer_from(Address::ZERO)).await.unwrap();

        assert_eq!(*keys.lock().unwrap(), vec![default]);
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
        transport.push_ok(
            "transfer_trx",
            RawTransaction::from_proto_extention(vec![0; 32], Vec::new(), 0, 0).unwrap(),
        );
        let provider = FilledProvider::new(
            RootProvider::new(transport),
            WalletFiller::new(TronWallet::default()),
        );

        let Err(err) = provider.send_transaction(transfer_from(Address::ZERO)).await else {
            panic!("signing without a credential should fail");
        };
        assert!(err.to_string().contains("missing signing credential"));
    }
}
