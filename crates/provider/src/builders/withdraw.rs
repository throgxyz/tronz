//! Withdraw-expire-unfreeze and cancel-all-unfreeze builders.

use tronz_primitives::Address;

use super::resolve_owner;
use crate::{
    error::Result,
    provider::{PendingTransaction, TronProvider},
    types::{
        CancelAllUnfreezeV2Contract, ContractType, TransactionRequest,
        WithdrawExpireUnfreezeContract,
    },
};

/// Claim TRX from expired unfreeze windows.
pub struct WithdrawExpireBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
}

impl<'a, P: TronProvider> WithdrawExpireBuilder<'a, P> {
    /// Start a new builder.
    pub fn new(provider: &'a P) -> Self {
        Self { provider, owner: None }
    }

    /// Override the account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let req = TransactionRequest {
            contract: Some(ContractType::WithdrawExpireUnfreeze(WithdrawExpireUnfreezeContract {
                owner_address: owner,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Cancel all in-progress unfreeze operations.
pub struct CancelAllUnfreezeBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
}

impl<'a, P: TronProvider> CancelAllUnfreezeBuilder<'a, P> {
    /// Start a new builder.
    pub fn new(provider: &'a P) -> Self {
        Self { provider, owner: None }
    }

    /// Override the account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let req = TransactionRequest {
            contract: Some(ContractType::CancelAllUnfreezeV2(CancelAllUnfreezeV2Contract {
                owner_address: owner,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
