//! Delegate / undelegate resource builders.

use tronz_primitives::{Address, ResourceCode, Trx};

use super::resolve_owner;
use crate::{
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    types::{
        ContractType, DelegateResourceContract, TransactionRequest, UnDelegateResourceContract,
    },
};

/// Delegate staked energy or bandwidth to another account.
pub struct DelegateBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    to: Option<Address>,
    amount: Option<Trx>,
    resource: ResourceCode,
    lock_period: Option<i64>,
}

impl<'a, P: TronProvider> DelegateBuilder<'a, P> {
    /// Start a new delegate builder (defaults to delegating energy).
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            to: None,
            amount: None,
            resource: ResourceCode::Energy,
            lock_period: None,
        }
    }

    /// Override the delegator account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Delegatee account.
    pub fn to(mut self, to: Address) -> Self {
        self.to = Some(to);
        self
    }

    /// Amount of staked TRX whose resource is delegated.
    pub fn amount(mut self, amount: Trx) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Resource being delegated.
    pub fn resource(mut self, resource: ResourceCode) -> Self {
        self.resource = resource;
        self
    }

    /// Lock the delegation for `secs` seconds (max 864_000 per protocol).
    pub fn lock_period(mut self, secs: i64) -> Self {
        self.lock_period = Some(secs);
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let to = self.to.ok_or(Error::missing_field("to"))?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::DelegateResource(DelegateResourceContract {
                owner_address: owner,
                resource: self.resource,
                balance: amount,
                receiver_address: to,
                lock_period: self.lock_period,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Reclaim resources previously delegated to another account.
pub struct UndelegateBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    receiver: Option<Address>,
    amount: Option<Trx>,
    resource: ResourceCode,
}

impl<'a, P: TronProvider> UndelegateBuilder<'a, P> {
    /// Start a new undelegate builder (defaults to reclaiming energy).
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            receiver: None,
            amount: None,
            resource: ResourceCode::Energy,
        }
    }

    /// Override the delegator account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Account whose delegation is being reclaimed.
    pub fn receiver(mut self, receiver: Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Amount of staked TRX whose resource is reclaimed.
    pub fn amount(mut self, amount: Trx) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Resource being reclaimed.
    pub fn resource(mut self, resource: ResourceCode) -> Self {
        self.resource = resource;
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let receiver = self.receiver.ok_or(Error::missing_field("receiver"))?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UnDelegateResource(
                UnDelegateResourceContract {
                    owner_address: owner,
                    resource: self.resource,
                    balance: amount,
                    receiver_address: receiver,
                },
            )),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
