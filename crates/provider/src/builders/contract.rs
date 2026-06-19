//! Smart-contract management builders.

use tronz_primitives::Address;

use super::resolve_owner;
use crate::{
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    types::{
        ClearContractAbiContract, ContractType, SetAccountIdContract, TransactionRequest,
        UpdateEnergyLimitContract, UpdateSettingContract,
    },
};

/// Builds a set-account-id transaction.
///
/// Assigns a unique short alphanumeric alias to an account. This can only be
/// done once per account.
///
/// Created by [`TronProvider::set_account_id`].
pub struct SetAccountIdBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    account_id: Option<String>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> SetAccountIdBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            account_id: None,
            memo: None,
        }
    }

    /// Override the account address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the account ID string (required).
    pub fn account_id(mut self, id: impl Into<String>) -> Self {
        self.account_id = Some(id.into());
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let account_id = self.account_id.ok_or(Error::missing_field("account_id"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::SetAccountId(SetAccountIdContract {
                owner_address: owner,
                account_id,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Builds a clear-contract-ABI transaction.
///
/// Removes the on-chain ABI for a deployed smart contract. Only the contract
/// owner can call this.
///
/// Created by [`TronProvider::clear_contract_abi`].
pub struct ClearContractAbiBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    contract_address: Option<Address>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> ClearContractAbiBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            contract_address: None,
            memo: None,
        }
    }

    /// Override the contract owner address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the contract whose ABI should be cleared (required).
    pub fn contract_address(mut self, address: Address) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let contract_address = self
            .contract_address
            .ok_or(Error::missing_field("contract_address"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ClearContractAbi(ClearContractAbiContract {
                owner_address: owner,
                contract_address,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Builds an update-setting transaction.
///
/// Changes the percentage of energy that callers pay versus the contract
/// origin. Only the contract owner can call this.
///
/// Created by [`TronProvider::update_contract_setting`].
pub struct UpdateContractSettingBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    contract_address: Option<Address>,
    consume_user_resource_percent: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> UpdateContractSettingBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            contract_address: None,
            consume_user_resource_percent: None,
            memo: None,
        }
    }

    /// Override the contract owner address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the contract to update (required).
    pub fn contract_address(mut self, address: Address) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Set the new caller-energy percentage (0–100, required).
    pub fn consume_user_resource_percent(mut self, percent: i64) -> Self {
        self.consume_user_resource_percent = Some(percent);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let contract_address = self
            .contract_address
            .ok_or(Error::missing_field("contract_address"))?;
        let consume_user_resource_percent = self
            .consume_user_resource_percent
            .ok_or(Error::missing_field("consume_user_resource_percent"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UpdateSetting(UpdateSettingContract {
                owner_address: owner,
                contract_address,
                consume_user_resource_percent,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Builds an update-energy-limit transaction.
///
/// Changes the per-call energy cap charged to the contract origin. Only the
/// contract owner can call this.
///
/// Created by [`TronProvider::update_contract_energy_limit`].
pub struct UpdateContractEnergyLimitBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    contract_address: Option<Address>,
    origin_energy_limit: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> UpdateContractEnergyLimitBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            contract_address: None,
            origin_energy_limit: None,
            memo: None,
        }
    }

    /// Override the contract owner address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the contract to update (required).
    pub fn contract_address(mut self, address: Address) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Set the new per-call origin energy limit (required).
    pub fn origin_energy_limit(mut self, limit: i64) -> Self {
        self.origin_energy_limit = Some(limit);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let contract_address = self
            .contract_address
            .ok_or(Error::missing_field("contract_address"))?;
        let origin_energy_limit = self
            .origin_energy_limit
            .ok_or(Error::missing_field("origin_energy_limit"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UpdateEnergyLimit(UpdateEnergyLimitContract {
                owner_address: owner,
                contract_address,
                origin_energy_limit,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
