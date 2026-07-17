//! Shared tonic connection, interception, and retry machinery.

use std::{future::Future, time::Duration};

use tonic::{
    GrpcMethod, Request,
    client::Grpc,
    codegen::http::uri::PathAndQuery,
    metadata::MetadataValue,
    service::Interceptor,
    transport::{Channel, Endpoint},
};

use crate::{
    error::TransportErrorKind,
    proto::{
        database_client::DatabaseClient, wallet_client::WalletClient,
        wallet_extension_client::WalletExtensionClient,
        wallet_solidity_client::WalletSolidityClient,
    },
};

/// Tonic interceptor that injects the TronGrid API key as a request header.
#[derive(Clone)]
pub(super) struct ApiKeyInterceptor(Option<String>);

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
                    warn!(
                        "TronGrid API key contains non-ASCII characters; skipping header injection"
                    );
                }
            }
        }
        Ok(req)
    }
}

pub(super) type WalletClientI =
    WalletClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

pub(super) type WalletExtensionClientI =
    WalletExtensionClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

pub(super) type DatabaseClientI =
    DatabaseClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

pub(super) type WalletSolidityClientI =
    WalletSolidityClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

/// Exponential back-off configuration for retryable gRPC errors.
///
/// Construct with [`RetryConfig::default`] or [`RetryConfig::disabled`] and
/// customize it through the `with_*` setters.
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
        Self { max_attempts: 1, ..Default::default() }
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

/// Full configuration for a gRPC connection.
///
/// Construct through a transport builder. Default retries can take roughly
/// 92 seconds before surfacing an error.
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

/// Shared gRPC connection state.
///
/// Used by the FullNode and SolidityNode transports.
#[derive(Clone)]
pub(super) struct GrpcCore {
    channel: Channel,
    api_key: Option<String>,
    retry: RetryConfig,
}

impl GrpcCore {
    pub(super) fn set_api_key(&mut self, key: String) {
        self.api_key = Some(key);
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
    pub(super) async fn connect_with_config(
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

        Ok(Self { channel, api_key: cfg.api_key, retry: cfg.retry })
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

    /// Retry `f` up to `retry.max_attempts` times on retryable errors, with
    /// exponential back-off and ±25 % jitter.
    ///
    /// `f` is `Fn` so it can be invoked once per attempt; each invocation should
    /// clone a fresh client + request into an `async move` future (see the
    /// `retry_unary!` macro) so nothing borrows `self` across `.await`.
    ///
    /// Broadcasts deliberately bypass this helper because a lost response
    /// leaves transaction acceptance ambiguous.
    pub(super) async fn call_with_retry<F, Fut, T>(&self, f: F) -> Result<T, TransportErrorKind>
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
                    debug!(
                        attempt,
                        backoff_ms = jittered.as_millis(),
                        error = %e,
                        "retrying gRPC call"
                    );
                    tokio::time::sleep(jittered).await;
                    backoff =
                        backoff.mul_f64(self.retry.backoff_multiplier).min(self.retry.max_backoff);
                }
            }
        }
    }

    pub(super) fn wallet_client(&self) -> WalletClientI {
        WalletClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    pub(super) fn wallet_extension_client(&self) -> WalletExtensionClientI {
        WalletExtensionClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    pub(super) fn database_client(&self) -> DatabaseClientI {
        DatabaseClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    pub(super) fn wallet_solidity_client(&self) -> WalletSolidityClientI {
        WalletSolidityClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    /// Calls a unary RPC using a custom wire-compatible response type.
    ///
    /// Generated clients fix the response type to the full TRON protobuf
    /// message. Block-summary methods use this helper so prost can skip the
    /// transaction payload instead of decoding data the public API discards.
    pub(super) async fn unary<Req, Res>(
        &self,
        req: Req,
        path: &'static str,
        service: &'static str,
        method: &'static str,
    ) -> Result<Res, TransportErrorKind>
    where
        Req: prost::Message + Default + Clone + Send + Sync + 'static,
        Res: prost::Message + Default + Send + Sync + 'static,
    {
        self.call_with_retry(|| {
            let intercepted = tonic::codegen::InterceptedService::new(
                self.channel.clone(),
                ApiKeyInterceptor(self.api_key.clone()),
            );
            let req = req.clone();
            async move {
                let mut client = Grpc::new(intercepted);
                client.ready().await.map_err(|e| {
                    TransportErrorKind::Grpc(tonic::Status::unknown(format!(
                        "service was not ready: {}",
                        e
                    )))
                })?;
                let mut request = Request::new(req);
                request.extensions_mut().insert(GrpcMethod::new(service, method));
                let codec = tonic_prost::ProstCodec::default();
                Ok(client
                    .unary(request, PathAndQuery::from_static(path), codec)
                    .await?
                    .into_inner())
            }
        })
        .await
    }
}
