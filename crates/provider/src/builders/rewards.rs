//! Claim block/vote rewards builder.

use tronz_primitives::Address;

use super::{builder_exits, resolve_owner};
use crate::{
    error::Result,
    provider::{PendingTransaction, TronProvider},
    types::{ContractType, TransactionRequest, WithdrawBalanceContract},
};

/// Claim accumulated block/vote rewards (`WithdrawBalance`).
///
/// Note: TRON allows this at most once per 24h per account.
#[derive(Debug)]
pub struct WithdrawBalanceBuilder<'a, P> {
    provider: &'a P,
    permission_id: Option<i32>,
    owner: Option<Address>,
}

impl<'a, P: TronProvider> WithdrawBalanceBuilder<'a, P> {
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
            contract: Some(ContractType::WithdrawBalance(WithdrawBalanceContract {
                owner_address: owner,
            })),
            permission_id: self.permission_id,
            ..Default::default()
        })
    }

    builder_exits!();
}
