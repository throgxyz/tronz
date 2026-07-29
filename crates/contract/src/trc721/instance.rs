//! Provider-bound TRC721 contract instance.

use alloy_sol_types::SolCall;
use tronz_primitives::{Address, Bytes, U256};
use tronz_provider::{ContractReadProvider, PendingTransaction, TronProvider};

use crate::{
    error::{ContractError, Result},
    instance::ContractInstance,
    sol_call::TronCallBuilder,
    trc721::ITRC721,
};

/// Errors returned by [`Trc721Instance`] methods.
pub type Trc721Error = ContractError;

/// A provider-bound handle to a TRC721 contract.
///
/// Construct via [`Trc721Ext::trc721`] on any provider:
///
/// ```no_run
/// # use tronz_contract::trc721::Trc721Ext;
/// # use tronz_primitives::Address;
/// # async fn run(provider: impl tronz_provider::ContractReadProvider + Clone) {
/// let contract: Address = "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf".parse().unwrap();
/// let caller: Address = "TXYJg94nXn8jDVVK4yg4B8yXWNR1pQxv6f".parse().unwrap();
/// let token = provider.trc721(contract).caller(caller);
/// let name = token.name().await.unwrap();
/// # }
/// ```
#[derive(Clone)]
pub struct Trc721Instance<P: ContractReadProvider> {
    inner: ContractInstance<P>,
}

impl<P: ContractReadProvider + Clone> Trc721Instance<P> {
    /// Bind to the TRC721 contract at `address`.
    pub fn new(provider: P, address: Address) -> Self {
        Self { inner: ContractInstance::new_raw(provider, address) }
    }

    /// The contract address.
    pub fn address(&self) -> Address {
        self.inner.address()
    }

    /// Borrow the underlying provider.
    pub fn provider(&self) -> &P {
        self.inner.provider()
    }

    /// Return a new instance pointing at a different address.
    pub fn at(self, address: Address) -> Self {
        Self { inner: self.inner.at(address) }
    }

    /// Set the default account calls are made as: the simulated
    /// `msg.sender` for reads, the transaction owner for writes.
    pub fn caller(self, caller: Address) -> Self {
        Self { inner: self.inner.caller(caller) }
    }

    fn typed_call<C: SolCall>(&self, call: &C) -> TronCallBuilder<P, C> {
        TronCallBuilder::new(self.inner.call_raw(call.abi_encode().into()))
    }

    /// Build a typed `name()` call.
    pub fn name_call(&self) -> TronCallBuilder<P, ITRC721::nameCall> {
        self.typed_call(&ITRC721::nameCall {})
    }

    /// Fetch the token name.
    pub async fn name(&self) -> Result<String, Trc721Error> {
        self.name_call().call().await
    }

    /// Build a typed `symbol()` call.
    pub fn symbol_call(&self) -> TronCallBuilder<P, ITRC721::symbolCall> {
        self.typed_call(&ITRC721::symbolCall {})
    }

    /// Fetch the token symbol.
    pub async fn symbol(&self) -> Result<String, Trc721Error> {
        self.symbol_call().call().await
    }

    /// Build a typed `tokenURI(tokenId)` call.
    pub fn token_uri_call(&self, token_id: U256) -> TronCallBuilder<P, ITRC721::tokenURICall> {
        self.typed_call(&ITRC721::tokenURICall { tokenId: token_id })
    }

    /// Fetch the metadata URI for `token_id`.
    pub async fn token_uri(&self, token_id: U256) -> Result<String, Trc721Error> {
        self.token_uri_call(token_id).call().await
    }

    /// Build a typed `balanceOf(owner)` call.
    pub fn balance_of_call(&self, owner: Address) -> TronCallBuilder<P, ITRC721::balanceOfCall> {
        self.typed_call(&ITRC721::balanceOfCall { owner: owner.into() })
    }

    /// Fetch the number of tokens owned by `owner`.
    pub async fn balance_of(&self, owner: Address) -> Result<U256, Trc721Error> {
        self.balance_of_call(owner).call().await
    }

    /// Build a typed `ownerOf(tokenId)` call.
    pub fn owner_of_call(&self, token_id: U256) -> TronCallBuilder<P, ITRC721::ownerOfCall> {
        self.typed_call(&ITRC721::ownerOfCall { tokenId: token_id })
    }

    /// Fetch the owner of `token_id`.
    pub async fn owner_of(&self, token_id: U256) -> Result<Address, Trc721Error> {
        self.owner_of_call(token_id).call().await.map(Into::into)
    }

    /// Build a typed `getApproved(tokenId)` call.
    pub fn get_approved_call(
        &self,
        token_id: U256,
    ) -> TronCallBuilder<P, ITRC721::getApprovedCall> {
        self.typed_call(&ITRC721::getApprovedCall { tokenId: token_id })
    }

    /// Fetch the approved address for `token_id`, if any.
    pub async fn get_approved(&self, token_id: U256) -> Result<Address, Trc721Error> {
        self.get_approved_call(token_id).call().await.map(Into::into)
    }

