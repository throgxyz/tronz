#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
extern crate tracing;

pub mod builders;
pub mod ext;
pub mod fillers;
pub mod layers;
pub mod transport;
mod type_aliases;

/// TRON's domain model, defined by [`tronz-rpc-types`] and re-exported here so
/// that a provider is the only dependency needed to use it.
///
/// [`tronz-rpc-types`]: tronz_rpc_types
pub mod types {
    pub use tronz_rpc_types::{ResponseError, types::*};
}

mod error;
pub use error::{
    ProviderError, Result, RpcError, RpcStatusCode, TransportErrorKind, TransportResult,
};
/// Backward-compatible alias — prefer [`ProviderError`] in new code.
pub type Error = ProviderError;

mod provider;
pub use ext::{GovernanceApi, Trc10Api, WitnessApi};
pub use fillers::{HasSigner, WalletFiller};
pub use layers::{ProviderLayer, Stack};
pub use provider::{
    ContractReadProvider, DynProvider, FilledProvider, PendingTransaction, PendingTransactionError,
    ProviderBuilder, RootProvider, SolidityProvider, SolidityProviderBuilder, TronProvider,
};
pub use transport::{DynSolidityTransport, DynTransport, SolidityTransport, TronTransport};
pub use type_aliases::*;
pub use types::{
    AccountNet, ChainProperties, NodeAddress, NodeInfo, ProposalInfo, ProposalState, SignWeight,
};

pub(crate) mod proto;
