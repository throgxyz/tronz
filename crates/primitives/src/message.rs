//! TronWeb `signMessageV2`-compatible message hashing and verification.
//!
//! Digest = `keccak256("\x19TRON Signed Message:\n" || ascii(byte_len) || message)`
//! (the TRON-prefixed format documented as TIP-191-compatible).

use alloy_primitives::keccak256;

use crate::{Address, B256, RecoverableSignature, error::SignatureError};

/// The TRON personal-message prefix used by TronWeb `signMessageV2`.
pub const TRON_MESSAGE_PREFIX: &[u8] = b"\x19TRON Signed Message:\n";

/// Hash a message the way TronWeb `signMessageV2` does.
///
/// Strings are hashed as UTF-8 bytes: `"0x1234"` is 6 plaintext bytes, not hex.
pub fn hash_message(message: impl AsRef<[u8]>) -> B256 {
    let message = message.as_ref();
    let len = message.len().to_string();

    let mut prefixed = Vec::with_capacity(TRON_MESSAGE_PREFIX.len() + len.len() + message.len());
    prefixed.extend_from_slice(TRON_MESSAGE_PREFIX);
    prefixed.extend_from_slice(len.as_bytes());
    prefixed.extend_from_slice(message);

    keccak256(prefixed)
}

/// Recover the TRON address that signed `message` (TronWeb `verifyMessageV2`).
///
/// Accepts a `v` byte of `0`/`1` or the legacy `27`/`28`.
pub fn recover_message_address(
    message: impl AsRef<[u8]>,
    signature: &RecoverableSignature,
) -> Result<Address, SignatureError> {
    signature.recover_address_from_prehash(hash_message(message))
}

/// Verify that `signature` over `message` was produced by `address`.
///
/// Returns `false` on any recovery error; use [`recover_message_address`] to
/// inspect the error.
pub fn verify_message(
    message: impl AsRef<[u8]>,
    signature: &RecoverableSignature,
    address: Address,
) -> bool {
    recover_message_address(message, signature)
        .map(|recovered| recovered == address)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Golden vectors from TronWeb `Trx.signMessageV2`, private key 0x00..01.
    const TRONWEB_ADDR: &str = "TMVQGm1qAQYVdetCeGRRkTWYYrLXuHK2HC";
    const VECTORS: &[(&str, &str)] = &[
        (
            "hello world",
            "0x0dc0b53d525e0103a6013061cf18e60cf158809149f2b8994a545af65a7004cb1eeaff560e801ab51b28df5d42549aa024c2aa7e9d34de1e01294b9afb5e6c7e1c",
        ),
        (
            "",
            "0x5fc8883facaeb9dbebe71ec179c2744f83dc0e4868c6c9a6d63d47b949f33ae4317dc553f28a9c64b06838dc1adf7f24da330394f05e4ea6318d18608293ffe91b",
        ),
        (
            "你好",
            "0xa1a3ca757f1a6e52018bff5de55903be6073072f3e1fc82bea75f9f9e491bab4516d9a02f448f13127175fd2e91df6a9ea7da6422fc3c8f89353b64beb24c6321b",
        ),
        (
            "0x1234",
            "0x76f9b3883c5fc5415a5f132fe6916d089441c27c2438bee9c8461409a2002c950d1882ed3b3468cfe8d77b303241446179e60971287fdd882ea566ef7e517a101c",
        ),
    ];

    fn sig_from_hex(s: &str) -> RecoverableSignature {
        let bytes = hex::decode(s.trim_start_matches("0x")).unwrap();
        RecoverableSignature::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn hash_message_matches_manual_assembly() {
        for (msg, _) in VECTORS {
            let mut expected = Vec::new();
            expected.extend_from_slice(b"\x19TRON Signed Message:\n");
            expected.extend_from_slice(msg.len().to_string().as_bytes());
            expected.extend_from_slice(msg.as_bytes());
            assert_eq!(hash_message(msg), keccak256(&expected), "msg={msg:?}");
        }
    }

    #[test]
    fn byte_length_prefix_is_utf8_not_char_count() {
        assert_eq!("你好".len(), 6);
        assert_eq!("0x1234".len(), 6);
    }

    #[test]
    fn recovers_tronweb_signatures() {
        let expected = Address::from_base58(TRONWEB_ADDR).unwrap();
        for (msg, sig_hex) in VECTORS {
            let sig = sig_from_hex(sig_hex);
            assert_eq!(recover_message_address(msg, &sig).unwrap(), expected, "msg={msg:?}");
            assert!(verify_message(msg, &sig, expected), "msg={msg:?}");
        }
    }

    #[test]
    fn verify_rejects_tampered_message_and_wrong_address() {
        let expected = Address::from_base58(TRONWEB_ADDR).unwrap();
        let sig = sig_from_hex(VECTORS[0].1); // "hello world"
        assert!(!verify_message("hello worlx", &sig, expected));
        assert!(!verify_message("hello world", &sig, Address::ZERO));
    }
}
