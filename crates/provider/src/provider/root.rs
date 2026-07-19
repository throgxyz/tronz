//! The base [`RootProvider`] over a transport.

use std::sync::Arc;

use tronz_primitives::{Address, TxId};

use crate::{
    error::{ProviderError, Result},
    provider::{ContractReadProvider, TronProvider},
    transport::TronTransport,
    types::{ConstantCallResult, TransactionInfo, TriggerSmartContract},
};

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
        Self { inner: Arc::new(RootProviderInner { transport, signer_address: None }) }
    }

    /// Create a provider that knows its signer's address.
    pub fn new_with_signer(transport: T, signer_address: Address) -> Self {
        Self {
            inner: Arc::new(RootProviderInner { transport, signer_address: Some(signer_address) }),
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

impl<T: TronTransport> crate::provider::private::Sealed for RootProvider<T> {}
impl<T: TronTransport> crate::provider::private::ContractReadSealed for RootProvider<T> {}

impl<T: TronTransport> ContractReadProvider for RootProvider<T> {
    fn default_caller(&self) -> Option<Address> {
        RootProvider::signer_address(self)
    }

    async fn call_contract(&self, params: TriggerSmartContract) -> Result<ConstantCallResult> {
        self.transport().trigger_constant_contract(params).await.map_err(ProviderError::transport)
    }

    async fn estimate_contract_energy(&self, params: TriggerSmartContract) -> Result<i64> {
        self.transport().estimate_energy(params).await.map_err(ProviderError::transport)
    }

    async fn transaction_info(&self, tx_id: TxId) -> Result<Option<TransactionInfo>> {
        self.transport().get_transaction_info(tx_id).await.map_err(ProviderError::transport)
    }

    async fn transaction_infos_by_block(&self, block_num: i64) -> Result<Vec<TransactionInfo>> {
        self.transport()
            .get_transaction_info_by_block_num(block_num)
            .await
            .map_err(ProviderError::transport)
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
