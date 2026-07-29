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
pub(super) fn map_status(status: tonic::Status) -> TransportErrorKind {
    TransportErrorKind::Rpc { code: map_code(status.code()), message: status.message().to_owned() }
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
