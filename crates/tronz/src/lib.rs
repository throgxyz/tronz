#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

/* --------------------------------- Primitives --------------------------------- */

#[doc(no_inline)]
pub use primitives::{Address, Trx, U256};
/// Core TRON primitives: addresses, amounts, resource codes, signatures.
#[doc(inline)]
pub use tronz_primitives as primitives;

/* ---------------------------------- Signers ----------------------------------- */

/// TRON signer abstraction and local key implementation.
///
/// See [`tronz_signer`] for more details.
pub mod signers {
    #[doc(inline)]
    pub use tronz_signer::*;
}

#[cfg(feature = "signer-keystore")]
#[doc(no_inline)]
pub use tronz_signer::KeystoreFile;
#[cfg(feature = "signer-mnemonic")]
#[doc(no_inline)]
pub use tronz_signer::MnemonicBuilder;
#[cfg(feature = "signer-mnemonic")]
#[doc(no_inline)]
pub use tronz_signer::coins_bip39;
#[doc(no_inline)]
pub use tronz_signer::{LocalSigner, TronSigner};

/* --------------------------------- Providers ---------------------------------- */

/// Interface with a TRON node.
///
/// See [`tronz_provider`] for more details.
pub mod providers {
    #[doc(inline)]
    pub use tronz_provider::*;
}

#[doc(no_inline)]
pub use tronz_provider::{ProviderBuilder, TronProvider};

/// Low-level gRPC transport and well-known endpoint constants.
///
/// You will likely not need to use this module directly;
/// see the [`providers`] module for high-level provider usage.
pub mod transports {
    #[doc(inline)]
    pub use tronz_provider::transport::*;
}

#[doc(no_inline)]
pub use tronz_provider::transport::grpc::{TRONGRID_MAINNET, TRONGRID_NILE};

/* --------------------------------- Contracts ---------------------------------- */

/// TRC20 contract bindings and provider-bound dynamic ABI helpers.
#[cfg(feature = "contract")]
pub mod contract {
    #[doc(inline)]
    pub use tronz_contract::*;
}
