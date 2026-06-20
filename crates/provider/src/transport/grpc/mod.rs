//! tonic-backed gRPC transport targeting the TRON full-node WalletClient API.
//!
//! Default endpoint: `https://grpc.trongrid.io:443` (TronGrid mainnet, TLS).
//! For local/private nodes use `http://127.0.0.1:50051` (no TLS).

mod codec;

use std::{collections::HashMap, future::Future, time::Duration};

use futures::future::try_join_all;
use prost::Message as _;
use tonic::{
    metadata::MetadataValue,
    service::Interceptor,
    transport::{Channel, Endpoint},
};
use tronz_primitives::{Address, B256, ResourceCode, Trx, TxId};

use crate::{
    error::TransportErrorKind,
    proto::{
        self, EmptyMessage, database_client::DatabaseClient, wallet_client::WalletClient,
        wallet_extension_client::WalletExtensionClient,
    },
    transport::TronTransport,
    types::{
        AccountInfo, AccountNet, AccountPermissionUpdateContract, AccountResource, AssetInfo,
        AssetIssueContract, BlockInfo, CancelAllUnfreezeV2Contract, ChainProperties,
        ClearContractAbiContract, ConstantCallResult, CreateAccountContract, CreateSmartContract,
        CreateWitnessContract, DelegateResourceContract, DelegatedResource, DelegatedResourceIndex,
        ExchangeCreateContract, ExchangeInfo, ExchangeInjectContract, ExchangeTransactionContract,
        ExchangeWithdrawContract, FreezeBalanceV1Contract, FreezeBalanceV2Contract,
        MarketCancelOrderContract, MarketOrderInfo, MarketOrderPair, MarketPrice,
        MarketSellAssetContract, NodeAddress, NodeInfo, ParticipateAssetIssueContract,
        ProposalApproveContract, ProposalCreateContract, ProposalDeleteContract, ProposalInfo,
        RawTransaction, SetAccountIdContract, SignWeight, SignedTransaction, SmartContractInfo,
        TransactionInfo, TransferAssetContract, TransferContract, TriggerSmartContract,
        UnDelegateResourceContract, UnfreezeAssetContract, UnfreezeBalanceV1Contract,
        UnfreezeBalanceV2Contract, UpdateAccountContract, UpdateAssetContract,
        UpdateBrokerageContract, UpdateEnergyLimitContract, UpdateSettingContract,
        UpdateWitnessContract, VoteWitnessContract, WithdrawBalanceContract,
        WithdrawExpireUnfreezeContract, WitnessInfo,
    },
};

/// TronGrid mainnet gRPC endpoint (TLS).
pub const TRONGRID_MAINNET: &str = "https://grpc.trongrid.io:443";
/// TronGrid Nile testnet gRPC endpoint (plain HTTP/2, no TLS).
///
/// TronGrid's wildcard TLS cert (`*.trongrid.io`) does not cover the
/// three-level hostname `grpc.nile.trongrid.io`, so connect without TLS:
/// ```no_run
/// use tronz_provider::{ProviderBuilder, transport::grpc::TRONGRID_NILE};
/// # async fn run() -> tronz_provider::Result<()> {
/// let provider = ProviderBuilder::new().on_grpc(TRONGRID_NILE).await?;
/// # Ok(()) }
/// ```
pub const TRONGRID_NILE: &str = "http://grpc.nile.trongrid.io:50051";

/// tonic interceptor that injects the TronGrid API key as a request header.
#[derive(Clone)]
struct ApiKeyInterceptor(Option<String>);

impl Interceptor for ApiKeyInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if let Some(ref key) = self.0 {
            match MetadataValue::try_from(key.as_str()) {
                Ok(val) => {
                    req.metadata_mut().insert("tron-pro-api-key", val);
                }
                Err(_) => {
                    // Invalid ASCII — log and continue rather than hard-failing
                    // a potentially valid RPC call.
                    tracing::warn!(
                        "TronGrid API key contains non-ASCII characters; skipping header injection"
                    );
                }
            }
        }
        Ok(req)
    }
}

/// Shorthand for the intercepted wallet client type used throughout this module.
type WalletClientI = WalletClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

/// Shorthand for the intercepted wallet-extension client.
type WalletExtensionClientI =
    WalletExtensionClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

/// Shorthand for the intercepted database client.
type DatabaseClientI =
    DatabaseClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

/// Exponential back-off configuration for retryable gRPC errors.
///
/// `#[non_exhaustive]`: construct via [`RetryConfig::default`] (or
/// [`disabled`](Self::disabled)) and the `with_*` setters so future fields
/// (e.g. `retry-after` handling, jitter tuning) can be added without breaking
/// callers.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RetryConfig {
    /// Maximum number of attempts (including the first).
    ///
    /// Clamped to 1 internally, so `0` never panics — it just means
    /// "try once, no retries".
    pub max_attempts: u32,
    /// Back-off duration before the second attempt. Default: 500 ms.
    pub initial_backoff: Duration,
    /// Upper bound on the back-off after multiplier application. Default: 10 s.
    pub max_backoff: Duration,
    /// Multiplier applied to the base back-off after each failed attempt.
    /// Default: 2.0.
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// A policy that never retries — equivalent to `max_attempts = 1`.
    pub fn disabled() -> Self {
        Self {
            max_attempts: 1,
            ..Default::default()
        }
    }

    /// Set the maximum number of attempts (including the first).
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Set the back-off duration before the second attempt.
    pub fn with_initial_backoff(mut self, initial_backoff: Duration) -> Self {
        self.initial_backoff = initial_backoff;
        self
    }

    /// Set the upper bound on the back-off after multiplier application.
    pub fn with_max_backoff(mut self, max_backoff: Duration) -> Self {
        self.max_backoff = max_backoff;
        self
    }

    /// Set the multiplier applied to the base back-off after each failed attempt.
    pub fn with_backoff_multiplier(mut self, backoff_multiplier: f64) -> Self {
        self.backoff_multiplier = backoff_multiplier;
        self
    }
}

