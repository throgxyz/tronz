//! The [`TronNetworkWallet`] trait and the [`TronWallet`] implementation.

use core::future::Future;
use std::{collections::HashMap, fmt, sync::Arc};

use auto_impl::auto_impl;
use tronz_primitives::{Address, B256, RecoverableSignature};

use crate::{SignerError, TronSigner};

/// A wallet capable of signing any transaction for the TRON network.
///
/// Holds one or more signing credentials keyed by address.
///
/// Signer authors should implement [`TronSigner`] instead; implement this trait
/// only to plug custom credential routing into a provider. [`TronWallet`] is the
/// standard implementation.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait TronNetworkWallet: fmt::Debug + Send + Sync {
    /// The default credential's address, used by [`sign_hash`](Self::sign_hash)
    /// and [`sign_message`](Self::sign_message).
    fn default_signer_address(&self) -> Address;

    /// Whether this wallet holds a credential whose address is `address`.
    fn has_signer_for(&self, address: &Address) -> bool;

    /// The addresses of every credential this wallet holds.
    fn signer_addresses(&self) -> impl Iterator<Item = Address>;

    /// Sign a 32-byte transaction hash (`tx_id`) with the credential whose
    /// address is `key`.
    fn sign_hash_with(
        &self,
        key: Address,
        hash: &B256,
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send;

    /// Sign a plaintext message with the credential whose address is `key`.
    fn sign_message_with(
        &self,
        key: Address,
        message: &[u8],
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send;

    /// Sign a 32-byte transaction hash (`tx_id`) with the default credential.
    fn sign_hash(
        &self,
        hash: &B256,
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
        self.sign_hash_with(self.default_signer_address(), hash)
    }

    /// Sign a plaintext message with the default credential.
    fn sign_message(
        &self,
        message: &[u8],
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
        self.sign_message_with(self.default_signer_address(), message)
    }

    /// Sign one transaction hash with several credentials.
    ///
    /// Preserves key order, ignores duplicates, and errors on an unknown key.
    fn sign_hash_with_many(
        &self,
        keys: &[Address],
        hash: &B256,
    ) -> impl Future<Output = Result<Vec<RecoverableSignature>, SignerError>> + Send {
        async move {
            let mut signatures = Vec::with_capacity(keys.len());
            let mut signed: Vec<Address> = Vec::with_capacity(keys.len());
            for key in keys {
                if signed.contains(key) {
                    continue;
                }
                signed.push(*key);
                signatures.push(self.sign_hash_with(*key, hash).await?);
            }
            Ok(signatures)
        }
    }
}

/// A shared signing credential held by a [`TronWallet`].
type Credential = Arc<dyn TronSigner + Send + Sync + 'static>;

/// A cloneable wallet holding one or more TRON signing credentials.
///
/// ```
/// # use tronz_signer::{LocalSigner, TronNetworkWallet, TronWallet};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let a =
///     LocalSigner::from_hex("0000000000000000000000000000000000000000000000000000000000000001")?;
/// let b =
///     LocalSigner::from_hex("0000000000000000000000000000000000000000000000000000000000000002")?;
///
/// let mut wallet = TronWallet::new(a.clone());
/// wallet.register_signer(b.clone());
///
/// assert_eq!(wallet.default_signer_address(), a.address());
/// assert!(wallet.has_signer_for(&b.address()));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Default)]
pub struct TronWallet {
    default: Address,
    signers: HashMap<Address, Credential>,
}

impl<S> From<S> for TronWallet
where
    S: TronSigner + Send + Sync + 'static,
{
    fn from(signer: S) -> Self {
        Self::new(signer)
    }
}

impl TronWallet {
    /// Create a wallet whose default credential is `signer`.
    pub fn new<S>(signer: S) -> Self
    where
        S: TronSigner + Send + Sync + 'static,
    {
        let mut this = Self::default();
        this.register_default_signer(signer);
        this
    }

    /// Register a credential, keyed by its own address.
    pub fn register_signer<S>(&mut self, signer: S)
    where
        S: TronSigner + Send + Sync + 'static,
    {
        self.signers.insert(signer.address(), Arc::new(signer));
    }

    /// Register a credential and make it the default.
    pub fn register_default_signer<S>(&mut self, signer: S)
    where
        S: TronSigner + Send + Sync + 'static,
    {
        self.default = signer.address();
        self.register_signer(signer);
    }

    /// Point the default credential at a registered address.
    pub fn set_default_signer(&mut self, address: Address) -> Result<(), SignerError> {
        if self.signers.contains_key(&address) {
            self.default = address;
            Ok(())
        } else {
            Err(SignerError::message(format!(
                "{address} is not a registered signer; use `register_default_signer`"
            )))
        }
    }

    /// The default credential.
    ///
    /// # Panics
    ///
    /// Panics if the wallet holds no credentials, i.e. it was built with
    /// [`Default`] and nothing was registered.
    pub fn default_signer(&self) -> Credential {
        self.signers.get(&self.default).cloned().expect("wallet has no credentials")
    }

    /// The credential for `address`, if this wallet holds one.
    pub fn signer_by_address(&self, address: Address) -> Option<Credential> {
        self.signers.get(&address).cloned()
    }

    fn require_signer(&self, key: Address) -> Result<Credential, SignerError> {
        self.signer_by_address(key)
            .ok_or_else(|| SignerError::message(format!("missing signing credential for {key}")))
    }
}

impl TronNetworkWallet for TronWallet {
    fn default_signer_address(&self) -> Address {
        self.default
    }

    fn has_signer_for(&self, address: &Address) -> bool {
        self.signers.contains_key(address)
    }

    fn signer_addresses(&self) -> impl Iterator<Item = Address> {
        self.signers.keys().copied()
    }

    fn sign_hash_with(
        &self,
        key: Address,
        hash: &B256,
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
        let signer = self.require_signer(key);
        async move { signer?.sign_hash(hash).await }
    }

    fn sign_message_with(
        &self,
        key: Address,
        message: &[u8],
    ) -> impl Future<Output = Result<RecoverableSignature, SignerError>> + Send {
        let signer = self.require_signer(key);
        async move { signer?.sign_message(message).await }
    }
}

impl fmt::Debug for TronWallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TronWallet")
            .field("default_signer", &self.default)
            .field("credentials", &self.signers.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use async_trait::async_trait;

    use super::*;
    use crate::LocalSigner;

    const KEY_A: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const KEY_B: &str = "0000000000000000000000000000000000000000000000000000000000000002";
    const KEY_C: &str = "0000000000000000000000000000000000000000000000000000000000000003";

    struct NonCloneSigner {
        inner: LocalSigner,
        message_override_called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl TronSigner for NonCloneSigner {
        fn address(&self) -> Address {
            self.inner.address()
        }

        async fn sign_hash(&self, hash: &B256) -> Result<RecoverableSignature, SignerError> {
            self.inner.sign_hash(hash).await
        }

        async fn sign_message(&self, message: &[u8]) -> Result<RecoverableSignature, SignerError> {
            self.message_override_called.store(true, Ordering::SeqCst);
            self.inner.sign_message(message).await
        }
    }

    #[tokio::test]
    async fn owns_non_clone_signer_and_preserves_overrides_across_clones() {
        let called = Arc::new(AtomicBool::new(false));
        let wallet = TronWallet::new(NonCloneSigner {
            inner: LocalSigner::from_hex(KEY_A).unwrap(),
            message_override_called: Arc::clone(&called),
        });
        let cloned = wallet.clone();

        let signature = cloned.sign_message(b"wallet").await.unwrap();
        assert!(called.load(Ordering::SeqCst));
        assert!(tronz_primitives::verify_message(
            b"wallet",
            &signature,
            wallet.default_signer_address()
        ));
    }

    #[tokio::test]
    async fn routes_to_the_credential_named_by_key() {
        let a = LocalSigner::from_hex(KEY_A).unwrap();
        let b = LocalSigner::from_hex(KEY_B).unwrap();

        let mut wallet = TronWallet::new(a.clone());
        wallet.register_signer(b.clone());

        let hash = B256::repeat_byte(7);
        let sig_a = wallet.sign_hash_with(a.address(), &hash).await.unwrap();
        let sig_b = wallet.sign_hash_with(b.address(), &hash).await.unwrap();

        assert_ne!(sig_a, sig_b);
        assert_eq!(sig_a.recover_address_from_prehash(hash).unwrap(), a.address());
        assert_eq!(sig_b.recover_address_from_prehash(hash).unwrap(), b.address());
        // Unqualified signing uses the default credential.
        assert_eq!(wallet.sign_hash(&hash).await.unwrap(), sig_a);
    }

    #[tokio::test]
    async fn unknown_key_is_an_error() {
        let wallet = TronWallet::new(LocalSigner::from_hex(KEY_A).unwrap());
        let stranger = LocalSigner::from_hex(KEY_B).unwrap().address();

        assert!(!wallet.has_signer_for(&stranger));
        let err = wallet.sign_hash_with(stranger, &B256::ZERO).await.unwrap_err();
        assert!(err.to_string().contains("missing signing credential"));
    }

    #[tokio::test]
    async fn signs_one_hash_with_several_keys_in_order() {
        let a = LocalSigner::from_hex(KEY_A).unwrap();
        let b = LocalSigner::from_hex(KEY_B).unwrap();

        let mut wallet = TronWallet::new(a.clone());
        wallet.register_signer(b.clone());

        let hash = B256::repeat_byte(7);
        let signatures =
            wallet.sign_hash_with_many(&[b.address(), a.address()], &hash).await.unwrap();

        let signers: Vec<_> =
            signatures.iter().map(|sig| sig.recover_address_from_prehash(hash).unwrap()).collect();
        assert_eq!(signers, vec![b.address(), a.address()]);
    }

    #[tokio::test]
    async fn a_repeated_key_signs_once() {
        let a = LocalSigner::from_hex(KEY_A).unwrap();
        let wallet = TronWallet::new(a.clone());

        let signatures =
            wallet.sign_hash_with_many(&[a.address(), a.address()], &B256::repeat_byte(7)).await;

        assert_eq!(signatures.unwrap().len(), 1);
    }

    /// All-or-nothing: a missing key must not yield a partially signed set that
    /// looks complete to the caller.
    #[tokio::test]
    async fn signing_with_many_keys_fails_on_the_first_unheld_one() {
        let a = LocalSigner::from_hex(KEY_A).unwrap();
        let stranger = LocalSigner::from_hex(KEY_B).unwrap().address();
        let wallet = TronWallet::new(a.clone());

        let err = wallet
            .sign_hash_with_many(&[a.address(), stranger], &B256::repeat_byte(7))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("missing signing credential"));
    }

    #[test]
    fn default_signer_can_be_repointed_only_to_registered_credentials() {
        let a = LocalSigner::from_hex(KEY_A).unwrap();
        let b = LocalSigner::from_hex(KEY_B).unwrap();

        let mut wallet = TronWallet::new(a.clone());
        wallet.register_signer(b.clone());
        assert_eq!(wallet.default_signer_address(), a.address());

        wallet.set_default_signer(b.address()).unwrap();
        assert_eq!(wallet.default_signer_address(), b.address());
        assert_eq!(wallet.default_signer().address(), b.address());

        let stranger = LocalSigner::from_hex(KEY_C).unwrap().address();
        assert!(wallet.set_default_signer(stranger).is_err());
        assert_eq!(wallet.default_signer_address(), b.address());
    }

    #[test]
    fn signer_addresses_lists_every_credential() {
        let a = LocalSigner::from_hex(KEY_A).unwrap();
        let b = LocalSigner::from_hex(KEY_B).unwrap();

        let mut wallet = TronWallet::from(a.clone());
        wallet.register_signer(b.clone());

        let mut addresses: Vec<_> = wallet.signer_addresses().collect();
        addresses.sort_unstable();
        let mut expected = vec![a.address(), b.address()];
        expected.sort_unstable();
        assert_eq!(addresses, expected);
    }

    #[test]
    fn re_registering_an_address_replaces_the_credential() {
        let a = LocalSigner::from_hex(KEY_A).unwrap();
        let mut wallet = TronWallet::new(a.clone());
        wallet.register_signer(a.clone());

        assert_eq!(wallet.signer_addresses().count(), 1);
    }
}
