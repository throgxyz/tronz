#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod address;
mod amount;
mod error;
mod log;
mod resource;
mod signature;

pub use address::{ADDRESS_LEN, ADDRESS_PREFIX, Address, EVM_ADDRESS_LEN};
/// Types re-used directly from `alloy-primitives`.
pub use alloy_primitives::{B256, Bytes, U256, keccak256};
pub use amount::{SUN_PER_TRX, Trx, format_trx, parse_trx};
pub use error::{AddressError, AmountError, SignatureError};
pub use log::Log;
pub use resource::ResourceCode;
pub use signature::{RecoverableSignature, SIGNATURE_LEN};

/// A transaction id: `sha256` of the protobuf-encoded raw transaction.
///
/// Defined here so it can appear in both signer and provider signatures
/// without a dependency cycle.
pub type TxId = B256;