/// Full configuration for a [`GrpcTransport`] connection.
///
/// With defaults, the worst-case wall time before an error surfaces is roughly
/// `max_attempts × request_timeout + Σbackoff` ≈ `3 × 30 s + (0.5 s + 1.0 s)`.
/// Tune `retry`/`request_timeout`, or set `retry: RetryConfig::disabled()`, to
/// shorten it.
///
/// `#[non_exhaustive]`: construct via [`GrpcTransport::builder`] or
/// [`ProviderBuilder`](crate::ProviderBuilder) so future fields (e.g. an overall
/// deadline or failover endpoints) can be added without breaking callers.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct GrpcTransportConfig {
    /// Timeout for the initial TCP + TLS handshake. Default: 10 s.
    pub connect_timeout: Duration,
    /// Per-call deadline applied to every RPC on this channel via
    /// [`Endpoint::timeout`]. Each retry attempt gets a fresh copy. Default: 30 s.
    pub request_timeout: Duration,
    /// Retry policy for retryable errors.
    pub retry: RetryConfig,
    /// Optional TronGrid API key (`TRON-PRO-API-KEY` header).
    pub api_key: Option<String>,
    /// Additional, *equivalent* node endpoints for client-side failover.
    ///
    /// Combined with the primary `uri` passed to `connect`, these form a
    /// load-balanced pool. With two or more total endpoints the channel is
    /// built via [`Channel::balance_list`], which connects lazily and routes
    /// around failed peers; a single endpoint keeps the eager, fail-fast
    /// connect. All endpoints must serve the same API (and share the same TLS
    /// expectation). Default: empty.
    pub endpoints: Vec<String>,
}

impl Default for GrpcTransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            retry: RetryConfig::default(),
            api_key: None,
            endpoints: Vec::new(),
        }
    }
}

/// Pre-connect builder for [`GrpcTransport`].
///
/// Accumulates a [`GrpcTransportConfig`] via chainable `with_*` setters, then
/// [`connect`](Self::connect)s. This is the advanced entry point;
/// [`ProviderBuilder`](crate::ProviderBuilder) is the primary one.
#[derive(Clone, Debug, Default)]
pub struct GrpcTransportBuilder {
    config: GrpcTransportConfig,
}

impl GrpcTransportBuilder {
    /// Override the connect (handshake) timeout.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Override the per-call request timeout.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    /// Override the retry policy.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.config.retry = retry;
        self
    }

    /// Add equivalent node endpoints for client-side failover / load balancing.
    ///
    /// These join the primary `uri` passed to [`connect`](Self::connect); two or
    /// more total endpoints switch the channel to [`Channel::balance_list`].
    pub fn with_endpoints(mut self, endpoints: Vec<String>) -> Self {
        self.config.endpoints = endpoints;
        self
    }

    /// Optionally set the TronGrid API key.
    pub fn maybe_api_key(mut self, key: Option<impl Into<String>>) -> Self {
        self.config.api_key = key.map(Into::into);
        self
    }

    /// Connect using the accumulated configuration.
    pub async fn connect(self, uri: impl AsRef<str>) -> Result<GrpcTransport, TransportErrorKind> {
        GrpcTransport::connect_with_config(uri, self.config).await
    }
}

/// gRPC transport wrapping a tonic [`Channel`].
///
/// Cheap to clone — the channel is already `Arc`-backed.
#[derive(Clone)]
pub struct GrpcTransport {
    channel: Channel,
    api_key: Option<String>,
    retry: RetryConfig,
}

impl GrpcTransport {
    /// Connect to a TRON gRPC node with default timeouts and retry policy.
    ///
    /// `uri` may be:
    /// - `"https://grpc.trongrid.io:443"` (TronGrid mainnet, TLS)
    /// - `"http://127.0.0.1:50051"` (local node, plain HTTP/2)
    ///
    /// For custom timeouts / retry / API key use [`builder`](Self::builder).
    pub async fn connect(uri: impl AsRef<str>) -> Result<Self, TransportErrorKind> {
        Self::connect_with_config(uri, GrpcTransportConfig::default()).await
    }

    /// Start an advanced, pre-connect [`GrpcTransportBuilder`] (timeouts, retry,
    /// API key).
    pub fn builder() -> GrpcTransportBuilder {
        GrpcTransportBuilder::default()
    }

    /// Connect with an explicit [`GrpcTransportConfig`].
    ///
    /// Timeouts are baked into the tonic [`Endpoint`] so they cover every RPC
    /// on the resulting channel with no per-call code.
    ///
    /// With a single endpoint the connect is **eager** (`Endpoint::connect`),
    /// so an unreachable node fails fast here. With two or more endpoints
    /// (primary `uri` plus `cfg.endpoints`) the channel is built via
    /// [`Channel::balance_list`], which connects **lazily** and load-balances /
    /// fails over across peers — construction cannot fail-fast, so an
    /// unreachable pool surfaces on the first RPC instead.
    pub(crate) async fn connect_with_config(
        uri: impl AsRef<str>,
        cfg: GrpcTransportConfig,
    ) -> Result<Self, TransportErrorKind> {
        let mut uris = Vec::with_capacity(1 + cfg.endpoints.len());
        uris.push(uri.as_ref().to_owned());
        uris.extend(cfg.endpoints.iter().cloned());

        let channel = if uris.len() == 1 {
            Self::build_endpoint(&uris[0], &cfg)?.connect().await?
        } else {
            let endpoints = uris
                .iter()
                .map(|u| Self::build_endpoint(u, &cfg))
                .collect::<Result<Vec<_>, _>>()?;
            Channel::balance_list(endpoints.into_iter())
        };

        Ok(Self {
            channel,
            api_key: cfg.api_key,
            retry: cfg.retry,
        })
    }

