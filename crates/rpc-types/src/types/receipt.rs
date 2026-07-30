//! Transaction receipt / log types.

use tronz_primitives::{Address, Log, Trx, TxId};

/// Receipt returned after a transaction is confirmed on-chain.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Total TRX the chain charged for the transaction.
    pub fee: Trx,
    /// Total energy consumed.
    pub energy_usage_total: i64,
    /// Energy fee paid (burned TRX).
    pub energy_fee: Trx,
    /// Bandwidth consumed.
    pub net_usage: i64,
    /// Bandwidth fee paid (burned TRX).
    pub net_fee: Trx,
    /// Resource usage exactly as the chain reported it.
    pub receipt: ResourceReceipt,
    /// Detailed contract execution result.
    pub contract_result: ContractResult,
    /// Deployed contract address (populated for deploy transactions).
    pub contract_address: Option<Address>,
    /// Emitted event logs.
    pub logs: Vec<Log>,
    /// Calls made by the contract while it ran.
    pub internal_transactions: Vec<InternalTransaction>,
    /// Revert reason, if the contract reverted.
    pub revert_reason: Option<String>,
}

impl TransactionInfo {
    /// Returns whether execution succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self.status, TxStatus::Success)
    }
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
    /// Jumped to a bytecode location that is not a valid destination.
    BadJumpDestination,
    /// The VM ran out of memory.
    OutOfMemory,
    /// A precompiled-contract call failed.
    PrecompiledContract,
    /// The VM stack had too few items.
    StackTooSmall,
    /// The VM stack grew beyond its limit.
    StackTooLarge,
    /// The VM encountered an illegal opcode or operation.
    IllegalOperation,
    /// The VM stack overflowed.
    StackOverflow,
    /// Ran out of energy.
    OutOfEnergy,
    /// Execution exceeded its time limit.
    OutOfTime,
    /// The java-tron VM thread overflowed its JVM stack.
    JvmStackOverflow,
    /// java-tron reported an unspecified VM failure.
    UnknownFailure,
    /// A value transfer performed by the contract failed.
    TransferFailed,
    /// Deployed bytecode was invalid.
    InvalidCode,
    /// A result code introduced by a newer node, conservatively treated as a failure.
    Other(i32),
}

impl ContractResult {
    /// Whether contract execution completed successfully.
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }

    /// Returns whether this is a failure, including unknown future codes.
    pub const fn is_failure(self) -> bool {
        !matches!(self, Self::Default | Self::Success)
    }

    /// Whether execution explicitly reverted.
    pub const fn is_revert(self) -> bool {
        matches!(self, Self::Revert)
    }
}

/// Resource usage for a transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceReceipt {
    /// Energy paid for from the caller's staked resource.
    pub energy_usage: i64,
    /// Energy paid for by burning TRX.
    pub energy_fee: Trx,
    /// Energy supplied by the contract origin.
    pub origin_energy_usage: i64,
    /// Total energy used.
    pub energy_usage_total: i64,
    /// Bandwidth used.
    pub net_usage: i64,
    /// Bandwidth paid for by burning TRX.
    pub net_fee: Trx,
    /// Energy charged on top as a penalty.
    pub energy_penalty_total: i64,
}

/// A call a contract made while executing, as the chain recorded it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct InternalTransaction {
    /// Identifies this internal call. The root one is the transaction's own id.
    pub hash: TxId,
    /// Who made the call.
    pub caller_address: Option<Address>,
    /// Who it called.
    pub transfer_to_address: Option<Address>,
    /// Value moved by the call, one entry per asset.
    pub call_values: Vec<CallValue>,
    /// What kind of call it was, as java-tron labelled it (`call`, `create`, ...).
    pub note: String,
    /// Whether the call was rolled back.
    pub rejected: bool,
    /// Extra information the node attached, if any.
    pub extra: String,
}

/// Value moved by an internal call.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CallValue {
    /// The amount moved, in the asset's own base unit.
    pub amount: i64,
    /// The TRC10 asset moved, or empty for TRX.
    pub token_id: String,
}
