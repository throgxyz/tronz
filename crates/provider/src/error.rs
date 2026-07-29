//! Error types for `tronz-provider`.

use std::{error::Error as StdError, fmt};

use tronz_primitives::TxId;

/// What a node said about a call, in terms no one transport defines.
///
/// The canonical status set, which a transport maps *onto* rather than into: the
/// gRPC transport turns `tonic::Code` into these, and an HTTP one would turn its
/// own status codes into the same set, so the classifications built on top —
/// whether a call is worth retrying, whether a broadcast was definitely refused —
/// are written once and hold for both.
///
/// There is deliberately no `Other(i32)`: a bare integer here would be a gRPC code
/// in all but name, which is the coupling this type exists to remove. Anything
/// unrecognised is [`Unknown`](Self::Unknown). Nor is there an `Ok`: the type only
/// ever describes a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RpcStatusCode {
    /// The call was cancelled, typically by the caller.
    Cancelled,
    /// No more specific status applies, or the transport reported one this set has
    /// no name for.
    Unknown,
    /// The request was rejected as malformed or nonsensical.
    InvalidArgument,
    /// The call outlived its deadline.
    DeadlineExceeded,
    /// The thing asked for does not exist.
    NotFound,
    /// The thing being created already exists.
    AlreadyExists,
    /// The caller is not allowed to do this.
    PermissionDenied,
    /// A quota or rate limit is exhausted.
    ResourceExhausted,
    /// The system is not in a state where this call can succeed.
    FailedPrecondition,
    /// The call was aborted, often over a concurrency conflict.
    Aborted,
    /// An argument was outside the range the node can serve.
    OutOfRange,
    /// The node does not implement this call.
    Unimplemented,
    /// The node hit an internal error.
    Internal,
    /// The node is unreachable or not currently serving.
    Unavailable,
    /// Data was lost or corrupted irrecoverably.
    DataLoss,
    /// The caller did not authenticate.
    Unauthenticated,
}

impl fmt::Display for RpcStatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
            Self::InvalidArgument => "invalid argument",
            Self::DeadlineExceeded => "deadline exceeded",
            Self::NotFound => "not found",
            Self::AlreadyExists => "already exists",
            Self::PermissionDenied => "permission denied",
            Self::ResourceExhausted => "resource exhausted",
            Self::FailedPrecondition => "failed precondition",
            Self::Aborted => "aborted",
            Self::OutOfRange => "out of range",
            Self::Unimplemented => "unimplemented",
            Self::Internal => "internal",
            Self::Unavailable => "unavailable",
            Self::DataLoss => "data loss",
            Self::Unauthenticated => "unauthenticated",
        };
        f.write_str(name)
    }
}

