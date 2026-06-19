//! Per-operation, typed transaction builders.
//!
//! Each builder exposes only the fields relevant to its operation and resolves
//! the sender from the provider's signer by default. Calling `.send()` builds a
//! [`TransactionRequest`](crate::types::TransactionRequest) and hands it to
//! [`TronProvider::send_transaction`].

pub mod account;
pub mod contract;
pub mod delegate;
pub mod freeze;
pub mod permission;
pub mod rewards;
pub mod transfer;
pub mod vote;
pub mod withdraw;

use tronz_primitives::Address;

use crate::{
    error::{Error, Result},
    provider::TronProvider,
};

/// Resolve the explicit `owner` override, falling back to the provider's
/// attached signer. Returns [`Error::no_signer()`] when neither is present.
///
/// Used by every builder's `send()` to avoid repeating the same 3-line
/// `or_else / ok_or` pattern across 27 call sites.
pub(crate) fn resolve_owner<P: TronProvider>(
    owner: Option<Address>,
    provider: &P,
) -> Result<Address> {
    owner
        .or_else(|| provider.signer_address())
        .ok_or(Error::no_signer())
}

pub use account::{CreateAccountBuilder, UpdateAccountBuilder};
pub use contract::{
    ClearContractAbiBuilder, SetAccountIdBuilder, UpdateContractEnergyLimitBuilder,
    UpdateContractSettingBuilder,
};
pub use delegate::{DelegateBuilder, UndelegateBuilder};
pub use freeze::{FreezeBuilder, FreezeV1Builder, UnfreezeBuilder, UnfreezeV1Builder};
pub use permission::AccountPermissionUpdateBuilder;
pub use rewards::WithdrawBalanceBuilder;
pub use transfer::TransferBuilder;
pub use vote::VoteBuilder;
pub use withdraw::{CancelAllUnfreezeBuilder, WithdrawExpireBuilder};
