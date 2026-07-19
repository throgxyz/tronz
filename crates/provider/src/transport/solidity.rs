//! Read-only transport over `protocol.WalletSolidity`.

use core::future::Future;

use tronz_primitives::{Address, TxId};

use crate::types::{
    AccountInfo, BlockInfo, ConstantCallResult, SignedTransaction, TransactionInfo,
    TriggerSmartContract, WitnessInfo,
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
}
