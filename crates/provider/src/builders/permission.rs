//! Account permission (multisig) update builder.

use tronz_primitives::Address;

use super::{builder_exits, resolve_owner};
use crate::{
    error::Result,
    provider::{PendingTransaction, TronProvider},
    types::{AccountPermissionUpdateContract, ContractType, Permission, TransactionRequest},
};

/// Update an account's owner/witness/active permissions (multisig).
#[derive(Debug)]
pub struct AccountPermissionUpdateBuilder<'a, P> {
    provider: &'a P,
    permission_id: Option<i32>,
    owner: Option<Address>,
    owner_permission: Option<Permission>,
    witness: Option<Permission>,
    actives: Vec<Permission>,
}

impl<'a, P: TronProvider> AccountPermissionUpdateBuilder<'a, P> {
    /// Start a new builder.
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            permission_id: None,
            owner: None,
            owner_permission: None,
            witness: None,
            actives: Vec::new(),
        }
    }

    /// Override the account being updated.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the new owner permission.
    pub fn owner_permission(mut self, permission: Permission) -> Self {
        self.owner_permission = Some(permission);
        self
    }

    /// Set the new witness permission.
    pub fn witness(mut self, permission: Permission) -> Self {
        self.witness = Some(permission);
        self
    }

    /// Set the new active permissions.
    pub fn actives(mut self, actives: Vec<Permission>) -> Self {
        self.actives = actives;
        self
    }

    /// The request this builder describes, without contacting the node.
    pub fn into_request(self) -> Result<TransactionRequest> {
        let owner = resolve_owner(self.owner, self.provider)?;
        Ok(TransactionRequest {
            contract: Some(ContractType::AccountPermissionUpdate(
                AccountPermissionUpdateContract {
                    owner_address: owner,
                    owner: self.owner_permission,
                    witness: self.witness,
                    actives: self.actives,
                },
            )),
            permission_id: self.permission_id,
            ..Default::default()
        })
    }

    builder_exits!();
}
