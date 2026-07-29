//! The base [`RootProvider`] over a transport.

use std::sync::Arc;

use async_trait::async_trait;
use tronz_primitives::{Address, TxId};

use crate::{
    error::{ProviderError, Result},
    provider::{ContractReadProvider, TronProvider},
    transport::{DynTransport, TronTransport},
    types::{ConstantCallResult, TransactionInfo, TriggerSmartContract},
};

/// The base provider: the transport every other provider ultimately reaches,
/// plus the attached signer's address.
///
/// The transport is erased on the way in, which is what lets this type be named
/// without naming it — see [`TronProvider::root`], the one method a provider has
/// to supply. Cloning is a refcount bump.
#[derive(Clone)]
pub struct RootProvider {
    inner: Arc<RootProviderInner>,
}

struct RootProviderInner {
    transport: DynTransport,
    signer_address: Option<Address>,
}

impl RootProvider {
    /// Create a read-only provider.
    pub fn new(transport: impl TronTransport) -> Self {
        Self::build(Arc::new(transport), None)
    }

    /// Create a provider that knows its signer's address.
    pub fn new_with_signer(transport: impl TronTransport, signer_address: Address) -> Self {
        Self::build(Arc::new(transport), Some(signer_address))
    }

    /// Create a provider over an already-erased transport, without boxing twice.
    pub fn new_erased(transport: DynTransport) -> Self {
        Self::build(transport, None)
    }

    fn build(transport: DynTransport, signer_address: Option<Address>) -> Self {
        Self { inner: Arc::new(RootProviderInner { transport, signer_address }) }
    }

    /// Borrow the transport.
    pub fn transport(&self) -> &dyn TronTransport {
        &*self.inner.transport
    }

    /// The signer address, if known.
    pub fn signer_address(&self) -> Option<Address> {
        self.inner.signer_address
    }
}

#[async_trait]
impl ContractReadProvider for RootProvider {
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

#[async_trait]
impl TronProvider for RootProvider {
    fn root(&self) -> &RootProvider {
        self
    }
}