    /// Build a tonic [`Endpoint`] from a URI, applying the connection timeouts
    /// and (when the `grpc-tls` feature is on) native-root TLS.
    fn build_endpoint(
        uri: &str,
        cfg: &GrpcTransportConfig,
    ) -> Result<Endpoint, TransportErrorKind> {
        let endpoint = Endpoint::from_shared(uri.to_owned())
            .map_err(|e| TransportErrorKind::Malformed(e.to_string()))?
            .connect_timeout(cfg.connect_timeout)
            .timeout(cfg.request_timeout);

        #[cfg(feature = "grpc-tls")]
        let endpoint = endpoint
            .tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
            .map_err(TransportErrorKind::Connect)?;

        Ok(endpoint)
    }

    /// Attach a TronGrid API key (sent as `TRON-PRO-API-KEY` header on each call).
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Retry `f` up to `retry.max_attempts` times on retryable errors, with
    /// exponential back-off and ±25 % jitter.
    ///
    /// `f` is `Fn` so it can be invoked once per attempt; each invocation should
    /// clone a fresh client + request into an `async move` future (see the
    /// `retry_unary!` macro) so nothing borrows `self` across `.await`.
    ///
    /// `broadcast_transaction` must **never** be wrapped in this helper
    /// (double-spend risk).
    async fn call_with_retry<F, Fut, T>(&self, f: F) -> Result<T, TransportErrorKind>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, TransportErrorKind>>,
    {
        let max = self.retry.max_attempts.max(1);
        let mut backoff = self.retry.initial_backoff;
        let mut attempt = 0u32;

        loop {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    attempt += 1;
                    if attempt >= max || !e.is_retryable() {
                        return Err(e);
                    }
                    // Jitter the sleep value only; advance the base backoff
                    // separately so randomness never accumulates in the sequence.
                    let jittered = backoff.mul_f64(0.75 + fastrand::f64() * 0.5);
                    tracing::debug!(
                        attempt,
                        backoff_ms = jittered.as_millis(),
                        error = %e,
                        "retrying gRPC call"
                    );
                    tokio::time::sleep(jittered).await;
                    backoff = backoff
                        .mul_f64(self.retry.backoff_multiplier)
                        .min(self.retry.max_backoff);
                }
            }
        }
    }

    fn wallet_client(&self) -> WalletClientI {
        WalletClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    fn wallet_extension_client(&self) -> WalletExtensionClientI {
        WalletExtensionClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    fn database_client(&self) -> DatabaseClientI {
        DatabaseClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    /// Check a `Return` message, converting failures to [`TransportErrorKind::NodeError`].
    fn check_return(ret: Option<proto::Return>) -> Result<(), TransportErrorKind> {
        if let Some(r) = ret {
            if !r.result {
                let msg = String::from_utf8_lossy(&r.message).into_owned();
                return Err(TransportErrorKind::NodeError(msg));
            }
        }
        Ok(())
    }

    /// Extract a [`RawTransaction`] from a [`proto::TransactionExtention`].
    fn raw_from_extention(
        ext: proto::TransactionExtention,
    ) -> Result<RawTransaction, TransportErrorKind> {
        Self::check_return(ext.result)?;

        let tx = ext.transaction.ok_or_else(|| {
            TransportErrorKind::Malformed("missing transaction in extention".into())
        })?;

        let (expiration, timestamp) = tx
            .raw_data
            .as_ref()
            .map(|r| (r.expiration, r.timestamp))
            .unwrap_or((0, 0));

        let raw_proto = tx.encode_to_vec();
        RawTransaction::from_proto_extention(ext.txid, raw_proto, expiration, timestamp)
    }
}

/// Decode a [`SignedTransaction`]'s unsigned proto and append its collected
/// signatures, producing the wire `Transaction` used for broadcast and for
/// sign-weight / approved-list queries.
fn signed_to_proto(tx: &SignedTransaction) -> Result<proto::Transaction, TransportErrorKind> {
    use prost::Message as _;

    let mut proto_tx = proto::Transaction::decode(tx.raw.raw_proto.as_ref())?;
    for sig in &tx.signatures {
        proto_tx.signature.push(sig.to_bytes().to_vec());
    }
    Ok(proto_tx)
}

/// Decode a lowercase hex string into bytes using only the standard library.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd number of hex digits".into());
    }
    s.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let hi = hex_digit(chunk[0])?;
            let lo = hex_digit(chunk[1])?;
            Ok((hi << 4) | lo)
        })
        .collect()
}

fn hex_digit(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex character: {}", b as char)),
    }
}

/// Route a unary gRPC call through [`GrpcTransport::call_with_retry`].
///
/// - `$client`: client accessor — `wallet_client`, `wallet_extension_client`, or `database_client`.
/// - `$method`: the generated tonic client method.
/// - `$req`: a **`Clone`** request *identifier* (all prost messages are `Clone`).
///
/// Clones a fresh client + request per attempt into an `async move` future, so
/// nothing borrows `self`/`$req` across `.await`. Never use for
/// `broadcast_transaction`.
macro_rules! retry_unary {
    ($self:ident, $client:ident, $method:ident, $req:ident) => {
        $self
            .call_with_retry(|| {
                let mut client = $self.$client();
                let req = $req.clone();
                // `?` converts tonic::Status -> TransportErrorKind via `#[from]`.
                async move { Ok(client.$method(req).await?.into_inner()) }
            })
            .await
    };
}

impl crate::transport::private::Sealed for GrpcTransport {}