/// Raw I/O and decoding failures from the transport layer.
///
/// `#[non_exhaustive]`: new transport backends may add variants in minor versions.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum TransportErrorKind {
    /// A status the node answered with, in terms this crate defines.
    #[error("rpc status {code}: {message}")]
    Rpc {
        /// What kind of failure the node reported.
        code: RpcStatusCode,
        /// What the node said about it.
        message: String,
    },

    /// The channel could not be established, or was lost.
    #[error("transport error: {0}")]
    Connect(#[source] Box<dyn StdError + Send + Sync + 'static>),

    /// A protobuf payload failed to decode.
    #[error("protobuf decode error: {0}")]
    Proto(#[from] prost::DecodeError),

    /// A response field was missing or had an unexpected shape.
    #[error("malformed response: {0}")]
    Malformed(String),

    /// The TRON node returned `Return { result: false }`.
    ///
    /// Promoted to [`RpcError::NodeError`] by the [`From`] impl.
    #[error("node error: {0}")]
    NodeError(String),

    /// The node does not offer this RPC.
    ///
    /// Several TRON endpoints are opt-in — `EstimateEnergy` needs
    /// `vm.estimateEnergy`, for one — and a node that has not enabled one says so
    /// rather than answering. Distinct from a call that failed, so a caller can fall
    /// back to another endpoint without mistaking a timeout for a missing feature.
    #[error("node does not support this call: {0}")]
    Unsupported(String),

    /// Custom or third-party transport error.
    #[error("{0}")]
    Custom(#[source] Box<dyn StdError + Send + Sync + 'static>),

    /// Deterministic, terminal failure that retry loops must not retry.
    #[error("{0}")]
    NonRetryable(#[source] Box<dyn StdError + Send + Sync + 'static>),
}

impl From<tronz_rpc_types::ResponseError> for TransportErrorKind {
    /// The three ways a response can be unusable carry over one for one; the
    /// transport adds the ways a call can fail before there is a response at all.
    fn from(err: tronz_rpc_types::ResponseError) -> Self {
        use tronz_rpc_types::ResponseError;

        match err {
            ResponseError::Decode(err) => Self::Proto(err),
            ResponseError::Malformed(msg) => Self::Malformed(msg),
            ResponseError::NodeError(msg) => Self::NodeError(msg),
        }
    }
}

impl TransportErrorKind {
    /// Wrap an arbitrary error as [`Custom`](Self::Custom).
    #[cold]
    pub fn custom(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Custom(Box::new(err))
    }

    /// Construct a [`Custom`](Self::Custom) error from a string.
    #[cold]
    pub fn custom_str(err: &str) -> Self {
        Self::Custom(err.to_string().into())
    }

    /// Construct a [`Malformed`](Self::Malformed) error.
    #[cold]
    pub fn malformed(msg: impl fmt::Display) -> Self {
        Self::Malformed(msg.to_string())
    }

    /// Wrap an arbitrary error as [`NonRetryable`](Self::NonRetryable).
    #[cold]
    pub fn non_retryable(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::NonRetryable(Box::new(err))
    }

    /// Construct a [`NonRetryable`](Self::NonRetryable) error from a string.
    #[cold]
    pub fn non_retryable_str(err: &str) -> Self {
        Self::NonRetryable(err.to_string().into())
    }

    /// A status the node reported, however the transport phrased it.
    #[cold]
    pub fn rpc(code: RpcStatusCode, message: impl Into<String>) -> Self {
        Self::Rpc { code, message: message.into() }
    }

    /// Wrap a channel-level failure as [`Connect`](Self::Connect).
    #[cold]
    pub fn connect(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Connect(Box::new(err))
    }

    /// Returns `true` if the error is likely transient and may be retried.
    pub fn is_retryable(&self) -> bool {
        match self {
            // `DeadlineExceeded` is intentionally excluded: with channel-level
            // `Endpoint::timeout()` it is almost always the client's own
            // request timeout firing, so retrying just multiplies latency.
            Self::Rpc { code, .. } => matches!(
                code,
                RpcStatusCode::Unavailable
                    | RpcStatusCode::ResourceExhausted
                    | RpcStatusCode::Aborted
            ),
            _ => false,
        }
    }

    /// Returns `true` if this is [`Rpc`](Self::Rpc).
    #[inline]
    pub const fn is_rpc(&self) -> bool {
        matches!(self, Self::Rpc { .. })
    }

    /// The status the node reported, if this is [`Rpc`](Self::Rpc).
    #[inline]
    pub const fn status_code(&self) -> Option<RpcStatusCode> {
        if let Self::Rpc { code, .. } = self { Some(*code) } else { None }
    }

    /// Returns `true` if this is [`Connect`](Self::Connect).
    #[inline]
    pub const fn is_connect(&self) -> bool {
        matches!(self, Self::Connect(_))
    }

    /// Returns `true` if this is [`Proto`](Self::Proto).
    #[inline]
    pub const fn is_proto(&self) -> bool {
        matches!(self, Self::Proto(_))
    }

    /// Returns `true` if this is [`Malformed`](Self::Malformed).
    #[inline]
    pub const fn is_malformed(&self) -> bool {
        matches!(self, Self::Malformed(_))
    }

    /// Returns `true` if this is [`NodeError`](Self::NodeError).
    #[inline]
    pub const fn is_node_error(&self) -> bool {
        matches!(self, Self::NodeError(_))
    }

    /// Returns `true` if this is [`Custom`](Self::Custom).
    #[inline]
    pub const fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }

    /// Returns `true` if this is [`NonRetryable`](Self::NonRetryable).
    #[inline]
    pub const fn is_non_retryable(&self) -> bool {
        matches!(self, Self::NonRetryable(_))
    }

    /// Returns the message if this is [`NodeError`](Self::NodeError).
    #[inline]
    pub fn as_node_error(&self) -> Option<&str> {
        if let Self::NodeError(msg) = self { Some(msg) } else { None }
    }

    /// Returns the message if this is [`Malformed`](Self::Malformed).
    #[inline]
    pub fn as_malformed(&self) -> Option<&str> {
        if let Self::Malformed(msg) = self { Some(msg) } else { None }
    }

    /// Returns the inner error if this is [`Custom`](Self::Custom).
    #[inline]
    pub const fn as_custom(&self) -> Option<&(dyn StdError + Send + Sync + 'static)> {
        if let Self::Custom(err) = self { Some(&**err) } else { None }
    }

    /// Returns the inner error if this is [`NonRetryable`](Self::NonRetryable).
    #[inline]
    pub const fn as_non_retryable(&self) -> Option<&(dyn StdError + Send + Sync + 'static)> {
        if let Self::NonRetryable(err) = self { Some(&**err) } else { None }
    }
}

/// Generic provider-layer error.
///
/// `E` is the transport kind — currently [`TransportErrorKind`] for gRPC.
/// The concrete alias for everyday use is [`ProviderError`].
///
/// `#[non_exhaustive]`: new variants may be added in minor versions.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum RpcError<E>
where
    E: StdError + 'static,
{
    /// A raw transport failure.
    #[error(transparent)]
    Transport(E),

    /// The TRON node returned an application-level failure
    /// (`Return { result: false }`).
    #[error("node error: {0}")]
    NodeError(String),

    /// Caller misuse: missing required field, no signer attached, invalid
    /// argument, etc.  Signer and address errors are also surfaced here so
    /// that `RpcError<E>` stays generic over concrete signer/primitive crates.
    #[error("local usage error: {0}")]
    LocalUsageError(#[source] Box<dyn StdError + Send + Sync + 'static>),

    /// A signed transaction went out, but the node's answer did not come back.
    ///
    /// Whether the transaction was accepted is unknown: the broadcast may not have
    /// arrived, or the reply may have been lost on the way back. Nothing is retried,
    /// since a second broadcast cannot be told apart from a node that took the first
    /// one — so the id is carried here instead, to be looked up.
    ///
    /// A node that answers with the transaction settles the question. A node that has
    /// never heard of it does not: it may not have indexed it yet, may be behind, or
    /// may not be the node the transaction reached. The way out is the expiry the
    /// transaction was built with — once it has passed, the transaction can no longer
    /// be included by anyone, and only then is building a replacement safe. Before
    /// then, keep asking, and prefer re-broadcasting the same signed bytes to signing
    /// a new transaction that could be included alongside the first.
    ///
    /// ```no_run
    /// # async fn recover(
    /// #     provider: &impl tronz_provider::TronProvider,
    /// #     err: tronz_provider::ProviderError,
    /// # ) -> tronz_provider::Result<()> {
    /// if let Some(tx_id) = err.tx_id() {
    ///     match provider.get_transaction(tx_id).await? {
    ///         Some(_) => println!("the node took it after all"),
    ///         // Not an answer yet — keep asking until the transaction expires.
    ///         None => println!("still unknown"),
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    #[error("transaction {tx_id} was broadcast but not acknowledged: {source}")]
    Broadcast {
        /// The id the transaction was signed under, computed locally before it was
        /// sent.
        tx_id: TxId,

        /// Why the broadcast did not complete.
        #[source]
        source: E,
    },
}

/// Promotes [`TransportErrorKind::NodeError`] to [`RpcError::NodeError`];
/// all other variants are wrapped as [`RpcError::Transport`].
impl From<TransportErrorKind> for RpcError<TransportErrorKind> {
    fn from(e: TransportErrorKind) -> Self {
        match e {
            TransportErrorKind::NodeError(msg) => Self::NodeError(msg),
            other => Self::Transport(other),
        }
    }
}

impl<E: StdError + 'static> RpcError<E> {
    /// Missing required builder field.
    #[cold]
    pub fn missing_field(name: &'static str) -> Self {
        Self::local_usage_str(&format!("missing required field: `{name}`"))
    }

    /// No signer is attached to this provider.
    #[cold]
    pub fn no_signer() -> Self {
        Self::local_usage_str("no signer attached to this provider")
    }

    /// Arbitrary caller-misuse error.
    #[cold]
    pub fn local_usage(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::LocalUsageError(Box::new(err))
    }

    /// Arbitrary caller-misuse message.
    #[cold]
    pub fn local_usage_str(err: &str) -> Self {
        Self::LocalUsageError(err.to_string().into())
    }

    /// Returns `true` if this is [`Transport`](Self::Transport).
    #[inline]
    pub const fn is_transport_error(&self) -> bool {
        matches!(self, Self::Transport(_))
    }

    /// Returns `true` if this is [`NodeError`](Self::NodeError).
    #[inline]
    pub const fn is_node_error(&self) -> bool {
        matches!(self, Self::NodeError(_))
    }

    /// Returns `true` if this is [`LocalUsageError`](Self::LocalUsageError).
    #[inline]
    pub const fn is_local_usage_error(&self) -> bool {
        matches!(self, Self::LocalUsageError(_))
    }

    /// Returns the node-rejection message if this is [`NodeError`](Self::NodeError).
    #[inline]
    pub fn as_node_error(&self) -> Option<&str> {
        if let Self::NodeError(msg) = self { Some(msg) } else { None }
    }

    /// Returns the inner transport error if this is [`Transport`](Self::Transport).
    #[inline]
    pub const fn as_transport_err(&self) -> Option<&E> {
        if let Self::Transport(e) = self { Some(e) } else { None }
    }

    /// The id of a transaction that was sent without being acknowledged.
    ///
    /// Present only on [`Broadcast`](Self::Broadcast), the one error that leaves a
    /// transaction's fate unknown.
    pub const fn tx_id(&self) -> Option<TxId> {
        if let Self::Broadcast { tx_id, .. } = self { Some(*tx_id) } else { None }
    }
}

impl RpcError<TransportErrorKind> {
    /// Lift a transport-layer error into a [`ProviderError`].
    #[inline]
    pub(crate) fn transport<E: Into<TransportErrorKind>>(e: E) -> Self {
        Self::from(e.into())
    }

    /// Classify a failed broadcast of the transaction with this id.
    ///
    /// Only a failure that leaves the outcome open becomes
    /// [`Broadcast`](Self::Broadcast). A node that answered — refusing the signature,
    /// the permission, the TAPOS reference — has told us it did not take the
    /// transaction, and saying otherwise would send the caller looking for something
    /// that was never there.
    ///
    /// Anything unclear is treated as unclear: mistaking a lost answer for a refusal
    /// invites a second broadcast of a transaction the chain already has, while the
    /// reverse costs only a lookup.
    pub(crate) fn broadcast(tx_id: TxId, source: TransportErrorKind) -> Self {
        let refused = match &source {
            // The node replied `Return { result: false }`.
            TransportErrorKind::NodeError(_) => true,
            // Never reached the point of being applied.
            TransportErrorKind::Rpc { code, .. } => matches!(
                code,
                RpcStatusCode::InvalidArgument
                    | RpcStatusCode::PermissionDenied
                    | RpcStatusCode::Unauthenticated
                    | RpcStatusCode::Unimplemented
            ),
            TransportErrorKind::Unsupported(_) => true,
            _ => false,
        };

        if refused { Self::from(source) } else { Self::Broadcast { tx_id, source } }
    }

    /// Returns `true` if the underlying transport error is retryable.
    #[inline]
    pub fn is_retryable(&self) -> bool {
        self.as_transport_err().is_some_and(TransportErrorKind::is_retryable)
    }
}

/// The standard provider error for the gRPC transport.
pub type ProviderError = RpcError<TransportErrorKind>;

/// Alias for [`ProviderError`].
pub type Error = ProviderError;

/// Convenient `Result` alias defaulting to [`ProviderError`].
pub type Result<T, E = ProviderError> = core::result::Result<T, E>;

/// Result alias for the raw transport layer.
pub type TransportResult<T> = core::result::Result<T, TransportErrorKind>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_that_may_answer_next_time_is_worth_retrying() {
        for code in
            [RpcStatusCode::Unavailable, RpcStatusCode::ResourceExhausted, RpcStatusCode::Aborted]
        {
            let err = TransportErrorKind::rpc(code, "");
            assert!(err.is_retryable(), "{code} should be retryable");
            assert!(err.is_rpc());
            assert_eq!(err.status_code(), Some(code));
        }
    }

    #[test]
    fn a_settled_answer_is_not_worth_retrying() {
        for code in [
            RpcStatusCode::DeadlineExceeded,
            RpcStatusCode::NotFound,
            RpcStatusCode::InvalidArgument,
        ] {
            let err = TransportErrorKind::rpc(code, "");
            assert!(!err.is_retryable(), "{code} should not be retryable");
        }
    }

    #[test]
    fn malformed_helpers() {
        let err = TransportErrorKind::malformed("bad payload");
        assert!(!err.is_retryable());
        assert!(err.is_malformed());
        assert_eq!(err.as_malformed(), Some("bad payload"));
    }

    #[test]
    fn node_error_helpers() {
        let err = TransportErrorKind::NodeError("contract failed".into());
        assert!(!err.is_retryable());
        assert!(err.is_node_error());
        assert_eq!(err.as_node_error(), Some("contract failed"));
    }

    #[test]
    fn non_retryable_helpers() {
        let err = TransportErrorKind::non_retryable_str("fatal");
        assert!(!err.is_retryable());
        assert!(err.is_non_retryable());
    }

    #[test]
    fn node_error_promoted_from_transport_kind() {
        let transport_err = TransportErrorKind::NodeError("contract failed".into());
        let rpc_err: ProviderError = transport_err.into();
        assert!(rpc_err.is_node_error());
        assert!(!rpc_err.is_transport_error());
        assert_eq!(rpc_err.as_node_error(), Some("contract failed"));
    }

    #[test]
    fn transport_error_wraps_non_node_kinds() {
        let transport_err = TransportErrorKind::malformed("bad");
        let rpc_err: ProviderError = transport_err.into();
        assert!(rpc_err.is_transport_error());
        assert!(!rpc_err.is_node_error());
    }

    #[test]
    fn rpc_is_retryable_delegates_to_transport() {
        let err =
            ProviderError::Transport(TransportErrorKind::rpc(RpcStatusCode::Unavailable, "down"));
        assert!(err.is_retryable());
    }

    #[test]
    fn local_usage_error_is_not_retryable() {
        let err = ProviderError::missing_field("to");
        assert!(err.is_local_usage_error());
        assert!(!err.is_transport_error());
        assert!(!err.is_retryable());
    }
    #[test]
    fn a_refused_broadcast_is_not_left_open() {
        let err = RpcError::broadcast(
            TxId::from([1u8; 32]),
            TransportErrorKind::NodeError("SIGERROR".into()),
        );

        assert!(matches!(err, RpcError::NodeError(_)), "{err}");
        assert_eq!(err.tx_id(), None);
    }

    #[test]
    fn a_rejected_request_is_not_left_open_either() {
        let err = RpcError::broadcast(
            TxId::from([1u8; 32]),
            TransportErrorKind::rpc(RpcStatusCode::InvalidArgument, "bad message"),
        );

        assert_eq!(err.tx_id(), None);
    }
    #[test]
    fn a_lost_broadcast_carries_the_id_to_look_up() {
        let tx_id = TxId::from([2u8; 32]);
        let err = RpcError::broadcast(
            tx_id,
            TransportErrorKind::rpc(RpcStatusCode::Unavailable, "connection reset"),
        );

        assert_eq!(err.tx_id(), Some(tx_id));
    }
}
