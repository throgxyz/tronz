//! tonic-backed gRPC transports for TRON FullNode and SolidityNode services.

#[cfg(test)]
mod capture;
mod core;
mod fullnode;
mod middleware;
mod solidity;

use core::GrpcCore;
pub use core::{GrpcTransportConfig, RetryConfig};

pub use fullnode::{GrpcTransport, GrpcTransportBuilder};
pub use middleware::{GrpcCall, GrpcMiddleware, GrpcOutcome};
pub use solidity::{SolidityGrpcTransport, SolidityGrpcTransportBuilder};
use tronz_rpc_types::{codec, light_block};

use crate::error::{RpcStatusCode, TransportErrorKind};

/// Marks an HTTP status embedded in tonic's decode error.
const HTTP_STATUS_MARKER: &str = "while receiving response with status: ";

pub(super) fn map_status(status: tonic::Status) -> TransportErrorKind {
    if let Some(http) = http_status(status.message())
        && let Some(code) = map_http_status(http)
    {
        return TransportErrorKind::Rpc {
            code,
            message: format!("endpoint answered HTTP {http} instead of a gRPC response"),
        };
    }

    TransportErrorKind::Rpc { code: map_code(status.code()), message: status.message().to_owned() }
}

fn http_status(message: &str) -> Option<u16> {
    let rest = message.split_once(HTTP_STATUS_MARKER)?.1;
    rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

fn map_http_status(status: u16) -> Option<RpcStatusCode> {
    match status {
        401 => Some(RpcStatusCode::Unauthenticated),
        403 => Some(RpcStatusCode::PermissionDenied),
        429 => Some(RpcStatusCode::ResourceExhausted),
        502..=504 => Some(RpcStatusCode::Unavailable),
        _ => None,
    }
}
pub(super) fn map_connect(err: tonic::transport::Error) -> TransportErrorKind {
    TransportErrorKind::connect(err)
}
fn map_code(code: tonic::Code) -> RpcStatusCode {
    use tonic::Code;

    match code {
        Code::Cancelled => RpcStatusCode::Cancelled,
        Code::InvalidArgument => RpcStatusCode::InvalidArgument,
        Code::DeadlineExceeded => RpcStatusCode::DeadlineExceeded,
        Code::NotFound => RpcStatusCode::NotFound,
        Code::AlreadyExists => RpcStatusCode::AlreadyExists,
        Code::PermissionDenied => RpcStatusCode::PermissionDenied,
        Code::ResourceExhausted => RpcStatusCode::ResourceExhausted,
        Code::FailedPrecondition => RpcStatusCode::FailedPrecondition,
        Code::Aborted => RpcStatusCode::Aborted,
        Code::OutOfRange => RpcStatusCode::OutOfRange,
        Code::Unimplemented => RpcStatusCode::Unimplemented,
        Code::Internal => RpcStatusCode::Internal,
        Code::Unavailable => RpcStatusCode::Unavailable,
        Code::DataLoss => RpcStatusCode::DataLoss,
        Code::Unauthenticated => RpcStatusCode::Unauthenticated,
        Code::Ok | Code::Unknown => RpcStatusCode::Unknown,
    }
}
/// TronGrid mainnet gRPC endpoint.
pub const TRONGRID_MAINNET: &str = "https://grpc.trongrid.io:443";
/// TronGrid mainnet SolidityNode endpoint.
pub const TRONGRID_MAINNET_SOLIDITY: &str = "http://grpc.trongrid.io:50052";
/// TronGrid Nile FullNode endpoint.
pub const TRONGRID_NILE: &str = "http://grpc.nile.trongrid.io:50051";
/// TronGrid Nile SolidityNode endpoint.
pub const TRONGRID_NILE_SOLIDITY: &str = "http://grpc.nile.trongrid.io:50061";

#[cfg(test)]
mod tests {
    use super::*;

    const RATE_LIMITED: &str = "protocol error: received message with invalid compression flag: \
        114 (valid flags are 0 and 1) while receiving response with status: 429 Too Many Requests";

    #[test]
    fn a_rate_limit_behind_a_decode_failure_is_reported_as_one() {
        let error = map_status(tonic::Status::internal(RATE_LIMITED));

        assert_eq!(error.status_code(), Some(RpcStatusCode::ResourceExhausted));
        assert_eq!(
            error.to_string(),
            "rpc status resource exhausted: endpoint answered HTTP 429 instead of a gRPC response"
        );
        assert!(error.is_retryable());
    }

    #[test]
    fn a_rejected_api_key_is_told_apart_from_a_missing_one() {
        for (http, expected) in
            [(401, RpcStatusCode::Unauthenticated), (403, RpcStatusCode::PermissionDenied)]
        {
            let message = format!("{HTTP_STATUS_MARKER}{http} Nope");

            let error = map_status(tonic::Status::internal(message));

            assert_eq!(error.status_code(), Some(expected));
            assert!(!error.is_retryable(), "a credential problem does not fix itself");
        }
    }

    #[test]
    fn a_gateway_failure_is_worth_retrying() {
        for http in [502, 503, 504] {
            let message = format!("{HTTP_STATUS_MARKER}{http} Bad Gateway");

            assert!(map_status(tonic::Status::internal(message)).is_retryable());
        }
    }

    #[test]
    fn an_untranslated_status_keeps_tonics_own_report() {
        let message = format!("{HTTP_STATUS_MARKER}418 I am a teapot");

        let error = map_status(tonic::Status::internal(message.clone()));

        assert_eq!(error.status_code(), Some(RpcStatusCode::Internal));
        assert_eq!(error.to_string(), format!("rpc status internal: {message}"));
    }

    #[test]
    fn an_ordinary_node_status_is_untouched() {
        let error = map_status(tonic::Status::not_found("no such block"));

        assert_eq!(error.status_code(), Some(RpcStatusCode::NotFound));
        assert_eq!(error.to_string(), "rpc status not found: no such block");
    }

    #[test]
    fn text_without_an_http_status_is_not_mistaken_for_one() {
        assert_eq!(http_status("plain old failure"), None);
        assert_eq!(http_status(&format!("{HTTP_STATUS_MARKER}not-a-number")), None);
        assert_eq!(http_status(RATE_LIMITED), Some(429));
    }
}
