//! Provider-bound [`Trc721Instance`] — high-level TRC721 contract interface.

use alloy_sol_types::SolCall as _;
use tronz_primitives::{Address, U256};
use tronz_provider::{ContractReadProvider, PendingTransaction, TronProvider};

use crate::{
    error::{ContractError, Result},
    instance::ContractInstance,
    trc721::{
        ITRC721, decode_address_return, decode_bool_return, decode_string_return,
        decode_uint256_return, encode_approve, encode_balance_of, encode_owner_of,
        encode_safe_transfer_from, encode_transfer_from,
    },
};

/// Errors returned by [`Trc721Instance`] methods — re-exported from [`ContractError`].
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

impl<P: ContractReadProvider> Trc721Instance<P> {
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

    /// Set the default caller (`msg.sender`) for read-only calls.
    pub fn caller(self, caller: Address) -> Self {
        Self { inner: self.inner.caller(caller) }
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Fetch the token name.
    pub async fn name(&self) -> Result<String, Trc721Error> {
        let out = self.inner.call_raw(ITRC721::nameCall {}.abi_encode().into()).call().await?;
        Ok(decode_string_return(&out)?)
    }

    /// Fetch the token symbol.
    pub async fn symbol(&self) -> Result<String, Trc721Error> {
        let out = self.inner.call_raw(ITRC721::symbolCall {}.abi_encode().into()).call().await?;
        Ok(decode_string_return(&out)?)
    }

    /// Fetch the metadata URI for `token_id`.
    pub async fn token_uri(&self, token_id: U256) -> Result<String, Trc721Error> {
        let out = self
            .inner
            .call_raw(ITRC721::tokenURICall { tokenId: token_id }.abi_encode().into())
            .call()
            .await?;
        Ok(decode_string_return(&out)?)
    }

    /// Fetch the number of tokens owned by `owner`.
    pub async fn balance_of(&self, owner: Address) -> Result<U256, Trc721Error> {
        let out = self.inner.call_raw(encode_balance_of(owner)).call().await?;
        Ok(decode_uint256_return(&out)?)
    }

    /// Fetch the owner of `token_id`.
    pub async fn owner_of(&self, token_id: U256) -> Result<Address, Trc721Error> {
        let out = self.inner.call_raw(encode_owner_of(token_id)).call().await?;
        Ok(decode_address_return(&out)?)
    }

    /// Fetch the approved address for `token_id`, if any.
    pub async fn get_approved(&self, token_id: U256) -> Result<Address, Trc721Error> {
        let out = self
            .inner
            .call_raw(ITRC721::getApprovedCall { tokenId: token_id }.abi_encode().into())
            .call()
            .await?;
        Ok(decode_address_return(&out)?)
    }

    /// Returns `true` if `operator` is approved to manage all of `owner`'s tokens.
    pub async fn is_approved_for_all(
        &self,
        owner: Address,
        operator: Address,
    ) -> Result<bool, Trc721Error> {
        let out = self
            .inner
            .call_raw(
                ITRC721::isApprovedForAllCall { owner: owner.into(), operator: operator.into() }
                    .abi_encode()
                    .into(),
            )
            .call()
            .await?;
        Ok(decode_bool_return(&out)?)
    }
}

impl<P: TronProvider> Trc721Instance<P> {
    // ── writes ────────────────────────────────────────────────────────────────

    /// Transfer `token_id` from `from` to `to`.
    pub async fn transfer_from(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<PendingTransaction<P>, Trc721Error> {
        self.inner.call_raw(encode_transfer_from(from, to, token_id)).send().await
    }

    /// Safe-transfer `token_id` from `from` to `to` (calls `onERC721Received` on the recipient).
    pub async fn safe_transfer_from(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<PendingTransaction<P>, Trc721Error> {
        self.inner.call_raw(encode_safe_transfer_from(from, to, token_id)).send().await
    }

    /// Approve `to` to transfer `token_id`.
    pub async fn approve(
        &self,
        to: Address,
        token_id: U256,
    ) -> Result<PendingTransaction<P>, Trc721Error> {
        self.inner.call_raw(encode_approve(to, token_id)).send().await
    }

    /// Approve or revoke `operator` to manage all of the signer's tokens.
    pub async fn set_approval_for_all(
        &self,
        operator: Address,
        approved: bool,
    ) -> Result<PendingTransaction<P>, Trc721Error> {
        self.inner
            .call_raw(
                ITRC721::setApprovalForAllCall { operator: operator.into(), approved }
                    .abi_encode()
                    .into(),
            )
            .send()
            .await
    }
}

// ── Extension trait ───────────────────────────────────────────────────────────

/// Convenience method on any [`ContractReadProvider`] for binding a TRC721 instance.
pub trait Trc721Ext: ContractReadProvider + Sized {
    /// Bind to the TRC721 contract at `address`.
    fn trc721(&self, address: Address) -> Trc721Instance<Self> {
        Trc721Instance::new(self.clone(), address)
    }
}

impl<P: ContractReadProvider> Trc721Ext for P {}
