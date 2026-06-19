//! BIP-39 mnemonic + BIP-44 HD key derivation for TRON.
//!
//! TRON's registered BIP-44 coin type is **195**, so the default derivation
//! path is `m/44'/195'/0'/0/{index}`.
//!
//! # Example
//!
//! ```rust
//! use tronz_signer::mnemonic::MnemonicBuilder;
//! use tronz_signer::TronSigner as _;
//! use coins_bip39::English;
//!
//! // From an existing phrase
//! let signer = MnemonicBuilder::<English>::default()
//!     .phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
//!     .index(0).unwrap()
//!     .build()
//!     .unwrap();
//! println!("{}", signer.address());
//!
//! // Generate a random 24-word phrase
//! let (signer, phrase) = MnemonicBuilder::<English>::default()
//!     .word_count(24)
//!     .build_random()
//!     .unwrap();
//! println!("phrase: {phrase}");
//! ```

use std::{marker::PhantomData, path::PathBuf};

use coins_bip32::{path::DerivationPath, prelude::Parent, xkeys::XPriv};
use coins_bip39::{English, Mnemonic, Wordlist};
use thiserror::Error;

use crate::{LocalSigner, SignerError, TronSigner};

/// TRON BIP-44 coin type (registered as 195).
const TRON_COIN_TYPE: u32 = 195;

const DEFAULT_DERIVATION_PATH_PREFIX: &str = "m/44'/195'/0'/0/";
const DEFAULT_DERIVATION_PATH: &str = "m/44'/195'/0'/0/0";

// ─── MnemonicBuilder ──────────────────────────────────────────────────────────

/// Builder that derives a [`LocalSigner`] from a BIP-39 mnemonic phrase.
///
/// The default derivation path is `m/44'/195'/0'/0/0` (TRON, account 0, index 0).
///
/// # Workflow
///
/// 1. Supply a phrase **or** a word count for random generation.
/// 2. Optionally set a BIP-39 passphrase, a custom derivation path, or an index shortcut.
/// 3. Call [`build`](Self::build) / [`build_random`](Self::build_random).
#[derive(Clone, Debug, PartialEq)]
#[must_use = "builders do nothing unless `build` or `build_random` is called"]
pub struct MnemonicBuilder<W: Wordlist = English> {
    phrase: Option<String>,
    word_count: usize,
    derivation_path: DerivationPath,
    password: Option<String>,
    /// If set, the mnemonic phrase is written to `write_to/<tron-address>` on
    /// random builds.
    write_to: Option<PathBuf>,
    _wordlist: PhantomData<W>,
}

impl<W: Wordlist> Default for MnemonicBuilder<W> {
    fn default() -> Self {
        Self {
            phrase: None,
            word_count: 12,
            derivation_path: DEFAULT_DERIVATION_PATH.parse().unwrap(),
            password: None,
            write_to: None,
            _wordlist: PhantomData,
        }
    }
}

// ── Convenience constructors for the common English case ─────────────────────

impl MnemonicBuilder<English> {
    /// Create a builder with the English wordlist.
    pub fn english() -> Self {
        Self::default()
    }

    /// Create a builder from `phrase` using the English wordlist.
    pub fn from_phrase<P: Into<String>>(phrase: P) -> Self {
        Self::english().phrase(phrase)
    }

    /// Derive the signer at `m/44'/195'/0'/0/{index}` from `phrase`.
    pub fn try_from_phrase_nth<P: Into<String>>(
        phrase: P,
        index: u32,
    ) -> Result<LocalSigner, SignerError> {
        Self::from_phrase(phrase).index(index)?.build()
    }

    /// Derive the signer at `m/44'/195'/0'/0/0` from `phrase`.
    pub fn try_from_phrase_first<P: Into<String>>(phrase: P) -> Result<LocalSigner, SignerError> {
        Self::try_from_phrase_nth(phrase, 0)
    }
}

// ── Builder methods ───────────────────────────────────────────────────────────

impl<W: Wordlist> MnemonicBuilder<W> {
    /// Set the mnemonic phrase. Calling this makes [`build`](Self::build) work.
    pub fn phrase<P: Into<String>>(mut self, phrase: P) -> Self {
        self.phrase = Some(phrase.into());
        self
    }

    /// Number of words to generate when calling [`build_random`](Self::build_random).
    /// Ignored if a phrase has been set. Valid values: 12, 15, 18, 21, 24.
    pub const fn word_count(mut self, count: usize) -> Self {
        self.word_count = count;
        self
    }

