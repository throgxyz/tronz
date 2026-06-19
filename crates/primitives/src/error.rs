//! Error types for `tronz-primitives`.

/// Errors produced when parsing or validating a TRON [`Address`](crate::Address).
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum AddressError {
    /// The raw address did not start with the mainnet prefix byte `0x41`.
    #[error("invalid prefix byte: expected 0x41, got 0x{0:02x}")]
    BadPrefix(u8),

    /// The decoded byte slice was not the expected length.
    #[error("bad length: expected {expected} bytes, got {got}")]
    BadLength {
        /// Number of bytes that were expected.
        expected: usize,
        /// Number of bytes that were actually provided.
        got: usize,
    },

    /// base58check decoding failed (bad checksum or invalid characters).
    #[error("base58 decode failed: {0}")]
    Base58(#[from] bs58::decode::Error),

    /// hex decoding failed.
    #[error("hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),
}

/// Errors produced when constructing a [`Trx`](crate::Trx) amount.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum AmountError {
    /// A negative value was supplied where only non-negative amounts are valid.
    #[error("negative amount is invalid: {0} sun")]
    Negative(i64),

    /// Converting a floating-point TRX value overflowed the `i64` sun range.
    #[error("amount out of range: {0} TRX cannot be represented in sun")]
    OutOfRange(f64),
}

/// Errors produced when constructing a [`RecoverableSignature`](crate::RecoverableSignature).
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum SignatureError {
    /// The signature byte slice was not exactly 65 bytes (`r || s || v`).
    #[error("bad signature length: expected 65 bytes, got {0}")]
    BadLength(usize),

    /// The recovery id byte was not `0` or `1` (after normalising `27`/`28`).
    #[error("invalid recovery id: {0}")]
    BadRecoveryId(u8),

    /// The underlying ECDSA library rejected the signature scalars.
    #[error("ecdsa error: {0}")]
    Ecdsa(#[from] k256::ecdsa::Error),
}
