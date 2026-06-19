//! The base [`RootProvider`] over a transport.

use std::sync::Arc;

use tronz_primitives::Address;

use crate::{provider::TronProvider, transport::TronTransport};

/// The base provider: wraps a transport (and optional signer address) in an
/// `Arc` so it is cheap to clone and `Send + Sync`.
#[derive(Clone)]
pub struct RootProvider<T: TronTransport> {
    inner: Arc<RootProviderInner<T>>,
}

struct RootProviderInner<T> {
    transport: T,
    signer_address: Option<Address>,
}

impl<T: TronTransport> RootProvider<T> {
    /// Create a read-only provider.
    pub fn new(transport: T) -> Self {
        Self {
            inner: Arc::new(RootProviderInner {
                transport,
                signer_address: None,
            }),
        }
    }

    /// Create a provider that knows its signer's address.
    pub fn new_with_signer(transport: T, signer_address: Address) -> Self {
        Self {
            inner: Arc::new(RootProviderInner {
                transport,
                signer_address: Some(signer_address),
            }),
        }
    }

    /// Borrow the transport.
    pub fn transport(&self) -> &T {
        &self.inner.transport
    }

    /// The signer address, if known.
    pub fn signer_address(&self) -> Option<Address> {
        self.inner.signer_address
    }
}

impl<T: TronTransport> TronProvider for RootProvider<T> {
    type Transport = T;

    fn transport(&self) -> &T {
        RootProvider::transport(self)
    }

    fn signer_address(&self) -> Option<Address> {
        RootProvider::signer_address(self)
    }
}
