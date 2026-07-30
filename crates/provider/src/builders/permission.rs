//! Account permission (multisig) update builder.

use tronz_primitives::Address;

use super::{builder_exits, resolve_owner};
use crate::{
    error::{Error, Result},
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
        check_operations(self.owner_permission.as_ref(), self.witness.as_ref(), &self.actives)?;
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

fn check_operations(
    owner: Option<&Permission>,
    witness: Option<&Permission>,
    actives: &[Permission],
) -> Result<()> {
    for (role, permission) in [("owner", owner), ("witness", witness)] {
        if let Some(permission) = permission
            && !permission.operations.is_empty()
        {
            return Err(Error::local_usage_str(&format!(
                "the {role} permission authorizes by role and cannot list operations"
            )));
        }
    }

    if let Some(permission) = actives.iter().find(|p| p.operations.is_empty()) {
        return Err(Error::local_usage_str(&format!(
            "active permission {} grants no operations: list the contract types its keys \
             may authorize, since granting every type would include rewriting the \
             permissions themselves",
            permission.id
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        provider::RootProvider,
        transport::mock::MockTransport,
        types::{ContractKind, OperationSet, PermissionKey},
    };

    fn addr(b: u8) -> Address {
        Address::from_evm_bytes({
            let mut a = [0u8; 20];
            a[19] = b;
            a
        })
    }

    fn permission(id: i32, operations: impl IntoIterator<Item = ContractKind>) -> Permission {
        Permission {
            id,
            permission_name: format!("p{id}"),
            threshold: 1,
            keys: vec![PermissionKey { address: addr(2), weight: 1 }],
            operations: OperationSet::try_from_iter(operations).unwrap(),
        }
    }

    #[test]
    fn an_active_permission_has_to_say_what_it_grants() {
        let provider = RootProvider::new(MockTransport::new());
        let err = provider
            .update_permissions()
            .from(addr(1))
            .actives(vec![permission(2, [])])
            .into_request()
            .unwrap_err();

        assert!(err.is_local_usage_error());
        assert!(err.to_string().contains("grants no operations"), "{err}");
    }

    #[test]
    fn operations_on_an_owner_permission_are_refused_rather_than_dropped() {
        let provider = RootProvider::new(MockTransport::new());
        let err = provider
            .update_permissions()
            .from(addr(1))
            .owner_permission(permission(0, [ContractKind::Transfer]))
            .into_request()
            .unwrap_err();

        assert!(err.is_local_usage_error());
        assert!(err.to_string().contains("owner permission"), "{err}");
    }

    #[test]
    fn a_grant_that_names_its_operations_goes_through() {
        let provider = RootProvider::new(MockTransport::new());
        let request = provider
            .update_permissions()
            .from(addr(1))
            .owner_permission(permission(0, []))
            .actives(vec![permission(2, [ContractKind::Transfer])])
            .into_request()
            .unwrap();

        assert!(matches!(request.contract, Some(ContractType::AccountPermissionUpdate(_))));
    }
}
