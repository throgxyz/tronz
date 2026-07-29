//! Provider-bound TRC20 contract instance.

use alloy_sol_types::SolCall;
use tronz_primitives::{Address, U256};
use tronz_provider::{ContractReadProvider, PendingTransaction, TronProvider};

use crate::{
    error::{ContractError, Result},
    instance::ContractInstance,
    sol_call::TronCallBuilder,
    trc20::ITRC20,
};

/// Errors returned by [`Trc20Instance`] methods.
pub type Trc20Error = ContractError;

/// A provider-bound handle to a TRC20 contract.
///
/// Construct via [`Trc20Ext::trc20`] on any provider:
///
/// ```no_run
/// # use tronz_contract::trc20::Trc20Ext;
/// # use tronz_primitives::Address;
/// # async fn run(provider: impl tronz_provider::ContractReadProvider + Clone) {
/// let contract: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap();
/// let caller: Address = "TXYJg94nXn8jDVVK4yg4B8yXWNR1pQxv6f".parse().unwrap();
/// let token = provider.trc20(contract).caller(caller);
/// let name = token.name().await.unwrap();
/// # }
/// ```
///
/// Internally wraps a [`ContractInstance`] and encodes all calldata using the
/// statically generated [`sol!`](alloy_sol_macro::sol) types — no JSON ABI required.
#[derive(Clone)]
pub struct Trc20Instance<P: ContractReadProvider> {
    inner: ContractInstance<P>,
}

impl<P: ContractReadProvider + Clone> Trc20Instance<P> {
    /// Bind to the TRC20 contract at `address`.
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
    pub fn name_call(&self) -> TronCallBuilder<P, ITRC20::nameCall> {
        self.typed_call(&ITRC20::nameCall {})
    }

    /// Fetch the token name (e.g. `"Tether USD"`).
    pub async fn name(&self) -> Result<String, Trc20Error> {
        self.name_call().call().await
    }

    /// Build a typed `symbol()` call.
    pub fn symbol_call(&self) -> TronCallBuilder<P, ITRC20::symbolCall> {
        self.typed_call(&ITRC20::symbolCall {})
    }

    /// Fetch the token symbol (e.g. `"USDT"`).
    pub async fn symbol(&self) -> Result<String, Trc20Error> {
        self.symbol_call().call().await
    }

    /// Build a typed `decimals()` call.
    pub fn decimals_call(&self) -> TronCallBuilder<P, ITRC20::decimalsCall> {
        self.typed_call(&ITRC20::decimalsCall {})
    }

    /// Fetch the number of decimal places.
    pub async fn decimals(&self) -> Result<u8, Trc20Error> {
        self.decimals_call().call().await
    }

    /// Build a typed `totalSupply()` call.
    pub fn total_supply_call(&self) -> TronCallBuilder<P, ITRC20::totalSupplyCall> {
        self.typed_call(&ITRC20::totalSupplyCall {})
    }

    /// Fetch the total token supply.
    pub async fn total_supply(&self) -> Result<U256, Trc20Error> {
        self.total_supply_call().call().await
    }

    /// Build a typed `balanceOf(account)` call.
    pub fn balance_of_call(&self, account: Address) -> TronCallBuilder<P, ITRC20::balanceOfCall> {
        self.typed_call(&ITRC20::balanceOfCall { account: account.into() })
    }

    /// Fetch the token balance of `account`.
    pub async fn balance_of(&self, account: Address) -> Result<U256, Trc20Error> {
        self.balance_of_call(account).call().await
    }

    /// Build a typed `allowance(owner, spender)` call.
    pub fn allowance_call(
        &self,
        owner: Address,
        spender: Address,
    ) -> TronCallBuilder<P, ITRC20::allowanceCall> {
        self.typed_call(&ITRC20::allowanceCall { owner: owner.into(), spender: spender.into() })
    }

    /// Fetch the remaining allowance that `spender` may transfer on behalf of `owner`.
    pub async fn allowance(&self, owner: Address, spender: Address) -> Result<U256, Trc20Error> {
        self.allowance_call(owner, spender).call().await
    }

    /// Build a typed `transfer(to, amount)` call.
    pub fn transfer_call(
        &self,
        to: Address,
        amount: U256,
    ) -> TronCallBuilder<P, ITRC20::transferCall> {
        self.typed_call(&ITRC20::transferCall { to: to.into(), amount })
    }

    /// Build a typed `approve(spender, amount)` call.
    pub fn approve_call(
        &self,
        spender: Address,
        amount: U256,
    ) -> TronCallBuilder<P, ITRC20::approveCall> {
        self.typed_call(&ITRC20::approveCall { spender: spender.into(), amount })
    }

    /// Build a typed `transferFrom(from, to, amount)` call.
    pub fn transfer_from_call(
        &self,
        from: Address,
        to: Address,
        amount: U256,
    ) -> TronCallBuilder<P, ITRC20::transferFromCall> {
        self.typed_call(&ITRC20::transferFromCall { from: from.into(), to: to.into(), amount })
    }
}

impl<P: TronProvider + Clone> Trc20Instance<P> {
    /// Transfer `amount` tokens from the signer's account to `to`.
    pub async fn transfer(
        &self,
        to: Address,
        amount: U256,
    ) -> Result<PendingTransaction, Trc20Error> {
        self.transfer_call(to, amount).send().await
    }

    /// Approve `spender` to transfer up to `amount` on the signer's behalf.
    pub async fn approve(
        &self,
        spender: Address,
        amount: U256,
    ) -> Result<PendingTransaction, Trc20Error> {
        self.approve_call(spender, amount).send().await
    }

    /// Transfer `amount` tokens from `from` to `to`, using the signer's allowance.
    pub async fn transfer_from(
        &self,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<PendingTransaction, Trc20Error> {
        self.transfer_from_call(from, to, amount).send().await
    }
}

/// Convenience method on any [`ContractReadProvider`] for binding a TRC20 instance.
pub trait Trc20Ext: ContractReadProvider + Clone + Sized {
    /// Bind to the TRC20 contract at `address`.
    fn trc20(&self, address: Address) -> Trc20Instance<Self> {
        Trc20Instance::new(self.clone(), address)
    }
}

impl<P: ContractReadProvider + Clone> Trc20Ext for P {}

#[cfg(test)]
mod tests {
    use tronz_primitives::Trx;
    use tronz_provider::{RootProvider, transport::mock::MockTransport};

    use super::*;

    #[test]
    fn write_builder_exposes_transaction_configuration() {
        let token = Trc20Instance::new(RootProvider::new(MockTransport::new()), Address::ZERO)
            .caller(Address::ZERO);
        let limit = Trx::from_sun(20_000_000).unwrap();

        let _ = token.transfer_call(Address::ZERO, U256::ZERO).fee_limit(limit).permission_id(2);
    }
}