    /// Shortcut: set the derivation path to `m/44'/195'/0'/0/{index}`.
    pub fn index(self, index: u32) -> Result<Self, SignerError> {
        self.derivation_path(format!("{DEFAULT_DERIVATION_PATH_PREFIX}{index}"))
    }

    /// Set a fully custom BIP-32 derivation path (e.g. `"m/44'/195'/1'/0/5"`).
    pub fn derivation_path<T: AsRef<str>>(mut self, path: T) -> Result<Self, SignerError> {
        self.derivation_path = path.as_ref().parse()?;
        Ok(self)
    }

    /// Return the configured derivation path.
    pub const fn get_derivation_path(&self) -> &DerivationPath {
        &self.derivation_path
    }

    /// BIP-39 passphrase (not the same as the mnemonic words).
    pub fn password<T: Into<String>>(mut self, password: T) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Directory to write the generated phrase into on random builds.
    /// The file is named after the TRON address (base58).
    pub fn write_to<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.write_to = Some(path.into());
        self
    }

    // ── Build methods ─────────────────────────────────────────────────────────

    /// Derive a [`LocalSigner`] from the configured phrase.
    ///
    /// Returns an error if no phrase has been set.
    pub fn build(&self) -> Result<LocalSigner, SignerError> {
        let phrase = self
            .phrase
            .as_deref()
            .ok_or(MnemonicBuilderError::MissingPhrase)?;
        let mnemonic = Mnemonic::<W>::new_from_phrase(phrase)?;
        xpriv_to_local_signer(
            &mnemonic.derive_key(&self.derivation_path, self.password.as_deref())?,
        )
    }

    /// Generate a random mnemonic using `rand::thread_rng` and derive a [`LocalSigner`].
    ///
    /// Returns `(signer, phrase_string)`. If [`write_to`](Self::write_to) was
    /// set the phrase is also written to `<dir>/<tron-address>`.
    ///
    /// Returns an error if a phrase has already been set.
    pub fn build_random(&self) -> Result<(LocalSigner, String), SignerError> {
        self.build_random_with(&mut rand::thread_rng())
    }

    /// Same as [`build_random`](Self::build_random) but with an explicit RNG.
    pub fn build_random_with<R: rand::Rng>(
        &self,
        rng: &mut R,
    ) -> Result<(LocalSigner, String), SignerError> {
        if self.phrase.is_some() {
            return Err(MnemonicBuilderError::PhraseAlreadySet.into());
        }
        let mnemonic = Mnemonic::<W>::new_with_count(rng, self.word_count)?;
        let signer = xpriv_to_local_signer(
            &mnemonic.derive_key(&self.derivation_path, self.password.as_deref())?,
        )?;
        let phrase = mnemonic.to_phrase();

        if let Some(dir) = &self.write_to {
            std::fs::write(dir.join(signer.address().to_string()), phrase.as_bytes())?;
        }

        Ok((signer, phrase))
    }

    /// Derive a [`MnemonicKey`] at the *parent* path (all but the last component).
    ///
    /// Useful for deriving many sequential accounts without re-processing the
    /// whole BIP-39 derivation each time.
    pub fn build_parent_key(&self) -> Result<MnemonicKey, SignerError> {
        let phrase = self
            .phrase
            .as_deref()
            .ok_or(MnemonicBuilderError::MissingPhrase)?;
        let mnemonic = Mnemonic::<W>::new_from_phrase(phrase)?;
        let mut key = mnemonic.master_key(self.password.as_deref())?;
        // Traverse all but the last path component.
        let depth = self.derivation_path.len();
        if depth > 1 {
            for &child_index in self.derivation_path.iter().take(depth - 1) {
                key = key.derive_child(child_index)?;
            }
        }
        Ok(MnemonicKey { key })
    }
}

// ── IntoIterator ──────────────────────────────────────────────────────────────

impl<W: Wordlist> IntoIterator for MnemonicBuilder<W> {
    type Item = Result<LocalSigner, SignerError>;
    type IntoIter = MnemonicSignerIter;

    /// Iterate over signers at consecutive derivation indices starting from the
    /// last index in the configured path.
    ///
    /// The start index is taken from the last component of the current
    /// derivation path. Use the default (`.index(0)`) or explicitly call
    /// `.index(0)` before iterating if you want to start from index 0.
    fn into_iter(self) -> Self::IntoIter {
        if self.phrase.is_none() {
            return MnemonicSignerIter::missing_phrase();
        }
        let start = self.derivation_path.last().copied().unwrap_or(0);
        match self.build_parent_key() {
            Ok(key) => key.children_from(start),
            Err(e) => MnemonicSignerIter::error(e),
        }
    }
}

