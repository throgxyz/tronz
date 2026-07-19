#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

/* --------------------------------- Primitives --------------------------------- */

#[doc(no_inline)]
pub use primitives::{
    Address, RecoverableSignature, ResourceCode, Trx, U256, format_trx, hash_message, parse_trx,
    recover_message_address, verify_message,
};
/// Core TRON primitives: addresses, amounts, resource codes, signatures.
#[doc(inline)]
pub use tronz_primitives as primitives;

/* ------------------------------------ ABI ------------------------------------- */

/// Native TRON smart-contract ABI metadata types.
pub mod abi {
    #[doc(inline)]
    pub use tronz_abi::*;
}

#[doc(no_inline)]
pub use tronz_abi::{
    TronAbi, TronAbiEntry, TronAbiEntryType, TronAbiParam, TronAbiStateMutability,
};

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
/// AWS KMS signer — keeps the private key inside the AWS HSM.
///
/// See [`tronz_signer_aws`] for more details.
#[cfg(feature = "signer-aws")]
#[doc(inline)]
pub use tronz_signer_aws as signer_aws;
#[cfg(feature = "signer-aws")]
#[doc(no_inline)]
pub use tronz_signer_aws::AwsSigner;

/* --------------------------------- Providers ---------------------------------- */

/// Interface with a TRON node.
///
/// See [`tronz_provider`] for more details.
pub mod providers {
    #[doc(inline)]
    pub use tronz_provider::*;
}

#[doc(no_inline)]
pub use tronz_provider::{
    ContractReadProvider, ProviderBuilder, SolidityProvider, SolidityProviderBuilder, TronProvider,
};

/// Low-level gRPC transport and well-known endpoint constants.
///
/// You will likely not need to use this module directly;
/// see the [`providers`] module for high-level provider usage.
pub mod transports {
    #[doc(inline)]
    pub use tronz_provider::transport::*;
}

#[doc(no_inline)]
pub use tronz_provider::transport::grpc::{
    TRONGRID_MAINNET, TRONGRID_MAINNET_SOLIDITY, TRONGRID_NILE, TRONGRID_NILE_SOLIDITY,
};

/* --------------------------------- Contracts ---------------------------------- */

/// TRC20 / TRC721 contract bindings and provider-bound instances.
#[cfg(feature = "contract")]
pub mod contract {
    #[doc(inline)]
    pub use tronz_contract::*;
}

#[cfg(feature = "contract")]
#[doc(no_inline)]
pub use tronz_contract::JsonAbi;
