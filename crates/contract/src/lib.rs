#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(test)]
extern crate self as tronz_contract;

/// Static TRC20 ABI bindings and encode/decode helpers.
pub mod trc20;

/// Static TRC721 ABI bindings and encode/decode helpers.
pub mod trc721;

/// Event log decoding helpers for TRON smart contracts.
pub mod event;
/// The `sol!` macro (re-exported from alloy) for generating Solidity type bindings.
pub use alloy_sol_types::sol;
/// Re-exported alloy ABI types for use with generated calls.
pub use alloy_sol_types::{SolCall, SolError, SolEvent, SolInterface, SolValue};
#[cfg(feature = "provider")]
pub use event::{decode_log, decode_logs, log_matches, topic0_set};
pub use tronz_primitives::{Address, Bytes, U256};
/// The `tron_sol!` macro — generates provider-bound, type-safe contract bindings.
///
/// Requires the `provider` feature (enabled by default via the `tronz` meta-crate).
pub use tronz_sol_macro::tron_sol;

#[cfg(feature = "provider")]
mod error;
#[cfg(feature = "provider")]
pub use error::{ContractError, Result};

#[cfg(feature = "provider")]
mod interface;
#[cfg(feature = "provider")]
pub use alloy_dyn_abi::DecodedEvent;
#[cfg(feature = "provider")]
pub use alloy_json_abi::JsonAbi;
#[cfg(feature = "provider")]
pub use interface::Interface;
#[cfg(feature = "provider")]
pub use tronz_abi::{
    TronAbi, TronAbiConversionError, TronAbiEntry, TronAbiEntryType, TronAbiParam,
    TronAbiStateMutability,
};

#[cfg(feature = "provider")]
mod instance;
#[cfg(feature = "provider")]
pub use instance::{ContractExt, ContractInstance};

#[cfg(feature = "provider")]
mod call;
#[cfg(feature = "provider")]
pub use call::CallBuilder;

#[cfg(feature = "provider")]
mod sol_call;
#[cfg(feature = "provider")]
pub use sol_call::TronCallBuilder;

#[cfg(feature = "provider")]
mod event_filter;
#[cfg(feature = "provider")]
pub use event_filter::TronEventFilter;

/// Internal re-exports referenced by [`tron_sol!`]-generated code. Not a stable API.
#[cfg(feature = "provider")]
#[doc(hidden)]
pub mod __private {
    pub use alloy_primitives;
    pub use alloy_sol_types;
    pub use tronz_primitives;
    pub use tronz_provider;

    pub use crate::{
        deploy::DeployBuilder, error::Result, event_filter::TronEventFilter,
        instance::ContractInstance, sol_call::TronCallBuilder,
    };
}

#[cfg(feature = "provider")]
mod deploy;
#[cfg(feature = "provider")]
pub use deploy::DeployBuilder;
#[cfg(feature = "provider")]
pub use trc20::{Trc20Error, Trc20Ext, Trc20Instance};
#[cfg(feature = "provider")]
pub use trc721::{Trc721Error, Trc721Ext, Trc721Instance};
