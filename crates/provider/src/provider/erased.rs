//! [`DynProvider`], a type-erased provider.

use std::sync::Arc;

use async_trait::async_trait;

use crate::provider::{ContractReadProvider, RootProvider, TronProvider};

/// A [`TronProvider`] with its concrete type erased.
///
/// Reach for this when a provider has to be stored in a struct, returned from a
/// function, or held in a collection alongside providers stacked differently —
/// anywhere the full filler-and-layer type would have to be spelled out. Cloning
/// is a refcount bump, and every call costs one extra pointer hop.
///
/// Everything a wrapped provider does survives erasure: this type reports the one it
/// holds as its [`inner`](TronProvider::inner), so every call travels down through it
/// rather than around it.
///
/// # Examples
///
/// ```no_run
/// # use tronz_provider::{DynProvider, ProviderBuilder, TronProvider};
/// # use tronz_signer::TronWallet;
/// # async fn run(wallet: TronWallet) -> tronz_provider::Result<()> {
/// let provider = ProviderBuilder::new().wallet(wallet).connect("grpc.trongrid.io:50051").await?;
///
/// // Two differently-stacked providers, one type.
/// let providers: Vec<DynProvider> = vec![provider.erased()];
/// for p in &providers {
///     let _ = p.get_now_block().await?;
/// }
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct DynProvider(Arc<dyn TronProvider>);

impl DynProvider {
    /// Erase a provider's type.
    ///
    /// Same as [`provider.erased()`](TronProvider::erased).
    pub fn new<P: TronProvider>(provider: P) -> Self {
        Self(Arc::new(provider))
    }
}

impl core::fmt::Debug for DynProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DynProvider").finish()
    }
}

impl ContractReadProvider for DynProvider {
    fn inner_read(&self) -> Option<&dyn ContractReadProvider> {
        Some(&*self.0)
    }
}

#[async_trait]
impl TronProvider for DynProvider {
    fn root(&self) -> &RootProvider {
        self.0.root()
    }

    fn inner(&self) -> Option<&dyn TronProvider> {
        Some(&*self.0)
    }

    fn erased(self) -> DynProvider {
        self
    }
}
