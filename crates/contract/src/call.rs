//! [`CallBuilder`] — a lazy contract call that can be simulated or broadcast.

use tronz_primitives::{Address, Bytes, Trx};
use tronz_provider::{
    Error as ProviderError, PendingTransaction, TronProvider,
    transport::TronTransport as _,
    types::{ContractType, TransactionRequest, TriggerSmartContract},
};

use crate::error::{ContractError, Result};

/// A builder for interacting with a TRON smart contract.
///
/// Created by [`ContractInstance::call_raw`], [`ContractInstance::function`], or
/// [`ContractInstance::function_from_selector`].
///
/// Optionally attach TRX or TRC10 tokens before executing:
///
/// ```ignore
/// // Read-only simulation (trigger_constant_contract)
/// let output = contract.call_raw(calldata).call().await?;
///
/// // State-changing broadcast (trigger_smart_contract → sign → broadcast)
/// let pending = contract.call_raw(calldata).send().await?;
///
/// // Payable call — send 1 TRX alongside
/// let pending = contract
///     .call_raw(calldata)
///     .value(Trx::from_sun_unchecked(1_000_000))
///     .send()
///     .await?;
/// ```
///
/// [`ContractInstance::call_raw`]: crate::instance::ContractInstance::call_raw
/// [`ContractInstance::function`]: crate::instance::ContractInstance::function
/// [`ContractInstance::function_from_selector`]: crate::instance::ContractInstance::function_from_selector
pub struct CallBuilder<P> {
    provider: P,
    address: Address,
    data: Bytes,
    call_value: Trx,
    call_token_value: Trx,
    token_id: i64,
}

impl<P: TronProvider> CallBuilder<P> {
    pub(crate) fn new(provider: P, address: Address, data: Bytes) -> Self {
        Self {
            provider,
            address,
            data,
            call_value: Trx::ZERO,
            call_token_value: Trx::ZERO,
            token_id: 0,
        }
    }

    /// Attach a TRX amount to the call (for payable functions).
    #[inline]
    pub fn value(mut self, trx: Trx) -> Self {
        self.call_value = trx;
        self
    }

    /// Attach a TRC10 token to the call.
    #[inline]
    pub fn token(mut self, token_id: i64, value: Trx) -> Self {
        self.token_id = token_id;
        self.call_token_value = value;
        self
    }

    /// Estimate the energy this call would consume (`estimate_energy`).
    ///
    /// Mirrors [`estimate_gas`] in alloy: no state change, no signer required.
    /// Use the result to set `fee_limit` before calling [`send`].
    ///
    /// [`estimate_gas`]: https://alloy.rs
    /// [`send`]: CallBuilder::send
    pub async fn estimate_energy(&self) -> Result<i64> {
        let caller = self.provider.signer_address().unwrap_or(self.address);
        let params = TriggerSmartContract {
            owner_address: caller,
            contract_address: self.address,
            call_value: self.call_value,
            data: self.data.clone(),
            call_token_value: self.call_token_value,
            token_id: self.token_id,
        };
        self.provider
            .estimate_energy(params)
            .await
            .map_err(ContractError::Provider)
    }

    /// Execute as a **constant call** (`trigger_constant_contract`).
    ///
    /// No state change, no energy consumed, no signer required.
    /// Returns the raw ABI-encoded output bytes.
    pub async fn call(self) -> Result<Bytes> {
        let caller = self.provider.signer_address().unwrap_or(self.address);
        let params = TriggerSmartContract {
            owner_address: caller,
            contract_address: self.address,
            call_value: self.call_value,
            data: self.data,
            call_token_value: self.call_token_value,
            token_id: self.token_id,
        };
        let result = self
            .provider
            .transport()
            .trigger_constant_contract(params)
            .await
            .map_err(|e| ContractError::Provider(ProviderError::Transport(e.into())))?;
        if result.revert_reason.is_some() {
            return Err(ContractError::Revert(result.output.into()));
        }
        Ok(result.output.into())
    }

    /// Execute as a **state-changing call** (`trigger_smart_contract`).
    ///
    /// Requires a signer to be attached to the provider. The transaction is
    /// filled (TAPOS, fee-limit), signed, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let caller = self
            .provider
            .signer_address()
            .ok_or_else(ProviderError::no_signer)
            .map_err(ContractError::Provider)?;
        let req = TransactionRequest::default().with_contract(ContractType::TriggerSmartContract(
            TriggerSmartContract {
                owner_address: caller,
                contract_address: self.address,
                call_value: self.call_value,
                data: self.data,
                call_token_value: self.call_token_value,
                token_id: self.token_id,
            },
        ));
        Ok(self.provider.send_transaction(req).await?)
    }
}
