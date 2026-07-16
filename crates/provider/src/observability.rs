//! Internal helpers for observability fields.

use std::time::Instant;

use crate::error::TransportErrorKind;

pub(crate) const OUTCOME_OK: &str = "ok";
pub(crate) const OUTCOME_ERROR: &str = "error";
pub(crate) const OUTCOME_NODE_ERROR: &str = "node_error";
pub(crate) const OUTCOME_TIMEOUT: &str = "timeout";

/// Convert an optional start instant to a tracing-friendly millisecond value.
pub(crate) fn elapsed_ms(started_at: Option<Instant>) -> u64 {
    started_at
        .map(|started_at| u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

/// Return the canonical lowercase name of a gRPC status code.
pub(crate) fn grpc_code(error: &TransportErrorKind) -> Option<&'static str> {
    let TransportErrorKind::Grpc(status) = error else {
        return None;
    };

    Some(match status.code() {
        tonic::Code::Ok => "ok",
        tonic::Code::Cancelled => "cancelled",
        tonic::Code::Unknown => "unknown",
        tonic::Code::InvalidArgument => "invalid_argument",
        tonic::Code::DeadlineExceeded => "deadline_exceeded",
        tonic::Code::NotFound => "not_found",
        tonic::Code::AlreadyExists => "already_exists",
        tonic::Code::PermissionDenied => "permission_denied",
        tonic::Code::ResourceExhausted => "resource_exhausted",
        tonic::Code::FailedPrecondition => "failed_precondition",
        tonic::Code::Aborted => "aborted",
        tonic::Code::OutOfRange => "out_of_range",
        tonic::Code::Unimplemented => "unimplemented",
        tonic::Code::Internal => "internal",
        tonic::Code::Unavailable => "unavailable",
        tonic::Code::DataLoss => "data_loss",
        tonic::Code::Unauthenticated => "unauthenticated",
    })
}
