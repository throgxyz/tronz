//! TRON address type.
//!
//! A TRON address is 21 bytes: a single `0x41` prefix byte followed by the
//! 20-byte EVM-style address (`keccak256(pubkey)[12..]`). It is most commonly
//! displayed in base58check form (the familiar `T...` string).

use core::{fmt, str::FromStr};

use alloy_primitives::keccak256;
use k256::ecdsa::VerifyingKey;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::AddressError;

/// The TRON mainnet address prefix byte.
pub const ADDRESS_PREFIX: u8 = 0x41;

/// Length of a raw TRON address in bytes (prefix + 20-byte body).
pub const ADDRESS_LEN: usize = 21;

/// Length of the EVM-style address body (without the `0x41` prefix).
pub const EVM_ADDRESS_LEN: usize = 20;

/// A TRON network address (`0x41` prefix + 20-byte body).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address([u8; ADDRESS_LEN]);

impl Address {
    /// The zero address (`0x41` prefix followed by 20 zero bytes).
    pub const ZERO: Self = {
        let mut bytes = [0u8; ADDRESS_LEN];
        bytes[0] = ADDRESS_PREFIX;
        Self(bytes)
    };

    /// Construct from the full 21-byte representation, validating the prefix.
    pub fn from_bytes(bytes: [u8; ADDRESS_LEN]) -> Result<Self, AddressError> {
        if bytes[0] != ADDRESS_PREFIX {
            return Err(AddressError::BadPrefix(bytes[0]));
        }
        Ok(Self(bytes))
    }

    /// Construct from a 21-byte slice, validating length and prefix.
    pub fn from_slice(slice: &[u8]) -> Result<Self, AddressError> {
        let bytes: [u8; ADDRESS_LEN] = slice
            .try_into()
            .map_err(|_| AddressError::BadLength { expected: ADDRESS_LEN, got: slice.len() })?;
        Self::from_bytes(bytes)
    }

    /// Construct from the 20-byte EVM-style body, prepending the `0x41` prefix.
    pub fn from_evm_bytes(evm: [u8; EVM_ADDRESS_LEN]) -> Self {
        let mut bytes = [0u8; ADDRESS_LEN];
        bytes[0] = ADDRESS_PREFIX;
        bytes[1..].copy_from_slice(&evm);
        Self(bytes)
    }

    /// Derive the address from a secp256k1 public key.
    ///
    /// `address = 0x41 || keccak256(uncompressed_pubkey[1..])[12..]`
    pub fn from_public_key(key: &VerifyingKey) -> Self {
        let point = key.to_encoded_point(false);
        // Uncompressed SEC1 encoding is `0x04 || X(32) || Y(32)`; hash the 64
        // coordinate bytes, skipping the `0x04` tag.
        let hash = keccak256(&point.as_bytes()[1..]);
        let mut evm = [0u8; EVM_ADDRESS_LEN];
        evm.copy_from_slice(&hash[12..]);
        Self::from_evm_bytes(evm)
    }

    /// Parse a base58check (`T...`) address string.
    pub fn from_base58(s: &str) -> Result<Self, AddressError> {
        let decoded = bs58::decode(s).with_check(None).into_vec()?;
        Self::from_slice(&decoded)
    }

    /// Parse a hex address string (with or without `0x` / `41` semantics is
    /// preserved: the bytes must already include the `0x41` prefix).
    pub fn from_hex(s: &str) -> Result<Self, AddressError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s)?;
        Self::from_slice(&bytes)
    }

    /// The full 21-byte representation, including the `0x41` prefix.
    pub fn as_bytes(&self) -> &[u8; ADDRESS_LEN] {
        &self.0
    }

    /// The 20-byte EVM-style body (prefix stripped). Use this when bridging to
    /// `alloy` / ABI encoding.
    pub fn as_evm_bytes(&self) -> &[u8; EVM_ADDRESS_LEN] {
        self.0[1..].try_into().expect("address body is always 20 bytes")
    }

    /// Encode as a base58check (`T...`) string.
    pub fn to_base58(&self) -> String {
        bs58::encode(&self.0).with_check().into_string()
    }

    /// Encode as a lowercase hex string including the `0x41` prefix (no `0x`).
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_base58())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({})", self.to_base58())
    }
}

impl FromStr for Address {
    type Err = AddressError;

    /// Accepts either a base58check (`T...`) or a hex (`41...` / `0x41...`)
    /// address. Hex is detected when every character is a hex digit and the
    /// string is the right length for a 21-byte address.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hexish = s.strip_prefix("0x").unwrap_or(s);
        let looks_hex =
            hexish.len() == ADDRESS_LEN * 2 && hexish.bytes().all(|b| b.is_ascii_hexdigit());
        if looks_hex { Self::from_hex(s) } else { Self::from_base58(s) }
    }
}

// --- alloy bridging ---------------------------------------------------------

impl From<Address> for alloy_primitives::Address {
    fn from(a: Address) -> Self {
        alloy_primitives::Address::from(*a.as_evm_bytes())
    }
}

impl From<alloy_primitives::Address> for Address {
    /// Re-attaches the TRON mainnet `0x41` prefix to a 20-byte EVM address.
    fn from(a: alloy_primitives::Address) -> Self {
        Address::from_evm_bytes(a.into_array())
    }
}

// --- serde ------------------------------------------------------------------

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_base58())
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Well-known TRON address used widely in docs/tests.
    const B58: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";
    const HEX: &str = "41a614f803b6fd780986a42c78ec9c7f77e6ded13c";

    #[test]
    fn base58_roundtrip() {
        let a = Address::from_base58(B58).unwrap();
        assert_eq!(a.to_base58(), B58);
        assert_eq!(a.to_hex(), HEX);
    }

    #[test]
    fn hex_roundtrip() {
        let a = Address::from_hex(HEX).unwrap();
        assert_eq!(a.to_base58(), B58);
    }

    #[test]
    fn fromstr_detects_format() {
        assert_eq!(B58.parse::<Address>().unwrap().to_hex(), HEX);
        assert_eq!(HEX.parse::<Address>().unwrap().to_base58(), B58);
        let with_0x = format!("0x{HEX}");
        assert_eq!(with_0x.parse::<Address>().unwrap().to_base58(), B58);
    }

    #[test]
    fn bad_prefix_rejected() {
        let mut bytes = [0u8; ADDRESS_LEN];
        bytes[0] = 0x42;
        assert!(matches!(Address::from_bytes(bytes), Err(AddressError::BadPrefix(0x42))));
    }

    #[test]
    fn alloy_bridge_roundtrip() {
        let a = Address::from_base58(B58).unwrap();
        let evm: alloy_primitives::Address = a.into();
        assert_eq!(evm.as_slice(), a.as_evm_bytes());
        let back: Address = evm.into();
        assert_eq!(back, a);
    }

    #[test]
    fn evm_bytes_strip_prefix() {
        let a = Address::from_hex(HEX).unwrap();
        assert_eq!(a.as_evm_bytes().len(), 20);
        assert_eq!(&a.as_bytes()[1..], a.as_evm_bytes());
    }
}
