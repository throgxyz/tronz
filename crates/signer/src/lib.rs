#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;
pub use error::SignerError;

mod signer;
pub use signer::{TronSigner, TronSignerSync};

mod wallet;
pub use wallet::{TronNetworkWallet, TronWallet};

mod local;
#[cfg(feature = "tip712")]
#[cfg_attr(docsrs, doc(cfg(feature = "tip712")))]
#[doc(no_inline)]
pub use alloy_dyn_abi::{self, TypedData};
#[cfg(feature = "tip712")]
#[cfg_attr(docsrs, doc(cfg(feature = "tip712")))]
#[doc(no_inline)]
pub use alloy_sol_types::{self, Eip712Domain, SolStruct};
pub use k256;
pub use local::LocalSigner;
pub use tronz_primitives::RecoverableSignature;

#[cfg(feature = "mnemonic")]
pub mod mnemonic;
#[cfg(feature = "mnemonic")]
pub use coins_bip39;
#[cfg(feature = "mnemonic")]
pub use mnemonic::MnemonicBuilder;

#[cfg(feature = "keystore")]
pub mod keystore;
#[cfg(feature = "keystore")]
pub use keystore::KeystoreFile;