// ─── MnemonicKey ──────────────────────────────────────────────────────────────

/// An extended private key anchored at a BIP-32 path prefix.
///
/// Created by [`MnemonicBuilder::build_parent_key`]. Use [`child`](Self::child)
/// to derive individual signers, or iterate to get sequential signers.
#[derive(Debug, Clone)]
pub struct MnemonicKey {
    key: XPriv,
}

impl MnemonicKey {
    /// Derive a child key at `index` (unhardened).
    pub fn child(&self, index: u32) -> Result<Self, SignerError> {
        Ok(Self {
            key: self.key.derive_child(index)?,
        })
    }

    /// Extract a [`LocalSigner`] from this key.
    pub fn signer(&self) -> Result<LocalSigner, SignerError> {
        xpriv_to_local_signer(&self.key)
    }

    /// Iterator over sequential child signers starting at index 0.
    pub fn children(&self) -> MnemonicSignerIter {
        self.children_from(0)
    }

    /// Iterator over sequential child signers starting at `start`.
    pub fn children_from(&self, start: u32) -> MnemonicSignerIter {
        MnemonicSignerIter {
            state: IterState::Active {
                key: self.clone(),
                next: start,
            },
        }
    }
}

impl IntoIterator for MnemonicKey {
    type Item = Result<LocalSigner, SignerError>;
    type IntoIter = MnemonicSignerIter;

    fn into_iter(self) -> Self::IntoIter {
        self.children()
    }
}

// ─── MnemonicSignerIter ───────────────────────────────────────────────────────

/// Infinite iterator that yields [`LocalSigner`]s at consecutive child indices.
#[derive(Debug)]
pub struct MnemonicSignerIter {
    state: IterState,
}

#[derive(Debug)]
enum IterState {
    Active {
        key: MnemonicKey,
        next: u32,
    },
    /// Emits one error then stops.
    Error {
        error: Option<SignerError>,
    },
}

impl MnemonicSignerIter {
    fn missing_phrase() -> Self {
        Self {
            state: IterState::Error {
                error: Some(MnemonicBuilderError::MissingPhrase.into()),
            },
        }
    }

    fn error(e: SignerError) -> Self {
        Self {
            state: IterState::Error { error: Some(e) },
        }
    }
}

impl Iterator for MnemonicSignerIter {
    type Item = Result<LocalSigner, SignerError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            IterState::Active { key, next } => {
                let idx = *next;
                *next = next.checked_add(1)?;
                Some(key.child(idx).and_then(|k| k.signer()))
            }
            IterState::Error { error } => error.take().map(Err),
        }
    }
}

// ─── MnemonicBuilderError ─────────────────────────────────────────────────────

/// Errors specific to [`MnemonicBuilder`] misuse (wrong call sequence).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MnemonicBuilderError {
    /// [`build`](MnemonicBuilder::build) requires a phrase but none was set.
    #[error("phrase is required but was not set — call .phrase() first")]
    MissingPhrase,
    /// [`build_random`](MnemonicBuilder::build_random) cannot be used when a
    /// phrase has already been set.
    #[error("a phrase was already set — use .build() instead of .build_random()")]
    PhraseAlreadySet,
}

// ─── TRON coin type constant ──────────────────────────────────────────────────

/// TRON BIP-44 coin type (195), for documentation and custom-path construction.
pub const TRON_BIP44_COIN_TYPE: u32 = TRON_COIN_TYPE;

// ─── Internal helper ──────────────────────────────────────────────────────────

