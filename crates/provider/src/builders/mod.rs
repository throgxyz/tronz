//! Per-operation, typed transaction builders.
//!
//! Each builder exposes only the fields relevant to its operation and resolves
//! the owner address from the provider's signer by default. Every builder ends in one
//! of three exits, all of which go through a
//! [`TransactionRequest`](crate::types::TransactionRequest):
//!
//! - `.send()` fills, signs, and broadcasts, via [`TronProvider::send_transaction`].
//! - `.build()` stops at the unsigned transaction, via [`TronProvider::build_transaction`] — the
//!   entry point for multisig, where several keys must sign before [`TronProvider::broadcast`].
//! - `.into_request()` stops before any network call, for callers that want to adjust fields the
//!   builders do not expose.
//!
//! Builders authorized by an active permission rather than by the account's own
//! owner permission additionally set `.permission_id(id)`.

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
    owner.or_else(|| provider.signer_address()).ok_or(Error::no_signer())
}

/// Generates the exits every transaction builder shares — the `permission_id`
/// setter plus `build` and `send` — on top of the builder's own
/// `into_request`.
macro_rules! builder_exits {
    () => {
        /// Authorize through an active permission instead of the owner
        /// permission.
        ///
        /// `0` is the owner permission and `2` upwards are the active ones, as
        /// configured through
        /// [`AccountPermissionUpdateBuilder`](crate::builders::AccountPermissionUpdateBuilder).
        /// Set this when the signing keys belong to an active permission rather
        /// than to the owner itself.
        pub fn permission_id(mut self, id: i32) -> Self {
            self.permission_id = Some(id);
            self
        }

        /// Ask the node to build the transaction, without signing or
        /// broadcasting it.
        ///
        /// Use this when the authorizing permission needs more than one
        /// signature: sign
        /// [`RawTransaction::tx_id`](crate::types::RawTransaction::tx_id) with
        /// every required key, then submit through
        /// [`TronProvider::broadcast`](crate::TronProvider::broadcast).
        pub async fn build(self) -> Result<crate::types::RawTransaction> {
            let provider = self.provider;
            provider.build_transaction(self.into_request()?).await
        }

        /// Build, sign, and broadcast.
        pub async fn send(self) -> Result<PendingTransaction> {
            let provider = self.provider;
            provider.send_transaction(self.into_request()?).await
        }
    };
}
pub use account::{CreateAccountBuilder, UpdateAccountBuilder};
pub(crate) use builder_exits;
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
