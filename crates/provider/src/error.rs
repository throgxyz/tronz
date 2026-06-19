//! Error types for `tronz-provider`.

use std::{error::Error as StdError, fmt};

/// Raw I/O and decoding failures from the transport layer.
///
/// `#[non_exhaustive]`: new transport backends may add variants in minor versions.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum TransportErrorKind {
    /// The gRPC channel returned an error status.
    #[error("gRPC status {}: {}", .0.code(), .0.message())]
    Grpc(#[from] tonic::Status),

    /// Failed to establish or configure the gRPC channel.
    #[error("gRPC transport error: {0}")]
    Connect(#[from] tonic::transport::Error),

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

    /// Custom or third-party transport error.
    #[error("{0}")]
    Custom(#[source] Box<dyn StdError + Send + Sync + 'static>),

    /// Deterministic, terminal failure that retry loops must not retry.
    #[error("{0}")]
    NonRetryable(#[source] Box<dyn StdError + Send + Sync + 'static>),
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

    /// Returns `true` if the error is likely transient and may be retried.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Grpc(s) => matches!(
                s.code(),
                tonic::Code::Unavailable
                    | tonic::Code::ResourceExhausted
                    | tonic::Code::DeadlineExceeded
                    | tonic::Code::Aborted
            ),
            _ => false,
        }
    }

    /// Returns `true` if this is [`Grpc`](Self::Grpc).
    #[inline]
    pub const fn is_grpc(&self) -> bool {
        matches!(self, Self::Grpc(_))
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
        if let Self::NodeError(msg) = self {
            Some(msg)
        } else {
            None
        }
    }

    /// Returns the message if this is [`Malformed`](Self::Malformed).
    #[inline]
    pub fn as_malformed(&self) -> Option<&str> {
        if let Self::Malformed(msg) = self {
            Some(msg)
        } else {
            None
        }
    }

    /// Returns the inner error if this is [`Custom`](Self::Custom).
    #[inline]
    pub const fn as_custom(&self) -> Option<&(dyn StdError + Send + Sync + 'static)> {
        if let Self::Custom(err) = self {
            Some(&**err)
        } else {
            None
        }
    }

    /// Returns the inner error if this is [`NonRetryable`](Self::NonRetryable).
    #[inline]
    pub const fn as_non_retryable(&self) -> Option<&(dyn StdError + Send + Sync + 'static)> {
        if let Self::NonRetryable(err) = self {
            Some(&**err)
        } else {
            None
        }
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
        if let Self::NodeError(msg) = self {
            Some(msg)
        } else {
            None
        }
    }

    /// Returns the inner transport error if this is [`Transport`](Self::Transport).
    #[inline]
    pub const fn as_transport_err(&self) -> Option<&E> {
        if let Self::Transport(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl RpcError<TransportErrorKind> {
    /// Returns `true` if the underlying transport error is retryable.
    #[inline]
    pub fn is_retryable(&self) -> bool {
        self.as_transport_err()
            .is_some_and(TransportErrorKind::is_retryable)
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