fn xpriv_to_local_signer(xpriv: &XPriv) -> Result<LocalSigner, SignerError> {
    // coins-bip32's SigningKey is a re-export of k256::ecdsa::SigningKey.
    let signing_key: &k256::ecdsa::SigningKey = xpriv.as_ref();
    let bytes: [u8; 32] = signing_key.to_bytes().into();
    LocalSigner::from_bytes(&bytes)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use coins_bip39::English;
    use hex;

    use super::*;

    // Standard BIP-39 test vector (all "abandon" + "about").
    // Private key verified against gotron-sdk TestFromSeedAndPassphrase (m/44'/195'/0'/0/0).
    const ABANDON_PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    // Private key for the abandon phrase at index 0 — cross-checked with gotron-sdk.
    const ABANDON_PRIVKEY_0: &str =
        "b5a4cea271ff424d7c31dc12a3e43e401df7a40d7412a15750f3f0b6b5449a28";

    // TRON address derived from the above private key.
    const ABANDON_ADDR_0: &str = "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH";

    #[test]
    fn abandon_phrase_index_0_private_key() {
        // Verify the derived private key matches the gotron-sdk reference vector.
        let signer = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .index(0)
            .unwrap()
            .build()
            .unwrap();
        let key_bytes: [u8; 32] = signer.signing_key().to_bytes().into();
        assert_eq!(hex::encode(key_bytes), ABANDON_PRIVKEY_0);
    }

    #[test]
    fn abandon_phrase_index_0_address() {
        // Address is determined by the private key above; this locks in the TRON encoding.
        let signer = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .index(0)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(signer.address().to_string(), ABANDON_ADDR_0);
    }

    #[test]
    fn deterministic_across_builds() {
        let build = || {
            MnemonicBuilder::<English>::default()
                .phrase(ABANDON_PHRASE)
                .index(0)
                .unwrap()
                .build()
                .unwrap()
        };
        assert_eq!(build().address(), build().address());
    }

    #[test]
    fn different_indices_give_different_addresses() {
        let addr = |i: u32| {
            MnemonicBuilder::<English>::default()
                .phrase(ABANDON_PHRASE)
                .index(i)
                .unwrap()
                .build()
                .unwrap()
                .address()
        };
        assert_ne!(addr(0), addr(1));
        assert_ne!(addr(1), addr(2));
    }

    #[test]
    fn build_random_returns_valid_signer() {
        let (signer, phrase) = MnemonicBuilder::<English>::default()
            .word_count(12)
            .build_random()
            .unwrap();
        // Phrase must be 12 words.
        assert_eq!(phrase.split_whitespace().count(), 12);
        // Address must start with 'T' (base58check, TRON prefix byte 0x41).
        assert!(signer.address().to_string().starts_with('T'));
    }

    #[test]
    fn random_and_phrase_are_consistent() {
        let (_, phrase) = MnemonicBuilder::<English>::default()
            .word_count(12)
            .build_random()
            .unwrap();
        let s1 = MnemonicBuilder::<English>::default()
            .phrase(&phrase)
            .build()
            .unwrap();
        let s2 = MnemonicBuilder::<English>::default()
            .phrase(&phrase)
            .build()
            .unwrap();
        assert_eq!(s1.address(), s2.address());
    }

    #[test]
    fn parent_key_children_match_index_build() {
        let parent = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .build_parent_key()
            .unwrap();

        for i in 0u32..3 {
            let from_parent = parent.child(i).unwrap().signer().unwrap();
            let from_index = MnemonicBuilder::<English>::default()
                .phrase(ABANDON_PHRASE)
                .index(i)
                .unwrap()
                .build()
                .unwrap();
            assert_eq!(from_parent.address(), from_index.address());
        }
    }

    #[test]
    fn iterator_yields_sequential_signers() {
        let signers: Vec<_> = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .into_iter()
            .take(3)
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(signers.len(), 3);
        // All different.
        assert_ne!(signers[0].address(), signers[1].address());
        assert_ne!(signers[1].address(), signers[2].address());
        // First matches index(0).
        let index0 = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .index(0)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(signers[0].address(), index0.address());
    }

    #[test]
    fn missing_phrase_returns_error() {
        assert!(MnemonicBuilder::<English>::default().build().is_err());
        let err = MnemonicBuilder::<English>::default()
            .into_iter()
            .next()
            .unwrap()
            .unwrap_err();
        assert!(err.to_string().contains("phrase"));
    }

    #[test]
    fn phrase_set_random_returns_error() {
        let result = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .build_random();
        assert!(result.is_err());
    }

    #[test]
    fn custom_derivation_path() {
        // m/44'/195'/1'/0/0 should differ from m/44'/195'/0'/0/0
        let s0 = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .index(0)
            .unwrap()
            .build()
            .unwrap();
        let s_custom = MnemonicBuilder::<English>::default()
            .phrase(ABANDON_PHRASE)
            .derivation_path("m/44'/195'/1'/0/0")
            .unwrap()
            .build()
            .unwrap();
        assert_ne!(s0.address(), s_custom.address());
    }
}
