//! The [`TronSigner`] and [`TronSignerSync`] traits.

#[cfg(feature = "eip712")]
use alloy_dyn_abi::TypedData;
#[cfg(feature = "eip712")]
use alloy_sol_types::{Eip712Domain, SolStruct};
use async_trait::async_trait;
use auto_impl::auto_impl;
use tronz_primitives::{Address, B256, RecoverableSignature};

use crate::error::SignerError;

/// A signer capable of producing recoverable secp256k1 signatures over a
/// transaction hash.
///
/// The trait is object-safe, allowing heterogeneous signer backends to be
/// selected at runtime. Ownership and sharing are handled by
/// [`TronWallet`](crate::TronWallet).
///
/// Signers that do not need to await anything should implement
/// [`TronSignerSync`] as well and delegate to it.
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[auto_impl(&mut, Box)]
pub trait TronSigner {
    /// The TRON address that corresponds to this signer's key.
    fn address(&self) -> Address;

    /// Sign a 32-byte transaction hash (`tx_id`), returning a recoverable
    /// signature in TRON's `r || s || v` form.
    async fn sign_hash(&self, hash: &B256) -> Result<RecoverableSignature, SignerError>;

    /// Sign a plaintext message, TronWeb `signMessageV2`-compatible.
    ///
    /// The returned `v` is `0`/`1`; use `to_legacy_bytes` for TronWeb's `27`/`28`.
    ///
    /// ```
    /// # use tronz_signer::{LocalSigner, TronSigner};
    /// # use tronz_primitives::verify_message;
    /// # async fn example(signer: LocalSigner) -> Result<(), Box<dyn std::error::Error>> {
    /// let sig = signer.sign_message(b"hello world").await?;
    /// assert!(verify_message(b"hello world", &sig, signer.address()));
    /// let _tronweb_sig = sig.to_legacy_bytes();
    /// # Ok(())
    /// # }
    /// ```
    async fn sign_message(&self, message: &[u8]) -> Result<RecoverableSignature, SignerError> {
        self.sign_hash(&tronz_primitives::hash_message(message)).await
    }

    /// Encode and sign structured data per [TIP-712], TronWeb
    /// `signTypedData`-compatible.
    ///
    /// TIP-712 reuses EIP-712 encoding, so `address` members are the 20-byte EVM
    /// form of a TRON address; convert with `alloy_primitives::Address::from`.
    ///
    /// Requires the `eip712` feature.
    ///
    /// [TIP-712]: https://github.com/tronprotocol/tips/blob/master/tip-712.md
    #[cfg(feature = "eip712")]
    #[inline]
    #[auto_impl(keep_default_for(&mut, Box))]
    async fn sign_typed_data<T: SolStruct + Send + Sync>(
        &self,
        payload: &T,
        domain: &Eip712Domain,
    ) -> Result<RecoverableSignature, SignerError>
    where
        Self: Sized,
    {
        self.sign_hash(&payload.eip712_signing_hash(domain)).await
    }

    /// Encode and sign dynamically-typed structured data per [TIP-712].
    ///
    /// Unlike [`sign_typed_data`](Self::sign_typed_data), this works through
    /// trait objects such as `Box<dyn TronSigner>`.
    ///
    /// Requires the `eip712` feature.
    ///
    /// [TIP-712]: https://github.com/tronprotocol/tips/blob/master/tip-712.md
    #[cfg(feature = "eip712")]
    #[inline]
    async fn sign_dynamic_typed_data(
        &self,
        payload: &TypedData,
    ) -> Result<RecoverableSignature, SignerError> {
        self.sign_hash(&payload.eip712_signing_hash()?).await
    }
}

