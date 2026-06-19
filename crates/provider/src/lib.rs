#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod builders;
pub mod ext;
pub mod fillers;
pub mod transport;
pub mod types;

mod error;
pub use error::{ProviderError, Result, RpcError, TransportErrorKind, TransportResult};
/// Backward-compatible alias — prefer [`ProviderError`] in new code.
pub type Error = ProviderError;

mod provider;
pub use ext::{GovernanceApi, Trc10Api, WitnessApi};
pub use fillers::HasSigner;
pub use provider::{
    FilledProvider, PendingTransaction, PendingTransactionError, ProviderBuilder, RootProvider,
    TronProvider,
};
pub use transport::TronTransport;
pub use types::{
    AccountNet, ChainProperties, NodeAddress, NodeInfo, ProposalInfo, ProposalState, SignWeight,
};

// Private: prost-generated code + codec conversions never leak publicly.
pub(crate) mod proto;
