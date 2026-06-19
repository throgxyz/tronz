//! Account management builders: create and rename accounts.

use tronz_primitives::Address;

use super::resolve_owner;
use crate::{
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    types::{ContractType, CreateAccountContract, TransactionRequest, UpdateAccountContract},
};

/// Builds an account-activation transaction.
///
/// On TRON, addresses that have never received TRX do not exist on-chain.
/// This transaction creates the account in one step.
///
/// Created by [`TronProvider::create_account`].
pub struct CreateAccountBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    account_address: Option<Address>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> CreateAccountBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            account_address: None,
            memo: None,
        }
    }

    /// Override the payer address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the address to activate.
    pub fn account_address(mut self, address: Address) -> Self {
        self.account_address = Some(address);
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
        let account_address = self
            .account_address
            .ok_or(Error::missing_field("account_address"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::CreateAccount(CreateAccountContract {
                owner_address: owner,
                account_address,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Builds an account-name-update transaction.
///
/// Account names on TRON are not unique and can be changed freely.
///
/// Created by [`TronProvider::update_account_name`].
pub struct UpdateAccountBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    name: Option<String>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> UpdateAccountBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            name: None,
            memo: None,
        }
    }

    /// Override the account address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the new account name (UTF-8).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
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
        let name = self.name.ok_or(Error::missing_field("name"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UpdateAccount(UpdateAccountContract {
                owner_address: owner,
                name,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
