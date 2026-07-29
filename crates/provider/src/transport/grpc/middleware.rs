//! [`GrpcMiddleware`], for observing and pacing every call a gRPC transport makes.

use std::time::Duration;

use async_trait::async_trait;

use crate::error::TransportErrorKind;

/// The call a transport is about to make, or has just made.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct GrpcCall<'a> {
    /// The RPC, named as the transport calls it — `get_account`, `freeze_balance_v2`.
    pub method: &'a str,
}

impl<'a> GrpcCall<'a> {
    /// Name a call, for testing middleware of your own.
    pub const fn new(method: &'a str) -> Self {
        Self { method }
    }
}

/// How a call turned out.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct GrpcOutcome<'a> {
    /// Wall clock for the whole call, including any retries and the waits between.
    pub elapsed: Duration,
    /// How many attempts reached the node. More than one means a retry; zero means
    /// the call was refused in `before` and never made.
    pub attempts: u32,
    /// Why it failed, if it did.
    pub error: Option<&'a TransportErrorKind>,
}

impl GrpcOutcome<'_> {
    /// Whether the call succeeded.
    pub const fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Observes and paces every call a gRPC transport makes.
///
/// This is the seam that nothing above it can route around. A
/// [`ProviderLayer`](crate::ProviderLayer) only sees the methods it overrides, and
/// what it leaves alone reaches the node through
/// [`root`](crate::TronProvider::root) — as does a
/// [`PendingTransaction`](crate::provider::PendingTransaction)'s polling and an
/// event watcher's. Middleware sits below all of it, so it sees those too, which is
/// what makes it the place for metrics and for staying inside a node's rate limit.
///
/// Both methods are optional, and neither sees the request or response payload:
/// middleware is for timing, counting, and pacing, not for rewriting traffic. To
/// answer calls without a node, use
/// [`MockTransport`](crate::transport::mock::MockTransport) instead.
///
/// The transport's own retry runs below middleware, so one call is one logical RPC
/// however many attempts it takes — [`GrpcOutcome::attempts`] reports how many.
///
/// ```
/// use std::{
///     sync::{Arc, atomic::{AtomicU64, Ordering}},
///     time::Duration,
/// };
///
/// use async_trait::async_trait;
/// use tronz_provider::transport::grpc::{GrpcCall, GrpcMiddleware, GrpcOutcome};
///
/// /// Counts calls and totals the time they take.
/// #[derive(Default)]
/// struct Metrics {
///     calls: AtomicU64,
///     micros: AtomicU64,
/// }
///
/// #[async_trait]
/// impl GrpcMiddleware for Metrics {
///     async fn after(&self, call: GrpcCall<'_>, outcome: GrpcOutcome<'_>) {
///         self.calls.fetch_add(1, Ordering::Relaxed);
///         self.micros.fetch_add(outcome.elapsed.as_micros() as u64, Ordering::Relaxed);
///
///         if let Some(error) = outcome.error {
///             tracing::warn!(call.method, %error, "rpc failed");
///         }
///     }
/// }
///
/// # async fn run() -> Result<(), tronz_provider::TransportErrorKind> {
/// let metrics = Arc::new(Metrics::default());
///
/// let transport = tronz_provider::transport::grpc::GrpcTransport::builder()
///     .with_middleware(metrics.clone())
///     .connect("grpc.trongrid.io:50051")
///     .await?;
/// # let _ = transport;
/// # Ok(()) }
/// ```
#[async_trait]
pub trait GrpcMiddleware: Send + Sync + 'static {
    /// Runs before the call goes out, and may hold it back — awaiting a rate
    /// limiter here paces the transport.
    ///
    /// An error fails the call without contacting the node.
    async fn before(&self, call: GrpcCall<'_>) -> Result<(), TransportErrorKind> {
        let _ = call;
        Ok(())
    }

    /// Runs once the call has finished, whichever way it went — including when a
    /// later middleware's `before` refused it, in which case
    /// [`attempts`](GrpcOutcome::attempts) is zero. A middleware whose own `before`
    /// failed does not see `after`.
    async fn after(&self, call: GrpcCall<'_>, outcome: GrpcOutcome<'_>) {
        let _ = (call, outcome);
    }
}
