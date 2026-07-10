//! Account permission (multisig) update builder.

use tronz_primitives::Address;

use super::resolve_owner;
use crate::{
    error::Result,
    provider::{PendingTransaction, TronProvider},
    types::{AccountPermissionUpdateContract, ContractType, Permission, TransactionRequest},
};

/// Update an account's owner/witness/active permissions (multisig).
pub struct AccountPermissionUpdateBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    owner_permission: Option<Permission>,
    witness: Option<Permission>,
    actives: Vec<Permission>,
}

impl<'a, P: TronProvider> AccountPermissionUpdateBuilder<'a, P> {
    /// Start a new builder.
    pub fn new(provider: &'a P) -> Self {
        Self { provider, owner: None, owner_permission: None, witness: None, actives: Vec::new() }
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

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let req = TransactionRequest {
            contract: Some(ContractType::AccountPermissionUpdate(
                AccountPermissionUpdateContract {
                    owner_address: owner,
                    owner: self.owner_permission,
                    witness: self.witness,
                    actives: self.actives,
                },
            )),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
