//! The protobuf surface the gRPC transport speaks.
//!
//! The messages come from [`tronz_rpc_types::proto`], which owns the schema; only
//! the service clients are generated here, against those messages. Neither
//! appears in a public signature — callers work with the domain types in
//! [`crate::types`], and the mapping onto them lives in `tronz-rpc-types`
//! alongside both sides of it.

pub(crate) use tronz_rpc_types::proto::*;

/// The generated gRPC clients.
///
/// `cargo xtask codegen` produces these from the same schema as the messages,
/// mapping the whole `protocol` package onto `tronz-rpc-types`, so a message has
/// exactly one definition in the workspace no matter which crate names it.
#[allow(dead_code, unused_imports, clippy::all, clippy::pedantic)]
mod services {
    include!("../generated/services.rs");
}

pub(crate) use services::{
    database_client, wallet_client, wallet_extension_client, wallet_solidity_client,
};
