//! Withdraw-expire-unfreeze and cancel-all-unfreeze builders.

use tronz_primitives::Address;

use super::{builder_exits, resolve_owner};
use crate::{
    error::Result,
    provider::{PendingTransaction, TronProvider},
    types::{
        CancelAllUnfreezeV2Contract, ContractType, TransactionRequest,
        WithdrawExpireUnfreezeContract,
    },
};

/// Claim TRX from expired unfreeze windows.
#[derive(Debug)]
pub struct WithdrawExpireBuilder<'a, P> {
    provider: &'a P,
    permission_id: Option<i32>,
    owner: Option<Address>,
}

impl<'a, P: TronProvider> WithdrawExpireBuilder<'a, P> {
    /// Start a new builder.
    pub fn new(provider: &'a P) -> Self {
        Self { provider, permission_id: None, owner: None }
    }

    /// Override the account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// The request this builder describes, without contacting the node.
    pub fn into_request(self) -> Result<TransactionRequest> {
        let owner = resolve_owner(self.owner, self.provider)?;
        Ok(TransactionRequest {
            contract: Some(ContractType::WithdrawExpireUnfreeze(WithdrawExpireUnfreezeContract {
                owner_address: owner,
            })),
            permission_id: self.permission_id,
            ..Default::default()
        })
    }

    builder_exits!();
}

/// Cancel all in-progress unfreeze operations.
#[derive(Debug)]
pub struct CancelAllUnfreezeBuilder<'a, P> {
    provider: &'a P,
    permission_id: Option<i32>,
    owner: Option<Address>,
}

impl<'a, P: TronProvider> CancelAllUnfreezeBuilder<'a, P> {
    /// Start a new builder.
    pub fn new(provider: &'a P) -> Self {
        Self { provider, permission_id: None, owner: None }
    }

    /// Override the account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// The request this builder describes, without contacting the node.
    pub fn into_request(self) -> Result<TransactionRequest> {
        let owner = resolve_owner(self.owner, self.provider)?;
        Ok(TransactionRequest {
            contract: Some(ContractType::CancelAllUnfreezeV2(CancelAllUnfreezeV2Contract {
                owner_address: owner,
            })),
            permission_id: self.permission_id,
            ..Default::default()
        })
    }

    builder_exits!();
}
