//! tonic-backed gRPC transports for TRON FullNode and SolidityNode services.

mod abi;
#[cfg(test)]
mod capture;
mod codec;
mod core;
mod fullnode;
mod light_block;
mod solidity;

use core::GrpcCore;
pub use core::{GrpcTransportConfig, RetryConfig};

pub use fullnode::{GrpcTransport, GrpcTransportBuilder};
pub use solidity::{SolidityGrpcTransport, SolidityGrpcTransportBuilder};

/// TronGrid mainnet gRPC endpoint (TLS).
pub const TRONGRID_MAINNET: &str = "https://grpc.trongrid.io:443";
/// TronGrid mainnet SolidityNode gRPC endpoint (plain HTTP/2).
pub const TRONGRID_MAINNET_SOLIDITY: &str = "http://grpc.trongrid.io:50052";
/// TronGrid Nile testnet FullNode gRPC endpoint (plain HTTP/2).
pub const TRONGRID_NILE: &str = "http://grpc.nile.trongrid.io:50051";
/// TronGrid Nile testnet SolidityNode gRPC endpoint (plain HTTP/2).
pub const TRONGRID_NILE_SOLIDITY: &str = "http://grpc.nile.trongrid.io:50061";
