//! Stake (freeze) and unstake (unfreeze) builders — Stake 1.0 (legacy) and 2.0.

use tronz_primitives::{Address, ResourceCode, Trx};

use super::resolve_owner;
use crate::{
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    types::{
        ContractType, FreezeBalanceV1Contract, FreezeBalanceV2Contract, TransactionRequest,
        UnfreezeBalanceV1Contract, UnfreezeBalanceV2Contract,
    },
};

/// Stake TRX to obtain energy or bandwidth (Stake 1.0, legacy).
///
/// On mainnet `frozen_duration` must be `3` (the only accepted value);
/// the builder defaults to `3` automatically.
/// Set `receiver` to delegate the obtained resource to another account in one
/// step (inline delegation).
pub struct FreezeV1Builder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    amount: Option<Trx>,
    resource: ResourceCode,
    frozen_duration: i64,
    receiver: Option<Address>,
}

impl<'a, P: TronProvider> FreezeV1Builder<'a, P> {
    /// Start a new V1 freeze builder (defaults to energy, duration = 3).
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            amount: None,
            resource: ResourceCode::Energy,
            frozen_duration: 3,
            receiver: None,
        }
    }

    /// Override the staking account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Amount of TRX to stake.
    pub fn amount(mut self, amount: Trx) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Resource to obtain.
    pub fn resource(mut self, resource: ResourceCode) -> Self {
        self.resource = resource;
        self
    }

    /// Lock duration in days (must be `3` on mainnet).
    pub fn frozen_duration(mut self, days: i64) -> Self {
        self.frozen_duration = days;
        self
    }

    /// Delegate the obtained resource to this address (inline delegation).
    pub fn receiver(mut self, receiver: Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::FreezeBalanceV1(FreezeBalanceV1Contract {
                owner_address: owner,
                frozen_balance: amount,
                frozen_duration: self.frozen_duration,
                resource: self.resource,
                receiver_address: self.receiver,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Unstake TRX (Stake 1.0, legacy).
///
/// This releases **all** staked TRX for the given resource immediately
/// (no unbonding delay, unlike Stake 2.0).
///
/// **Important**: if the original freeze used `.receiver(addr)` (inline delegation),
/// you must call `.receiver(addr)` here with the same address, or the node will
/// reject the transaction.
pub struct UnfreezeV1Builder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    resource: ResourceCode,
    receiver: Option<Address>,
}

impl<'a, P: TronProvider> UnfreezeV1Builder<'a, P> {
    /// Start a new V1 unfreeze builder (defaults to releasing energy stake).
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            resource: ResourceCode::Energy,
            receiver: None,
        }
    }

    /// Override the account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Resource being released.
    pub fn resource(mut self, resource: ResourceCode) -> Self {
        self.resource = resource;
        self
    }

    /// If the stake was delegated, the delegatee address.
    pub fn receiver(mut self, receiver: Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;

        let req = TransactionRequest {
            contract: Some(ContractType::UnfreezeBalanceV1(UnfreezeBalanceV1Contract {
                owner_address: owner,
                resource: self.resource,
                receiver_address: self.receiver,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Stake TRX to obtain energy or bandwidth (`FreezeBalanceV2`).
pub struct FreezeBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    amount: Option<Trx>,
    resource: ResourceCode,
}

impl<'a, P: TronProvider> FreezeBuilder<'a, P> {
    /// Start a new freeze builder (defaults to staking for energy).
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            amount: None,
            resource: ResourceCode::Energy,
        }
    }

    /// Override the staking account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Amount of TRX to stake.
    pub fn amount(mut self, amount: Trx) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Resource to obtain.
    pub fn resource(mut self, resource: ResourceCode) -> Self {
        self.resource = resource;
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::FreezeBalanceV2(FreezeBalanceV2Contract {
                owner_address: owner,
                frozen_balance: amount,
                resource: self.resource,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

/// Unstake TRX (`UnfreezeBalanceV2`); subject to the network unbonding delay.
pub struct UnfreezeBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    amount: Option<Trx>,
    resource: ResourceCode,
}

impl<'a, P: TronProvider> UnfreezeBuilder<'a, P> {
    /// Start a new unfreeze builder (defaults to releasing energy stake).
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            amount: None,
            resource: ResourceCode::Energy,
        }
    }

    /// Override the account.
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Amount of TRX to unstake.
    pub fn amount(mut self, amount: Trx) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Resource being released.
    pub fn resource(mut self, resource: ResourceCode) -> Self {
        self.resource = resource;
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UnfreezeBalanceV2(UnfreezeBalanceV2Contract {
                owner_address: owner,
                unfreeze_balance: amount,
                resource: self.resource,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