impl TronTransport for GrpcTransport {
    type Error = TransportErrorKind;

    // --- Block ---

    async fn get_now_block(&self) -> Result<BlockInfo, Self::Error> {
        let req = EmptyMessage::default();
        let ext = retry_unary!(self, wallet_client, get_now_block2, req)?;
        codec::block_from_extention(ext)
    }

    async fn get_block_by_number(&self, num: i64) -> Result<BlockInfo, Self::Error> {
        let req = proto::NumberMessage { num };
        let ext = retry_unary!(self, wallet_client, get_block_by_num2, req)?;
        codec::block_from_extention(ext)
    }

    // --- Account ---

    async fn get_account(&self, address: Address) -> Result<AccountInfo, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let account = retry_unary!(self, wallet_client, get_account, req)?;
        codec::account_from_proto(account, address)
    }

    async fn get_account_resource(&self, address: Address) -> Result<AccountResource, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let res = retry_unary!(self, wallet_client, get_account_resource, req)?;
        Ok(codec::account_resource_from_proto(res))
    }

    // --- Transaction ---

    async fn broadcast_transaction(&self, tx: &SignedTransaction) -> Result<(), Self::Error> {
        let proto_tx = signed_to_proto(tx)?;

        let ret = self
            .wallet_client()
            .broadcast_transaction(proto_tx)
            .await?
            .into_inner();
        Self::check_return(Some(ret))
    }

    async fn get_transaction_by_id(&self, tx_id: TxId) -> Result<SignedTransaction, Self::Error> {
        let req = proto::BytesMessage {
            value: tx_id.as_slice().to_vec(),
        };
        let tx = retry_unary!(self, wallet_client, get_transaction_by_id, req)?;
        codec::signed_tx_from_proto(tx)
    }

    async fn get_transaction_info(
        &self,
        tx_id: TxId,
    ) -> Result<Option<TransactionInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: tx_id.as_slice().to_vec(),
        };
        let info = retry_unary!(self, wallet_client, get_transaction_info_by_id, req)?;
        codec::transaction_info_from_proto(info)
    }

    // --- Native contracts ---

    async fn transfer_trx(&self, params: TransferContract) -> Result<RawTransaction, Self::Error> {
        let req = codec::transfer_to_proto(params);
        let ext = retry_unary!(self, wallet_client, create_transaction2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn account_permission_update(
        &self,
        params: AccountPermissionUpdateContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::account_permission_update_to_proto(params);
        let ext = retry_unary!(self, wallet_client, account_permission_update, req)?;
        Self::raw_from_extention(ext)
    }

    async fn create_smart_contract(
        &self,
        params: CreateSmartContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::create_smart_contract_to_proto(params);
        let ext = retry_unary!(self, wallet_client, deploy_contract, req)?;
        Self::raw_from_extention(ext)
    }

    // --- Smart contracts ---

    async fn trigger_smart_contract(
        &self,
        params: TriggerSmartContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let ext = retry_unary!(self, wallet_client, trigger_contract, req)?;
        Self::raw_from_extention(ext)
    }

    async fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> Result<ConstantCallResult, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let ext = retry_unary!(self, wallet_client, trigger_constant_contract, req)?;
        codec::constant_result_from_extention(ext)
    }

    async fn estimate_energy(&self, params: TriggerSmartContract) -> Result<i64, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let msg = retry_unary!(self, wallet_client, estimate_energy, req)?;
        Self::check_return(msg.result)?;
        Ok(msg.energy_required)
    }

    // --- Staking ---

    async fn freeze_balance_v1(
        &self,
        params: FreezeBalanceV1Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::FreezeBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            frozen_balance: params.frozen_balance.as_sun(),
            frozen_duration: params.frozen_duration,
            resource: params.resource.as_i32(),
            receiver_address: params
                .receiver_address
                .map(|a| a.as_bytes().to_vec())
                .unwrap_or_default(),
        };
        let ext = retry_unary!(self, wallet_client, freeze_balance2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn unfreeze_balance_v1(
        &self,
        params: UnfreezeBalanceV1Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::UnfreezeBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            receiver_address: params
                .receiver_address
                .map(|a| a.as_bytes().to_vec())
                .unwrap_or_default(),
        };
        let ext = retry_unary!(self, wallet_client, unfreeze_balance2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn freeze_balance_v2(
        &self,
        params: FreezeBalanceV2Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::FreezeBalanceV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            frozen_balance: params.frozen_balance.as_sun(),
            resource: params.resource.as_i32(),
        };
        let ext = retry_unary!(self, wallet_client, freeze_balance_v2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn unfreeze_balance_v2(
        &self,
        params: UnfreezeBalanceV2Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::UnfreezeBalanceV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            unfreeze_balance: params.unfreeze_balance.as_sun(),
            resource: params.resource.as_i32(),
        };
        let ext = retry_unary!(self, wallet_client, unfreeze_balance_v2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn delegate_resource(
        &self,
        params: DelegateResourceContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::DelegateResourceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            balance: params.balance.as_sun(),
            receiver_address: params.receiver_address.as_bytes().to_vec(),
            lock: params.lock_period.is_some(),
            lock_period: params.lock_period.unwrap_or(0),
        };
        let ext = retry_unary!(self, wallet_client, delegate_resource, req)?;
        Self::raw_from_extention(ext)
    }

    async fn undelegate_resource(
        &self,
        params: UnDelegateResourceContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::UnDelegateResourceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            balance: params.balance.as_sun(),
            receiver_address: params.receiver_address.as_bytes().to_vec(),
        };
        let ext = retry_unary!(self, wallet_client, un_delegate_resource, req)?;
        Self::raw_from_extention(ext)
    }

    async fn withdraw_expire_unfreeze(
        &self,
        params: WithdrawExpireUnfreezeContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::WithdrawExpireUnfreezeContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let ext = retry_unary!(self, wallet_client, withdraw_expire_unfreeze, req)?;
        Self::raw_from_extention(ext)
    }

    async fn cancel_all_unfreeze_v2(
        &self,
        params: CancelAllUnfreezeV2Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::CancelAllUnfreezeV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let ext = retry_unary!(self, wallet_client, cancel_all_unfreeze_v2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn withdraw_balance(
        &self,
        params: WithdrawBalanceContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::WithdrawBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let ext = retry_unary!(self, wallet_client, withdraw_balance2, req)?;
        Self::raw_from_extention(ext)
    }

    // --- Resource queries ---

    async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>, Self::Error> {
        let req = proto::DelegatedResourceMessage {
            from_address: from.as_bytes().to_vec(),
            to_address: to.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_delegated_resource, req)?;
        list.delegated_resource
            .into_iter()
            .map(codec::delegated_resource_from_proto)
            .collect()
    }

    async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let idx = retry_unary!(
            self,
            wallet_client,
            get_delegated_resource_account_index,
            req
        )?;
        codec::delegated_resource_index_from_proto(idx)
    }

    async fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>, Self::Error> {
        let req = proto::DelegatedResourceMessage {
            from_address: from.as_bytes().to_vec(),
            to_address: to.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_delegated_resource_v2, req)?;
        list.delegated_resource
            .into_iter()
            .map(codec::delegated_resource_from_proto)
            .collect()
    }

    async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let idx = retry_unary!(
            self,
            wallet_client,
            get_delegated_resource_account_index_v2,
            req
        )?;
        codec::delegated_resource_index_from_proto(idx)
    }

    async fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> Result<Trx, Self::Error> {
        let req = proto::CanDelegatedMaxSizeRequestMessage {
            owner_address: address.as_bytes().to_vec(),
            r#type: resource.as_i32(),
        };
        let res = retry_unary!(self, wallet_client, get_can_delegated_max_size, req)?;
        Ok(Trx::from_sun_unchecked(res.max_size))
    }

    async fn get_reward(&self, address: Address) -> Result<Trx, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let res = retry_unary!(self, wallet_client, get_reward_info, req)?;
        Ok(Trx::from_sun_unchecked(res.num))
    }

    // --- Network ---

    async fn get_chain_parameters(&self) -> Result<HashMap<String, i64>, Self::Error> {
        let req = EmptyMessage::default();
        let params = retry_unary!(self, wallet_client, get_chain_parameters, req)?;
        Ok(params
            .chain_parameter
            .into_iter()
            .map(|p| (p.key, p.value))
            .collect())
    }

    async fn get_contract(&self, address: Address) -> Result<SmartContractInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let contract = retry_unary!(self, wallet_client, get_contract, req)?;
        Ok(codec::smart_contract_from_proto(contract))
    }

    async fn get_contract_info(&self, address: Address) -> Result<SmartContractInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let wrapper = retry_unary!(self, wallet_client, get_contract_info, req)?;
        Ok(codec::smart_contract_info_from_wrapper(wrapper))
    }

    async fn list_witnesses(&self) -> Result<Vec<WitnessInfo>, Self::Error> {
        let req = proto::EmptyMessage::default();
        let list = retry_unary!(self, wallet_client, list_witnesses, req)?;
        Ok(list
            .witnesses
            .into_iter()
            .filter_map(codec::witness_from_proto)
            .collect())
    }

    // --- Governance ---

    async fn proposal_create(
        &self,
        params: ProposalCreateContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::proposal_create_to_proto(params);
        let ext = retry_unary!(self, wallet_client, proposal_create, req)?;
        Self::raw_from_extention(ext)
    }

    async fn proposal_approve(
        &self,
        params: ProposalApproveContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::proposal_approve_to_proto(params);
        let ext = retry_unary!(self, wallet_client, proposal_approve, req)?;
        Self::raw_from_extention(ext)
    }

    async fn proposal_delete(
        &self,
        params: ProposalDeleteContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::proposal_delete_to_proto(params);
        let ext = retry_unary!(self, wallet_client, proposal_delete, req)?;
        Self::raw_from_extention(ext)
    }

    async fn list_proposals(&self) -> Result<Vec<ProposalInfo>, Self::Error> {
        let req = proto::EmptyMessage::default();
        let list = retry_unary!(self, wallet_client, list_proposals, req)?;
        Ok(list
            .proposals
            .into_iter()
            .map(codec::proposal_from_proto)
            .collect())
    }

    async fn get_paginated_proposal_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ProposalInfo>, Self::Error> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = retry_unary!(self, wallet_client, get_paginated_proposal_list, req)?;
        Ok(list
            .proposals
            .into_iter()
            .map(codec::proposal_from_proto)
            .collect())
    }

    async fn get_proposal_by_id(&self, proposal_id: i64) -> Result<ProposalInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: proposal_id.to_be_bytes().to_vec(),
        };
        let proposal = retry_unary!(self, wallet_client, get_proposal_by_id, req)?;
        Ok(codec::proposal_from_proto(proposal))
    }

    // --- TRC10 ---

    async fn create_asset_issue(
        &self,
        params: AssetIssueContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::asset_issue_to_proto(params);
        let ext = retry_unary!(self, wallet_client, create_asset_issue2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn transfer_asset(
        &self,
        params: TransferAssetContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::transfer_asset_to_proto(params);
        let ext = retry_unary!(self, wallet_client, transfer_asset2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn get_asset_issue_by_id(
        &self,
        token_id: &str,
    ) -> Result<Option<AssetInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: token_id.as_bytes().to_vec(),
        };
        let asset = retry_unary!(self, wallet_client, get_asset_issue_by_id, req)?;
        codec::asset_info_from_proto(asset)
    }

    async fn get_asset_issue_by_account(
        &self,
        address: Address,
    ) -> Result<Vec<AssetInfo>, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let list = retry_unary!(self, wallet_client, get_asset_issue_by_account, req)?;
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect()
    }

    async fn get_paginated_asset_issue_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<AssetInfo>, Self::Error> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = retry_unary!(self, wallet_client, get_paginated_asset_issue_list, req)?;
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect()
    }

    async fn get_asset_issue_by_name(&self, name: &str) -> Result<Option<AssetInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: name.as_bytes().to_vec(),
        };
        let asset = retry_unary!(self, wallet_client, get_asset_issue_by_name, req)?;
        codec::asset_info_from_proto(asset)
    }

    async fn get_asset_issue_list_by_name(
        &self,
        name: &str,
    ) -> Result<Vec<AssetInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: name.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_asset_issue_list_by_name, req)?;
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect()
    }

    async fn participate_asset_issue(
        &self,
        params: ParticipateAssetIssueContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::participate_asset_issue_to_proto(params);
        let ext = retry_unary!(self, wallet_client, participate_asset_issue2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn unfreeze_asset(
        &self,
        params: UnfreezeAssetContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::unfreeze_asset_to_proto(params);
        let ext = retry_unary!(self, wallet_client, unfreeze_asset2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn update_asset(
        &self,
        params: UpdateAssetContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_asset_to_proto(params);
        let ext = retry_unary!(self, wallet_client, update_asset2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn create_account(
        &self,
        params: CreateAccountContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::create_account_to_proto(params);
        let ext = retry_unary!(self, wallet_client, create_account2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn vote_witness_account(
        &self,
        params: VoteWitnessContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::vote_witness_to_proto(params);
        let ext = retry_unary!(self, wallet_client, vote_witness_account2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn update_account(
        &self,
        params: UpdateAccountContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_account_to_proto(params);
        let ext = retry_unary!(self, wallet_client, update_account2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn set_account_id(
        &self,
        params: SetAccountIdContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::set_account_id_to_proto(params);
        // SetAccountId only has a v1 endpoint (returns Transaction, not TransactionExtention).
        let tx = retry_unary!(self, wallet_client, set_account_id, req)?;
        codec::raw_from_plain(tx)
    }

    async fn clear_contract_abi(
        &self,
        params: ClearContractAbiContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::clear_contract_abi_to_proto(params);
        let ext = retry_unary!(self, wallet_client, clear_contract_abi, req)?;
        Self::raw_from_extention(ext)
    }

    async fn update_setting(
        &self,
        params: UpdateSettingContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_setting_to_proto(params);
        let ext = retry_unary!(self, wallet_client, update_setting, req)?;
        Self::raw_from_extention(ext)
    }

    async fn update_energy_limit(
        &self,
        params: UpdateEnergyLimitContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_energy_limit_to_proto(params);
        let ext = retry_unary!(self, wallet_client, update_energy_limit, req)?;
        Self::raw_from_extention(ext)
    }

    async fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> Result<Trx, Self::Error> {
        let req = proto::CanWithdrawUnfreezeAmountRequestMessage {
            owner_address: address.as_bytes().to_vec(),
            timestamp: timestamp_ms,
        };
        let res = retry_unary!(self, wallet_client, get_can_withdraw_unfreeze_amount, req)?;
        Ok(Trx::from_sun_unchecked(res.amount))
    }

    async fn get_available_unfreeze_count(&self, address: Address) -> Result<i64, Self::Error> {
        let req = proto::GetAvailableUnfreezeCountRequestMessage {
            owner_address: address.as_bytes().to_vec(),
        };
        let res = retry_unary!(self, wallet_client, get_available_unfreeze_count, req)?;
        Ok(res.count)
    }

    // --- Pricing / fees ---

    async fn get_bandwidth_prices(&self) -> Result<String, Self::Error> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_bandwidth_prices, req)?;
        Ok(res.prices)
    }

    async fn get_energy_prices(&self) -> Result<String, Self::Error> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_energy_prices, req)?;
        Ok(res.prices)
    }

    async fn get_memo_fee(&self) -> Result<u64, Self::Error> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_memo_fee, req)?;
        Ok(res.prices.parse::<u64>().unwrap_or(0))
    }

    // --- Network / chain ---

    async fn get_next_maintenance_time(&self) -> Result<i64, Self::Error> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_next_maintenance_time, req)?;
        Ok(res.num)
    }

    async fn get_burn_trx(&self) -> Result<u64, Self::Error> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_burn_trx, req)?;
        Ok(res.num as u64)
    }

    async fn get_total_transactions(&self) -> Result<u64, Self::Error> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, total_transaction, req)?;
        Ok(res.num as u64)
    }

    async fn get_node_info(&self) -> Result<NodeInfo, Self::Error> {
        let req = EmptyMessage::default();
        let info = retry_unary!(self, wallet_client, get_node_info, req)?;
        Ok(NodeInfo {
            block: info.block,
            solidity_block: info.solidity_block,
            peer_num: info.current_connect_count,
        })
    }

    async fn list_nodes(&self) -> Result<Vec<NodeAddress>, Self::Error> {
        let req = EmptyMessage::default();
        let list = retry_unary!(self, wallet_client, list_nodes, req)?;
        Ok(list
            .nodes
            .into_iter()
            .filter_map(|n| {
                n.address.map(|a| NodeAddress {
                    host: String::from_utf8_lossy(&a.host).into_owned(),
                    port: a.port,
                })
            })
            .collect())
    }

    async fn get_dynamic_properties(&self) -> Result<ChainProperties, Self::Error> {
        let req = EmptyMessage::default();
        let props = retry_unary!(self, database_client, get_dynamic_properties, req)?;
        // DynamicProperties only has last_solidity_block_num; use block ref for head info.
        // Return what the proto gives us directly.
        Ok(ChainProperties {
            head_block_id: String::new(),
            head_block_num: props.last_solidity_block_num,
            head_block_time_stamp: 0,
        })
    }

    // --- Block queries ---

    async fn get_block_by_id(&self, block_id: B256) -> Result<BlockInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: block_id.as_slice().to_vec(),
        };
        let block = retry_unary!(self, wallet_client, get_block_by_id, req)?;
        codec::block_from_plain(block)
    }

    async fn get_blocks_by_latest_num(&self, count: i64) -> Result<Vec<BlockInfo>, Self::Error> {
        let req = proto::NumberMessage { num: count };
        let list = retry_unary!(self, wallet_client, get_block_by_latest_num2, req)?;
        list.block
            .into_iter()
            .map(codec::block_from_extention)
            .collect()
    }

    async fn get_blocks_by_limit(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<BlockInfo>, Self::Error> {
        let req = proto::BlockLimit {
            start_num: start,
            end_num: end,
        };
        let list = retry_unary!(self, wallet_client, get_block_by_limit_next2, req)?;
        list.block
            .into_iter()
            .map(codec::block_from_extention)
            .collect()
    }

    async fn get_transaction_count_by_block_num(&self, block_num: i64) -> Result<u64, Self::Error> {
        let req = proto::NumberMessage { num: block_num };
        let res = retry_unary!(self, wallet_client, get_transaction_count_by_block_num, req)?;
        Ok(res.num as u64)
    }

    // --- Transaction history ---

    async fn get_transactions_from(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<RawTransaction>, Self::Error> {
        let req = proto::AccountPaginated {
            account: Some(proto::Account {
                address: address.as_bytes().to_vec(),
                ..Default::default()
            }),
            offset,
            limit,
        };
        let list = retry_unary!(
            self,
            wallet_extension_client,
            get_transactions_from_this2,
            req
        )?;
        list.transaction
            .into_iter()
            .map(GrpcTransport::raw_from_extention)
            .collect()
    }

    async fn get_transactions_to(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<RawTransaction>, Self::Error> {
        let req = proto::AccountPaginated {
            account: Some(proto::Account {
                address: address.as_bytes().to_vec(),
                ..Default::default()
            }),
            offset,
            limit,
        };
        let list = retry_unary!(
            self,
            wallet_extension_client,
            get_transactions_to_this2,
            req
        )?;
        list.transaction
            .into_iter()
            .map(GrpcTransport::raw_from_extention)
            .collect()
    }

    async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> Result<Vec<TransactionInfo>, Self::Error> {
        let req = proto::NumberMessage { num: block_num };
        let list = retry_unary!(self, wallet_client, get_transaction_info_by_block_num, req)?;
        list.transaction_info
            .into_iter()
            .filter_map(|info| codec::transaction_info_from_proto(info).transpose())
            .collect()
    }

    // --- Pending pool ---

    async fn get_pending_size(&self) -> Result<u64, Self::Error> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_pending_size, req)?;
        Ok(res.num as u64)
    }

    async fn get_transaction_from_pending(
        &self,
        tx_id: TxId,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::BytesMessage {
            value: tx_id.as_slice().to_vec(),
        };
        let tx = retry_unary!(self, wallet_client, get_transaction_from_pending, req)?;
        codec::raw_from_plain(tx)
    }

    async fn get_pending_transactions(&self) -> Result<Vec<RawTransaction>, Self::Error> {
        // GetTransactionListFromPending returns TransactionIdList (list of tx id hex strings).
        let req = EmptyMessage::default();
        let id_list = retry_unary!(self, wallet_client, get_transaction_list_from_pending, req)?;

        // Fan out all per-ID fetches concurrently (mirrors alloy's try_join_all pattern)
        // rather than issuing N sequential RPC calls.
        let futs = id_list.tx_id.into_iter().map(|tx_id_hex| {
            let transport = self.clone();
            async move {
                let id_bytes = decode_hex(&tx_id_hex)
                    .map_err(|e| TransportErrorKind::Malformed(format!("bad tx id hex: {e}")))?;
                let req = proto::BytesMessage { value: id_bytes };
                let tx = retry_unary!(transport, wallet_client, get_transaction_from_pending, req)?;
                codec::raw_from_plain(tx)
            }
        });
        try_join_all(futs).await
    }

    // --- Multi-sig ---

    async fn get_transaction_sign_weight(
        &self,
        tx: &SignedTransaction,
    ) -> Result<SignWeight, Self::Error> {
        let proto_tx = signed_to_proto(tx)?;
        let weight = retry_unary!(self, wallet_client, get_transaction_sign_weight, proto_tx)?;
        codec::sign_weight_from_proto(weight)
    }

    async fn get_transaction_approved_list(
        &self,
        tx: &SignedTransaction,
    ) -> Result<Vec<Address>, Self::Error> {
        let proto_tx = signed_to_proto(tx)?;
        let approved = retry_unary!(self, wallet_client, get_transaction_approved_list, proto_tx)?;
        approved
            .approved_list
            .into_iter()
            .map(|bytes| {
                Address::from_slice(&bytes)
                    .map_err(|e| TransportErrorKind::Malformed(format!("bad address: {e}")))
            })
            .collect()
    }

    // --- Account net ---

    async fn get_account_net(&self, address: Address) -> Result<AccountNet, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let msg = retry_unary!(self, wallet_client, get_account_net, req)?;
        Ok(AccountNet {
            free_net_used: msg.free_net_used,
            free_net_limit: msg.free_net_limit,
            net_used: msg.net_used,
            net_limit: msg.net_limit,
            total_net_weight: msg.total_net_weight,
            energy_used: 0,
            energy_limit: 0,
            total_energy_weight: 0,
        })
    }

    // --- Witness ---

    async fn create_witness(
        &self,
        params: CreateWitnessContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::create_witness_to_proto(params);
        let ext = retry_unary!(self, wallet_client, create_witness2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn update_witness(
        &self,
        params: UpdateWitnessContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_witness_to_proto(params);
        let ext = retry_unary!(self, wallet_client, update_witness2, req)?;
        Self::raw_from_extention(ext)
    }

    async fn update_brokerage(
        &self,
        params: UpdateBrokerageContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_brokerage_to_proto(params);
        let ext = retry_unary!(self, wallet_client, update_brokerage, req)?;
        Self::raw_from_extention(ext)
    }

    async fn get_brokerage(&self, address: Address) -> Result<u64, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let res = retry_unary!(self, wallet_client, get_brokerage_info, req)?;
        Ok(res.num as u64)
    }

    async fn get_reward_info(&self, address: Address) -> Result<u64, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let res = retry_unary!(self, wallet_client, get_reward_info, req)?;
        Ok(res.num as u64)
    }

    // --- DEX (Bancor exchange) ---

    async fn exchange_create(
        &self,
        params: ExchangeCreateContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::exchange_create_to_proto(params);
        let ext = retry_unary!(self, wallet_client, exchange_create, req)?;
        Self::raw_from_extention(ext)
    }

    async fn exchange_inject(
        &self,
        params: ExchangeInjectContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::exchange_inject_to_proto(params);
        let ext = retry_unary!(self, wallet_client, exchange_inject, req)?;
        Self::raw_from_extention(ext)
    }

    async fn exchange_withdraw(
        &self,
        params: ExchangeWithdrawContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::exchange_withdraw_to_proto(params);
        let ext = retry_unary!(self, wallet_client, exchange_withdraw, req)?;
        Self::raw_from_extention(ext)
    }

    async fn exchange_transaction(
        &self,
        params: ExchangeTransactionContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::exchange_transaction_to_proto(params);
        let ext = retry_unary!(self, wallet_client, exchange_transaction, req)?;
        Self::raw_from_extention(ext)
    }

    async fn list_exchanges(&self) -> Result<Vec<ExchangeInfo>, Self::Error> {
        let req = EmptyMessage {};
        let list = retry_unary!(self, wallet_client, list_exchanges, req)?;
        list.exchanges
            .into_iter()
            .map(codec::exchange_info_from_proto)
            .collect()
    }

    async fn get_paginated_exchange_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ExchangeInfo>, Self::Error> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = retry_unary!(self, wallet_client, get_paginated_exchange_list, req)?;
        list.exchanges
            .into_iter()
            .map(codec::exchange_info_from_proto)
            .collect()
    }

    async fn get_exchange_by_id(
        &self,
        exchange_id: i64,
    ) -> Result<Option<ExchangeInfo>, Self::Error> {
        let mut id_bytes = [0u8; 8];
        id_bytes.copy_from_slice(&exchange_id.to_be_bytes());
        let req = proto::BytesMessage {
            value: id_bytes.to_vec(),
        };
        let exchange = retry_unary!(self, wallet_client, get_exchange_by_id, req)?;
        if exchange.exchange_id == 0 && exchange.creator_address.is_empty() {
            return Ok(None);
        }
        Ok(Some(codec::exchange_info_from_proto(exchange)?))
    }

    // --- Market (order-book DEX) ---

    async fn market_sell_asset(
        &self,
        params: MarketSellAssetContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::market_sell_asset_to_proto(params);
        let ext = retry_unary!(self, wallet_client, market_sell_asset, req)?;
        Self::raw_from_extention(ext)
    }

    async fn market_cancel_order(
        &self,
        params: MarketCancelOrderContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::market_cancel_order_to_proto(params);
        let ext = retry_unary!(self, wallet_client, market_cancel_order, req)?;
        Self::raw_from_extention(ext)
    }

    async fn get_market_order_by_id(
        &self,
        order_id: &[u8],
    ) -> Result<Option<MarketOrderInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: order_id.to_vec(),
        };
        let order = retry_unary!(self, wallet_client, get_market_order_by_id, req)?;
        if order.order_id.is_empty() {
            return Ok(None);
        }
        Ok(Some(codec::market_order_from_proto(order)?))
    }

    async fn get_market_order_by_account(
        &self,
        address: Address,
    ) -> Result<Vec<MarketOrderInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_market_order_by_account, req)?;
        list.orders
            .into_iter()
            .map(codec::market_order_from_proto)
            .collect()
    }

    async fn get_market_price_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> Result<Vec<MarketPrice>, Self::Error> {
        let req = proto::MarketOrderPair {
            sell_token_id: sell_token_id.as_bytes().to_vec(),
            buy_token_id: buy_token_id.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_market_price_by_pair, req)?;
        Ok(list
            .prices
            .into_iter()
            .map(codec::market_price_from_proto)
            .collect())
    }

    async fn get_market_order_list_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> Result<Vec<MarketOrderInfo>, Self::Error> {
        let req = proto::MarketOrderPair {
            sell_token_id: sell_token_id.as_bytes().to_vec(),
            buy_token_id: buy_token_id.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_market_order_list_by_pair, req)?;
        list.orders
            .into_iter()
            .map(codec::market_order_from_proto)
            .collect()
    }

    async fn get_market_pair_list(&self) -> Result<Vec<MarketOrderPair>, Self::Error> {
        let req = EmptyMessage {};
        let list = retry_unary!(self, wallet_client, get_market_pair_list, req)?;
        Ok(list
            .order_pair
            .into_iter()
            .map(codec::market_order_pair_from_proto)
            .collect())
    }
}
