//! Account and resource types.

use std::collections::HashMap;

use tronz_primitives::{Address, ResourceCode, Trx};

use crate::types::contract::{ContractKind, Permission};

/// On-chain account state.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct AccountInfo {
    /// Account address.
    pub address: Address,
    /// TRX balance.
    pub balance: Trx,
    /// Account name.
    pub name: String,
    /// Whether the account is activated on-chain (vs. just a key).
    pub is_activated: bool,
    /// Stake 2.0 frozen balances.
    pub frozen_v2: Vec<FreezeV2>,
    /// Stake 2.0 in-progress unfreezes.
    pub unfrozen_v2: Vec<UnfreezeV2>,
    /// Witness votes cast by this account.
    pub votes: Vec<Vote>,
    /// Multisig permissions.
    pub permissions: AccountPermissions,
    /// TRC10 token balances: token ID → raw amount (apply `decimals` for display).
    pub trc10_balances: HashMap<String, i64>,
}

/// A Stake 2.0 frozen-balance entry.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct FreezeV2 {
    /// Resource the stake provides.
    pub resource: ResourceCode,
    /// Staked amount.
    pub amount: Trx,
}

/// A Stake 2.0 in-progress unfreeze entry.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct UnfreezeV2 {
    /// Resource being released.
    pub resource: ResourceCode,
    /// Amount being released.
    pub amount: Trx,
    /// When the funds become withdrawable (unix ms).
    pub expire_time_ms: i64,
}

/// A witness vote.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Vote {
    /// Witness (super representative) address.
    pub vote_address: Address,
    /// Number of votes cast.
    pub vote_count: i64,
}

/// The set of permissions on an account.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct AccountPermissions {
    /// Owner permission.
    pub owner: Option<Permission>,
    /// Witness permission (super representatives only).
    pub witness: Option<Permission>,
    /// Active permissions.
    pub actives: Vec<Permission>,
}

impl AccountPermissions {
    fn locate(&self, id: i32) -> Option<PermissionRef<'_>> {
        match id {
            0 => self.owner.as_ref().map(PermissionRef::Owner),
            1 => self.witness.as_ref().map(PermissionRef::Witness),
            _ => self
                .actives
                .iter()
                .find(|permission| permission.id == id)
                .map(PermissionRef::Active),
        }
    }

    /// Returns the permission for a transaction permission id.
    pub fn by_id(&self, id: i32) -> Option<&Permission> {
        match self.locate(id)? {
            PermissionRef::Owner(permission)
            | PermissionRef::Witness(permission)
            | PermissionRef::Active(permission) => Some(permission),
        }
    }

    /// Returns whether a permission authorizes a native contract operation.
    ///
    /// Missing and witness permissions return `false`.
    pub fn allows(&self, permission_id: i32, kind: ContractKind) -> bool {
        match self.locate(permission_id) {
            Some(PermissionRef::Owner(_)) => true,
            Some(PermissionRef::Witness(_)) | None => false,
            Some(PermissionRef::Active(permission)) => permission.operations.contains(kind),
        }
    }
}

enum PermissionRef<'a> {
    Owner(&'a Permission),
    Witness(&'a Permission),
    Active(&'a Permission),
}

/// Bandwidth + energy usage/limits and delegation totals for an account.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct AccountResource {
    /// Free bandwidth consumed.
    pub free_bandwidth_used: i64,
    /// Free bandwidth limit.
    pub free_bandwidth_limit: i64,
    /// Staked bandwidth consumed.
    pub bandwidth_used: i64,
    /// Staked bandwidth limit.
    pub bandwidth_limit: i64,
    /// Energy consumed.
    pub energy_used: i64,
    /// Energy limit.
    pub energy_limit: i64,
    /// Bandwidth delegated out to others.
    pub delegated_bandwidth_for_others: Trx,
    /// Energy delegated out to others.
    pub delegated_energy_for_others: Trx,
    /// Bandwidth received via delegation.
    pub received_bandwidth: Trx,
    /// Energy received via delegation.
    pub received_energy: Trx,
    /// TRON Power (voting weight) used.
    pub tron_power_used: Trx,
    /// TRON Power limit.
    pub tron_power_limit: Trx,
}

/// A single delegation relationship between two accounts.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DelegatedResource {
    /// Delegator.
    pub from: Address,
    /// Delegatee.
    pub to: Address,
    /// Delegated bandwidth amount (in staked TRX terms).
    pub bandwidth_amount: Trx,
    /// Delegated energy amount (in staked TRX terms).
    pub energy_amount: Trx,
    /// Bandwidth lock expiry (unix ms; `0` = unlocked).
    pub bandwidth_expire_time_ms: i64,
    /// Energy lock expiry (unix ms; `0` = unlocked).
    pub energy_expire_time_ms: i64,
}

/// On-chain super representative (SR) or SR candidate.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct WitnessInfo {
    /// SR address.
    pub address: Address,
    /// Total votes received.
    pub vote_count: i64,
    /// SR announcement URL.
    pub url: String,
    /// Total blocks produced by this SR.
    pub total_produced: i64,
    /// Total blocks missed by this SR.
    pub total_missed: i64,
    /// Whether this SR is currently in the active producing set (top 27).
    pub is_active: bool,
}

/// Index of all delegation relationships for an account.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DelegatedResourceIndex {
    /// The account this index describes.
    pub account: Address,
    /// Accounts that delegated **to** this address.
    pub from_accounts: Vec<Address>,
    /// Accounts this address delegated **to**.
    pub to_accounts: Vec<Address>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission(id: i32) -> Permission {
        Permission {
            id,
            permission_name: format!("permission{id}"),
            threshold: 1,
            keys: Vec::new(),
            operations: crate::types::OperationSet::empty(),
        }
    }

    #[test]
    fn permissions_are_looked_up_by_id_not_position() {
        let permissions = AccountPermissions {
            owner: Some(permission(0)),
            witness: Some(permission(1)),
            actives: vec![permission(2), permission(4)],
        };

        assert_eq!(permissions.by_id(0).map(|p| p.id), Some(0));
        assert_eq!(permissions.by_id(1).map(|p| p.id), Some(1));
        assert_eq!(permissions.by_id(4).map(|p| p.id), Some(4));
        assert!(permissions.by_id(3).is_none());
    }

    #[test]
    fn authorization_comes_from_the_permission_slot() {
        let mut active = permission(2);
        active.operations.insert(ContractKind::Transfer).unwrap();
        let permissions = AccountPermissions {
            owner: Some(permission(0)),
            witness: Some(permission(1)),
            actives: vec![active],
        };

        assert!(permissions.allows(0, ContractKind::AccountPermissionUpdate));
        assert!(!permissions.allows(1, ContractKind::Transfer));
        assert!(permissions.allows(2, ContractKind::Transfer));
        assert!(!permissions.allows(2, ContractKind::TriggerSmartContract));
        assert!(!permissions.allows(99, ContractKind::Transfer));
    }
}
