#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod abi;
mod item;
mod param;

#[cfg(feature = "alloy")]
mod alloy;
#[cfg(feature = "alloy")]
mod error;

pub use abi::TronAbi;
#[cfg(feature = "alloy")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloy")))]
#[doc(no_inline)]
pub use alloy_json_abi as json_abi;
#[cfg(feature = "alloy")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloy")))]
#[doc(no_inline)]
pub use alloy_json_abi::JsonAbi;
#[cfg(feature = "alloy")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloy")))]
pub use error::TronAbiConversionError;
pub use item::{TronAbiEntry, TronAbiEntryType, TronAbiStateMutability};
pub use param::TronAbiParam;
