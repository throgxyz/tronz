//! Provider-bound [`Trc20Instance`] — high-level TRC20 contract interface.

use alloy_sol_types::SolCall as _;
use tronz_primitives::{Address, U256};
use tronz_provider::{PendingTransaction, TronProvider};

use crate::{
    error::{ContractError, Result},
    instance::ContractInstance,
    trc20::{
        ITRC20, decode_decimals_return, decode_string_return, decode_uint256_return,
        encode_allowance, encode_approve, encode_balance_of, encode_transfer, encode_transfer_from,
    },
};

/// Errors returned by [`Trc20Instance`] methods — re-exported from [`ContractError`].
pub type Trc20Error = ContractError;

/// A provider-bound handle to a TRC20 contract.
///
/// Construct via [`Trc20Ext::trc20`] on any provider:
///
/// ```no_run
/// # use tronz_contract::trc20::Trc20Ext;
/// # use tronz_primitives::Address;
/// # async fn run(provider: impl tronz_provider::TronProvider + Clone) {
/// let contract: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap();
/// let token = provider.trc20(contract);
/// let name = token.name().await.unwrap();
/// # }
/// ```
///
/// Internally wraps a [`ContractInstance`] and encodes all calldata using the
/// statically generated [`sol!`](alloy_sol_macro::sol) types — no JSON ABI required.
#[derive(Clone)]
pub struct Trc20Instance<P: TronProvider> {
    inner: ContractInstance<P>,
}

impl<P: TronProvider> Trc20Instance<P> {
    /// Bind to the TRC20 contract at `address`.
    pub fn new(provider: P, address: Address) -> Self {
        Self {
            inner: ContractInstance::new_raw(provider, address),
        }
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
        Self {
            inner: self.inner.at(address),
        }
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Fetch the token name (e.g. `"Tether USD"`).
    pub async fn name(&self) -> Result<String, Trc20Error> {
        let out = self
            .inner
            .call_raw(ITRC20::nameCall {}.abi_encode().into())
            .call()
            .await?;
        Ok(decode_string_return(&out)?)
    }

    /// Fetch the token symbol (e.g. `"USDT"`).
    pub async fn symbol(&self) -> Result<String, Trc20Error> {
        let out = self
            .inner
            .call_raw(ITRC20::symbolCall {}.abi_encode().into())
            .call()
            .await?;
        Ok(decode_string_return(&out)?)
    }

    /// Fetch the number of decimal places.
    pub async fn decimals(&self) -> Result<u8, Trc20Error> {
        let out = self
            .inner
            .call_raw(ITRC20::decimalsCall {}.abi_encode().into())
            .call()
            .await?;
        Ok(decode_decimals_return(&out)?)
    }

    /// Fetch the total token supply.
    pub async fn total_supply(&self) -> Result<U256, Trc20Error> {
        let out = self
            .inner
            .call_raw(ITRC20::totalSupplyCall {}.abi_encode().into())
            .call()
            .await?;
        Ok(decode_uint256_return(&out)?)
    }

    /// Fetch the token balance of `account`.
    pub async fn balance_of(&self, account: Address) -> Result<U256, Trc20Error> {
        let out = self
            .inner
            .call_raw(encode_balance_of(account))
            .call()
            .await?;
        Ok(decode_uint256_return(&out)?)
    }

    /// Fetch the remaining allowance that `spender` may transfer on behalf of `owner`.
    pub async fn allowance(&self, owner: Address, spender: Address) -> Result<U256, Trc20Error> {
        let out = self
            .inner
            .call_raw(encode_allowance(owner, spender))
            .call()
            .await?;
        Ok(decode_uint256_return(&out)?)
    }

    // ── writes ────────────────────────────────────────────────────────────────

    /// Transfer `amount` tokens from the signer's account to `to`.
    pub async fn transfer(
        &self,
        to: Address,
        amount: U256,
    ) -> Result<PendingTransaction<P>, Trc20Error> {
        self.inner
            .call_raw(encode_transfer(to, amount))
            .send()
            .await
    }

    /// Approve `spender` to transfer up to `amount` on the signer's behalf.
    pub async fn approve(
        &self,
        spender: Address,
        amount: U256,
    ) -> Result<PendingTransaction<P>, Trc20Error> {
        self.inner
            .call_raw(encode_approve(spender, amount))
            .send()
            .await
    }

    /// Transfer `amount` tokens from `from` to `to`, using the signer's allowance.
    pub async fn transfer_from(
        &self,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<PendingTransaction<P>, Trc20Error> {
        self.inner
            .call_raw(encode_transfer_from(from, to, amount))
            .send()
            .await
    }
}

// ── Extension trait ───────────────────────────────────────────────────────────

/// Convenience method on any [`TronProvider`] for binding a TRC20 instance.
pub trait Trc20Ext: TronProvider + Sized {
    /// Bind to the TRC20 contract at `address`.
    fn trc20(&self, address: Address) -> Trc20Instance<Self> {
        Trc20Instance::new(self.clone(), address)
    }
}

impl<P: TronProvider> Trc20Ext for P {}
