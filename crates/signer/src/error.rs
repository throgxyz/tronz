//! Error types for `tronz-signer`.

use std::fmt;

/// Errors produced while creating a signer or signing a payload.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum SignerError {
    /// The underlying ECDSA operation failed.
    #[error("signing failed: {0}")]
    Ecdsa(#[from] k256::ecdsa::Error),

    /// A private key could not be decoded from the supplied bytes/hex.
    #[error("invalid private key: {0}")]
    InvalidKey(String),

    /// Hex decoding of a private key failed.
    #[error("hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),

    /// The signer has no associated address (e.g. [`NoSigner`](crate::NoSigner)).
    #[error("signer has no address")]
    NoAddress,

    /// I/O error (e.g. writing a mnemonic phrase to disk or a keystore file).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Keystore-specific error (wrong password, unsupported algorithm, etc.).
    #[cfg(feature = "keystore")]
    #[error(transparent)]
    Keystore(#[from] crate::keystore::KeystoreError),

    /// JSON serialization/deserialization error.
    #[cfg(feature = "keystore")]
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// BIP-32 HD key derivation error.
    #[cfg(feature = "mnemonic")]
    #[error("BIP-32 error: {0}")]
    Bip32(#[from] coins_bip32::Bip32Error),

    /// BIP-39 mnemonic error.
    #[cfg(feature = "mnemonic")]
    #[error("BIP-39 error: {0}")]
    Bip39(#[from] coins_bip39::MnemonicError),

    /// [`MnemonicBuilder`](crate::mnemonic::MnemonicBuilder) misuse.
    #[cfg(feature = "mnemonic")]
    #[error("{0}")]
    MnemonicBuilder(#[from] crate::mnemonic::MnemonicBuilderError),

    /// Escape hatch for errors from custom signer implementations.
    ///
    /// Analogous to `alloy_signer::Error::Other`.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl SignerError {
    /// Wrap an arbitrary error as [`Other`](Self::Other).
    ///
    /// Analogous to `alloy_signer::Error::other`.
    #[cold]
    pub fn other(err: impl Into<Box<dyn std::error::Error + Send + Sync + 'static>>) -> Self {
        Self::Other(err.into())
    }

    /// Construct an [`Other`](Self::Other) error from a display message.
    ///
    /// Analogous to `alloy_signer::Error::message`.
    #[cold]
    pub fn message(msg: impl fmt::Display) -> Self {
        Self::Other(msg.to_string().into())
    }
}
