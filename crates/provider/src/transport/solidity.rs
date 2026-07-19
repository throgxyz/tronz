//! Read-only transport over `protocol.WalletSolidity`.

use core::future::Future;

use tronz_primitives::{Address, ResourceCode, Trx, TxId};

use crate::types::{
    AccountInfo, BlockInfo, ConstantCallResult, DelegatedResource, DelegatedResourceIndex,
    SignedTransaction, TransactionInfo, TriggerSmartContract, WitnessInfo,
};

/// A low-level transport for `protocol.WalletSolidity`.
///
/// Implementations are cheap to clone and must be `Send + Sync + 'static`.
///
/// This trait is **sealed** — only `tronz` may implement it. For tests, use the
/// `MockSolidityTransport` provided under the `mock` feature.
pub trait SolidityTransport: Clone + Send + Sync + 'static + super::private::Sealed {
    /// The transport's error type.
    type Error: std::error::Error + Into<crate::error::TransportErrorKind> + Send + Sync + 'static;

    /// Fetch the latest solidified block.
    fn get_now_block(&self) -> impl Future<Output = Result<BlockInfo, Self::Error>> + Send;

    /// Fetch a solidified block by height.
    fn get_block_by_number(
        &self,
        num: i64,
    ) -> impl Future<Output = Result<BlockInfo, Self::Error>> + Send;

    /// Fetch solidified on-chain account state.
    fn get_account(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<AccountInfo, Self::Error>> + Send;

    /// Fetch a transaction by id from solidified state.
    fn get_transaction_by_id(
        &self,
        tx_id: TxId,
    ) -> impl Future<Output = Result<SignedTransaction, Self::Error>> + Send;

    /// Fetch a transaction's receipt from solidified state.
    ///
    /// Returns `None` until the transaction has solidified — this is the signal
    /// the SDK polls on to confirm irreversibility.
    fn get_transaction_info(
        &self,
        tx_id: TxId,
    ) -> impl Future<Output = Result<Option<TransactionInfo>, Self::Error>> + Send;

    /// Fetch all transaction receipts included in a solidified block.
    fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> impl Future<Output = Result<Vec<TransactionInfo>, Self::Error>> + Send;

    /// Count transactions in a solidified block by block number.
    fn get_transaction_count_by_block_num(
        &self,
        block_num: i64,
    ) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    /// Execute a constant (read-only) contract call against solidified state.
    fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> impl Future<Output = Result<ConstantCallResult, Self::Error>> + Send;

    /// Estimate the energy a contract call would consume against solidified state.
    fn estimate_energy(
        &self,
        params: TriggerSmartContract,
    ) -> impl Future<Output = Result<i64, Self::Error>> + Send;

    /// List all super representatives and candidates from solidified state.
    fn list_witnesses(&self) -> impl Future<Output = Result<Vec<WitnessInfo>, Self::Error>> + Send;

    /// Fetch a paginated list of witnesses sorted by real-time vote count.
    fn get_paginated_now_witness_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<WitnessInfo>, Self::Error>> + Send;

    /// Query delegations between two accounts from solidified state (Stake 1.0, legacy).
    fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> impl Future<Output = Result<Vec<DelegatedResource>, Self::Error>> + Send;

    /// Query the delegation index for an account from solidified state (Stake 1.0, legacy).
    fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<DelegatedResourceIndex, Self::Error>> + Send;

    /// Query delegations between two accounts from solidified state (Stake 2.0).
    fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> impl Future<Output = Result<Vec<DelegatedResource>, Self::Error>> + Send;

    /// Query the delegation index for an account from solidified state (Stake 2.0).
    fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<DelegatedResourceIndex, Self::Error>> + Send;

    /// Query the max amount still delegatable for a resource from solidified state.
    fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> impl Future<Output = Result<Trx, Self::Error>> + Send;

    /// Query how many unfreeze operations are still available from solidified state.
    fn get_available_unfreeze_count(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<i64, Self::Error>> + Send;

    /// Query the amount withdrawable at a timestamp from solidified state.
    fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> impl Future<Output = Result<Trx, Self::Error>> + Send;
}