    /// Build a typed `isApprovedForAll(owner, operator)` call.
    pub fn is_approved_for_all_call(
        &self,
        owner: Address,
        operator: Address,
    ) -> TronCallBuilder<P, ITRC721::isApprovedForAllCall> {
        self.typed_call(&ITRC721::isApprovedForAllCall {
            owner: owner.into(),
            operator: operator.into(),
        })
    }

    /// Returns `true` if `operator` is approved to manage all of `owner`'s tokens.
    pub async fn is_approved_for_all(
        &self,
        owner: Address,
        operator: Address,
    ) -> Result<bool, Trc721Error> {
        self.is_approved_for_all_call(owner, operator).call().await
    }

    /// Build a typed `transferFrom(from, to, tokenId)` call.
    pub fn transfer_from_call(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> TronCallBuilder<P, ITRC721::transferFromCall> {
        self.typed_call(&ITRC721::transferFromCall {
            from: from.into(),
            to: to.into(),
            tokenId: token_id,
        })
    }

    /// Build the three-argument `safeTransferFrom` overload.
    pub fn safe_transfer_from_call(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> TronCallBuilder<P, ITRC721::safeTransferFrom_0Call> {
        self.typed_call(&ITRC721::safeTransferFrom_0Call {
            from: from.into(),
            to: to.into(),
            tokenId: token_id,
        })
    }

    /// Build the four-argument `safeTransferFrom` overload with recipient data.
    pub fn safe_transfer_from_with_data_call(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
        data: Bytes,
    ) -> TronCallBuilder<P, ITRC721::safeTransferFrom_1Call> {
        self.typed_call(&ITRC721::safeTransferFrom_1Call {
            from: from.into(),
            to: to.into(),
            tokenId: token_id,
            data,
        })
    }

    /// Build a typed `approve(to, tokenId)` call.
    pub fn approve_call(
        &self,
        to: Address,
        token_id: U256,
    ) -> TronCallBuilder<P, ITRC721::approveCall> {
        self.typed_call(&ITRC721::approveCall { to: to.into(), tokenId: token_id })
    }

    /// Build a typed `setApprovalForAll(operator, approved)` call.
    pub fn set_approval_for_all_call(
        &self,
        operator: Address,
        approved: bool,
    ) -> TronCallBuilder<P, ITRC721::setApprovalForAllCall> {
        self.typed_call(&ITRC721::setApprovalForAllCall { operator: operator.into(), approved })
    }
}

impl<P: TronProvider + Clone> Trc721Instance<P> {
    /// Transfer `token_id` from `from` to `to`.
    pub async fn transfer_from(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<PendingTransaction, Trc721Error> {
        self.transfer_from_call(from, to, token_id).send().await
    }

    /// Safe-transfer `token_id` from `from` to `to` (calls `onERC721Received` on the recipient).
    pub async fn safe_transfer_from(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<PendingTransaction, Trc721Error> {
        self.safe_transfer_from_call(from, to, token_id).send().await
    }

    /// Safe-transfer `token_id` with recipient callback data.
    pub async fn safe_transfer_from_with_data(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
        data: Bytes,
    ) -> Result<PendingTransaction, Trc721Error> {
        self.safe_transfer_from_with_data_call(from, to, token_id, data).send().await
    }

    /// Approve `to` to transfer `token_id`.
    pub async fn approve(
        &self,
        to: Address,
        token_id: U256,
    ) -> Result<PendingTransaction, Trc721Error> {
        self.approve_call(to, token_id).send().await
    }

    /// Approve or revoke `operator` to manage all of the signer's tokens.
    pub async fn set_approval_for_all(
        &self,
        operator: Address,
        approved: bool,
    ) -> Result<PendingTransaction, Trc721Error> {
        self.set_approval_for_all_call(operator, approved).send().await
    }
}

/// Convenience method on any [`ContractReadProvider`] for binding a TRC721 instance.
pub trait Trc721Ext: ContractReadProvider + Clone + Sized {
    /// Bind to the TRC721 contract at `address`.
    fn trc721(&self, address: Address) -> Trc721Instance<Self> {
        Trc721Instance::new(self.clone(), address)
    }
}

impl<P: ContractReadProvider + Clone> Trc721Ext for P {}

#[cfg(test)]
mod tests {
    use tronz_primitives::Trx;
    use tronz_provider::{RootProvider, transport::mock::MockTransport};

    use super::*;

    #[test]
    fn data_safe_transfer_builder_exposes_transaction_configuration() {
        let token = Trc721Instance::new(RootProvider::new(MockTransport::new()), Address::ZERO)
            .caller(Address::ZERO);
        let limit = Trx::from_sun(20_000_000).unwrap();

        let _ = token
            .safe_transfer_from_with_data_call(
                Address::ZERO,
                Address::ZERO,
                U256::ZERO,
                Bytes::from_static(b"callback"),
            )
            .fee_limit(limit)
            .permission_id(2);
    }
}