/// A signer that can sign without awaiting, such as an in-memory key.
///
/// Implementors should also implement [`TronSigner`] by delegating the async
/// methods to the synchronous ones, so the signer works in both worlds.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait TronSignerSync {
    /// Sign a 32-byte transaction hash (`tx_id`), returning a recoverable
    /// signature in TRON's `r || s || v` form.
    fn sign_hash_sync(&self, hash: &B256) -> Result<RecoverableSignature, SignerError>;

    /// Sign a plaintext message, TronWeb `signMessageV2`-compatible.
    ///
    /// The returned `v` is `0`/`1`; use `to_legacy_bytes` for TronWeb's `27`/`28`.
    #[inline]
    fn sign_message_sync(&self, message: &[u8]) -> Result<RecoverableSignature, SignerError> {
        self.sign_hash_sync(&tronz_primitives::hash_message(message))
    }

    /// Encode and sign structured data per [TIP-712], TronWeb
    /// `signTypedData`-compatible.
    ///
    /// Requires the `eip712` feature.
    ///
    /// [TIP-712]: https://github.com/tronprotocol/tips/blob/master/tip-712.md
    #[cfg(feature = "eip712")]
    #[inline]
    #[auto_impl(keep_default_for(&, &mut, Box, Rc, Arc))]
    fn sign_typed_data_sync<T: SolStruct>(
        &self,
        payload: &T,
        domain: &Eip712Domain,
    ) -> Result<RecoverableSignature, SignerError>
    where
        Self: Sized,
    {
        self.sign_hash_sync(&payload.eip712_signing_hash(domain))
    }

    /// Encode and sign dynamically-typed structured data per [TIP-712].
    ///
    /// Unlike [`sign_typed_data_sync`](Self::sign_typed_data_sync), this works
    /// through trait objects such as `Box<dyn TronSignerSync>`.
    ///
    /// Requires the `eip712` feature.
    ///
    /// [TIP-712]: https://github.com/tronprotocol/tips/blob/master/tip-712.md
    #[cfg(feature = "eip712")]
    #[inline]
    fn sign_dynamic_typed_data_sync(
        &self,
        payload: &TypedData,
    ) -> Result<RecoverableSignature, SignerError> {
        self.sign_hash_sync(&payload.eip712_signing_hash()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalSigner;

    const KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    #[tokio::test]
    async fn supports_boxed_trait_objects() {
        let boxed: Box<dyn TronSigner + Send + Sync> =
            Box::new(LocalSigner::from_hex(KEY).unwrap());
        let boxed_address = boxed.address();
        let signature = boxed.sign_message(b"boxed").await.unwrap();
        assert!(tronz_primitives::verify_message(b"boxed", &signature, boxed_address));
    }
}

#[cfg(all(test, feature = "eip712"))]
mod eip712_tests {
    use alloy_primitives::{address, b256};
    use alloy_sol_types::{eip712_domain, sol};

    use super::*;
    use crate::LocalSigner;

    const KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    sol! {
        struct Person {
            string name;
            address wallet;
        }

        struct Mail {
            Person from;
            Person to;
            string contents;
        }
    }

    /// Signing hash of the worked example in the EIP-712 specification. TIP-712
    /// reuses EIP-712 encoding, so ours must match it byte for byte.
    const REFERENCE_HASH: B256 =
        b256!("be609aee343fb3c4b28e1df9e632fca64fcfaede20f02e86244efddf30957bd2");

    fn reference_payload() -> (Mail, Eip712Domain) {
        let mail = Mail {
            from: Person {
                name: "Cow".into(),
                wallet: address!("CD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826"),
            },
            to: Person {
                name: "Bob".into(),
                wallet: address!("bBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB"),
            },
            contents: "Hello, Bob!".into(),
        };
        let domain = eip712_domain! {
            name: "Ether Mail",
            version: "1",
            chain_id: 1,
            verifying_contract: address!("CcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"),
        };
        (mail, domain)
    }

    #[tokio::test]
    async fn signs_the_eip712_reference_hash() {
        let (mail, domain) = reference_payload();
        assert_eq!(mail.eip712_signing_hash(&domain), REFERENCE_HASH);

        let signer = LocalSigner::from_hex(KEY).unwrap();
        let signature = signer.sign_typed_data(&mail, &domain).await.unwrap();

        assert_eq!(signature, signer.sign_hash(&REFERENCE_HASH).await.unwrap());
        assert_eq!(
            signature.recover_address_from_prehash(REFERENCE_HASH).unwrap(),
            signer.address()
        );
    }

    #[test]
    fn sync_signing_agrees_with_async() {
        let (mail, domain) = reference_payload();
        let signer = LocalSigner::from_hex(KEY).unwrap();

        assert_eq!(
            signer.sign_typed_data_sync(&mail, &domain).unwrap(),
            signer.sign_hash_sync(&REFERENCE_HASH).unwrap()
        );
    }

    /// The same payload in the JSON shape a TronWeb `signTypedData` caller sends.
    const REFERENCE_JSON: &str = r#"{
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" }
            ],
            "Person": [
                { "name": "name", "type": "string" },
                { "name": "wallet", "type": "address" }
            ],
            "Mail": [
                { "name": "from", "type": "Person" },
                { "name": "to", "type": "Person" },
                { "name": "contents", "type": "string" }
            ]
        },
        "primaryType": "Mail",
        "domain": {
            "name": "Ether Mail",
            "version": "1",
            "chainId": 1,
            "verifyingContract": "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"
        },
        "message": {
            "from": { "name": "Cow", "wallet": "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826" },
            "to": { "name": "Bob", "wallet": "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB" },
            "contents": "Hello, Bob!"
        }
    }"#;

    #[tokio::test]
    async fn dynamic_typed_data_matches_the_static_form() {
        let (mail, domain) = reference_payload();
        let dynamic: TypedData = serde_json::from_str(REFERENCE_JSON).unwrap();
        assert_eq!(dynamic.eip712_signing_hash().unwrap(), REFERENCE_HASH);

        let signer = LocalSigner::from_hex(KEY).unwrap();
        assert_eq!(
            signer.sign_dynamic_typed_data(&dynamic).await.unwrap(),
            signer.sign_typed_data(&mail, &domain).await.unwrap()
        );
    }

    #[tokio::test]
    async fn dynamic_typed_data_works_through_a_trait_object() {
        let dynamic: TypedData = serde_json::from_str(REFERENCE_JSON).unwrap();
        let boxed: Box<dyn TronSigner + Send + Sync> =
            Box::new(LocalSigner::from_hex(KEY).unwrap());

        let signature = boxed.sign_dynamic_typed_data(&dynamic).await.unwrap();
        assert_eq!(
            signature.recover_address_from_prehash(REFERENCE_HASH).unwrap(),
            boxed.address()
        );
    }

    #[test]
    fn dynamic_typed_data_works_through_a_sync_trait_object() {
        let dynamic: TypedData = serde_json::from_str(REFERENCE_JSON).unwrap();
        let boxed: Box<dyn TronSignerSync + Send + Sync> =
            Box::new(LocalSigner::from_hex(KEY).unwrap());

        let signature = boxed.sign_dynamic_typed_data_sync(&dynamic).unwrap();
        assert_eq!(
            signature.recover_address_from_prehash(REFERENCE_HASH).unwrap(),
            LocalSigner::from_hex(KEY).unwrap().address()
        );
    }
}
