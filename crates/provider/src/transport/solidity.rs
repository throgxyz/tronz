//! Read-only transport over `protocol.WalletSolidity`.

use std::sync::Arc;

use async_trait::async_trait;
use auto_impl::auto_impl;
use tronz_primitives::{Address, ResourceCode, Trx, TxId};

use crate::{
    error::TransportResult,
    types::{
        AccountInfo, BlockInfo, ConstantCallResult, DelegatedResource, DelegatedResourceIndex,
        SignedTransaction, TransactionInfo, TriggerSmartContract, WitnessInfo,
    },
};

/// A low-level transport for `protocol.WalletSolidity`.
///
/// Boxed and object-safe on the same terms as
/// [`TronTransport`](super::TronTransport); see [`DynSolidityTransport`].
///
/// This trait is **sealed** — only `tronz` may implement it. For tests, use the
/// `MockSolidityTransport` provided under the `mock` feature.
#[async_trait]
#[auto_impl(&, Arc)]
pub trait SolidityTransport: Send + Sync + 'static + super::private::Sealed {
    /// Fetch the latest solidified block.
    async fn get_now_block(&self) -> TransportResult<BlockInfo>;

    /// Fetch a solidified block by height.
    async fn get_block_by_number(&self, num: i64) -> TransportResult<Option<BlockInfo>>;

    /// Fetch solidified on-chain account state.
    async fn get_account(&self, address: Address) -> TransportResult<AccountInfo>;

    /// Fetch a transaction by id from solidified state.
    async fn get_transaction_by_id(
        &self,
        tx_id: TxId,
    ) -> TransportResult<Option<SignedTransaction>>;

    /// Fetch a transaction's receipt from solidified state.
    ///
    /// Returns `None` until the transaction has solidified — this is the signal
    /// the SDK polls on to confirm irreversibility.
    async fn get_transaction_info(&self, tx_id: TxId) -> TransportResult<Option<TransactionInfo>>;

    /// Fetch all transaction receipts included in a solidified block.
    async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> TransportResult<Vec<TransactionInfo>>;

    /// Count transactions in a solidified block by block number.
    async fn get_transaction_count_by_block_num(&self, block_num: i64) -> TransportResult<u64>;

    /// Execute a constant (read-only) contract call against solidified state.
    async fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> TransportResult<ConstantCallResult>;

    /// Estimate the energy a contract call would consume against solidified state.
    async fn estimate_energy(&self, params: TriggerSmartContract) -> TransportResult<i64>;

    /// List all super representatives and candidates from solidified state.
    async fn list_witnesses(&self) -> TransportResult<Vec<WitnessInfo>>;

    /// Fetch a paginated list of witnesses sorted by real-time vote count.
    async fn get_paginated_now_witness_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<WitnessInfo>>;

    /// Query delegations between two accounts from solidified state (Stake 1.0, legacy).
    async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> TransportResult<Vec<DelegatedResource>>;

    /// Query the delegation index for an account from solidified state (Stake 1.0, legacy).
    async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> TransportResult<DelegatedResourceIndex>;

    /// Query delegations between two accounts from solidified state (Stake 2.0).
    async fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> TransportResult<Vec<DelegatedResource>>;

    /// Query the delegation index for an account from solidified state (Stake 2.0).
    async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> TransportResult<DelegatedResourceIndex>;

    /// Query the max amount still delegatable for a resource from solidified state.
    async fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> TransportResult<Trx>;

    /// Query how many unfreeze operations are still available from solidified state.
    async fn get_available_unfreeze_count(&self, address: Address) -> TransportResult<i64>;

    /// Query the amount withdrawable at a timestamp from solidified state.
    async fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> TransportResult<Trx>;
}

/// A [`SolidityTransport`] with its concrete type erased. The solidified
/// counterpart to [`DynTransport`](super::DynTransport).
pub type DynSolidityTransport = Arc<dyn SolidityTransport>;
