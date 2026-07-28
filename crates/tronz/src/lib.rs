#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// Compile workspace README examples only when their required features are enabled.
#![cfg_attr(
    all(doctest, feature = "signer-mnemonic", feature = "signer-keystore"),
    doc = include_str!("../../../README.md")
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[doc(no_inline)]
pub use primitives::{
    Address, RecoverableSignature, ResourceCode, Trx, U256, format_trx, hash_message, parse_trx,
    recover_message_address, verify_message,
};
/// Core TRON primitives: addresses, amounts, resource codes, signatures.
#[doc(inline)]
pub use tronz_primitives as primitives;

/// Native TRON smart-contract ABI metadata types.
pub mod abi {
    #[doc(inline)]
    pub use tronz_abi::*;
}

#[doc(no_inline)]
pub use tronz_abi::{
    TronAbi, TronAbiEntry, TronAbiEntryType, TronAbiParam, TronAbiStateMutability,
};

/// TRON signers, wallets, and local key implementations.
pub mod signers {
    #[doc(inline)]
    pub use tronz_signer::*;
    /// AWS KMS signer — keeps the private key inside the AWS HSM.
    #[cfg(feature = "signer-aws")]
    #[doc(inline)]
    pub use tronz_signer_aws as aws;
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
pub use tronz_signer::{LocalSigner, TronNetworkWallet, TronSigner, TronSignerSync, TronWallet};
#[cfg(feature = "signer-aws")]
#[doc(no_inline)]
pub use tronz_signer_aws::AwsSigner;

/// Interface with a TRON node.
pub mod providers {
    #[doc(inline)]
    pub use tronz_provider::*;
}

#[doc(no_inline)]
pub use tronz_provider::{
    ContractReadProvider, ProviderBuilder, SolidityProvider, SolidityProviderBuilder, TronProvider,
};

/// Low-level gRPC transport and well-known endpoint constants.
pub mod transports {
    #[doc(inline)]
    pub use tronz_provider::transport::*;
}

#[doc(no_inline)]
pub use tronz_provider::transport::grpc::{
    TRONGRID_MAINNET, TRONGRID_MAINNET_SOLIDITY, TRONGRID_NILE, TRONGRID_NILE_SOLIDITY,
};

/// TRC20 / TRC721 contract bindings and provider-bound instances.
#[cfg(feature = "contract")]
pub mod contract {
    #[doc(inline)]
    pub use tronz_contract::*;
}

#[cfg(feature = "contract")]
#[doc(no_inline)]
pub use tronz_contract::{JsonAbi, tron_sol};
