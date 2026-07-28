//! Account management builders: create and rename accounts.

use tronz_primitives::{Address, Bytes};

use super::{builder_exits, resolve_owner};
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
#[derive(Debug)]
pub struct CreateAccountBuilder<'a, P> {
    provider: &'a P,
    permission_id: Option<i32>,
    owner: Option<Address>,
    account_address: Option<Address>,
    memo: Option<Bytes>,
}

impl<'a, P: TronProvider> CreateAccountBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self { provider, permission_id: None, owner: None, account_address: None, memo: None }
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
    pub fn memo(mut self, memo: impl Into<Bytes>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// The request this builder describes, without contacting the node.
    pub fn into_request(self) -> Result<TransactionRequest> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let account_address =
            self.account_address.ok_or(Error::missing_field("account_address"))?;

        Ok(TransactionRequest {
            contract: Some(ContractType::CreateAccount(CreateAccountContract {
                owner_address: owner,
                account_address,
            })),
            memo: self.memo,
            permission_id: self.permission_id,
            ..Default::default()
        })
    }

    builder_exits!();
}

/// Builds an account-name-update transaction.
///
/// Account names on TRON are not unique and can be changed freely.
///
/// Created by [`TronProvider::update_account_name`].
#[derive(Debug)]
pub struct UpdateAccountBuilder<'a, P> {
    provider: &'a P,
    permission_id: Option<i32>,
    owner: Option<Address>,
    name: Option<String>,
    memo: Option<Bytes>,
}

impl<'a, P: TronProvider> UpdateAccountBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self { provider, permission_id: None, owner: None, name: None, memo: None }
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
    pub fn memo(mut self, memo: impl Into<Bytes>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// The request this builder describes, without contacting the node.
    pub fn into_request(self) -> Result<TransactionRequest> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let name = self.name.ok_or(Error::missing_field("name"))?;

        Ok(TransactionRequest {
            contract: Some(ContractType::UpdateAccount(UpdateAccountContract {
                owner_address: owner,
                name,
            })),
            memo: self.memo,
            permission_id: self.permission_id,
            ..Default::default()
        })
    }

    builder_exits!();
}
