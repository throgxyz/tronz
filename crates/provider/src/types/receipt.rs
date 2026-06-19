//! Transaction receipt / log types.

use tronz_primitives::{Address, B256, Bytes, Trx, TxId};

/// Receipt returned after a transaction is confirmed on-chain.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TransactionInfo {
    /// Transaction id.
    pub tx_id: TxId,
    /// Block the transaction was included in.
    pub block_number: i64,
    /// Block timestamp (unix ms).
    pub block_timestamp: i64,
    /// Overall success/failure status.
    pub status: TxStatus,
    /// Energy consumed.
    pub energy_usage: i64,
    /// Energy fee paid (burned TRX).
    pub energy_fee: Trx,
    /// Bandwidth consumed.
    pub net_usage: i64,
    /// Bandwidth fee paid (burned TRX).
    pub net_fee: Trx,
    /// Detailed contract execution result.
    pub contract_result: ContractResult,
    /// Deployed contract address (populated for deploy transactions).
    pub contract_address: Option<Address>,
    /// Emitted event logs.
    pub logs: Vec<Log>,
    /// Revert reason, if the contract reverted.
    pub revert_reason: Option<String>,
}

/// Top-level transaction status.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum TxStatus {
    /// The transaction succeeded.
    Success,
    /// The transaction failed.
    Failed,
}

/// Detailed contract execution result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ContractResult {
    /// Default / not applicable.
    Default,
    /// Executed successfully.
    Success,
    /// Reverted.
    Revert,
    /// Ran out of energy.
    OutOfEnergy,
    /// Other VM-level failure.
    Failed,
}

/// An EVM-style event log emitted during contract execution.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Log {
    /// Emitting contract address.
    pub address: Address,
    /// Indexed topics (topic0 = event signature hash).
    pub topics: Vec<B256>,
    /// Non-indexed data.
    pub data: Bytes,
}

/// Resource usage receipt for a transaction.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct ResourceReceipt {
    /// Energy paid for from staked resource.
    pub energy_usage: i64,
    /// Energy paid for by burning TRX.
    pub energy_fee: i64,
    /// Energy supplied by the contract origin.
    pub origin_energy_usage: i64,
    /// Total energy used.
    pub energy_usage_total: i64,
    /// Bandwidth used.
    pub net_usage: i64,
    /// Bandwidth paid for by burning TRX.
    pub net_fee: i64,
}
