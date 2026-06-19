//! Error types for contract interactions.

use alloy_primitives::{B256, Selector};
use alloy_sol_types::{SolError, SolInterface};
use thiserror::Error;
use tronz_primitives::Bytes;
use tronz_provider::{PendingTransactionError, ProviderError, types::TransactionInfo};

/// The result type for contract operations.
pub type Result<T, E = ContractError> = core::result::Result<T, E>;

/// Errors returned by [`ContractInstance`](crate::instance::ContractInstance) and
/// token instance methods.
///
/// There is **no `NoSigner` variant** here: a missing signer surfaces as
/// `Provider(ProviderError::LocalUsageError(...))`, avoiding duplication
/// with the provider layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContractError {
    /// A provider, transport, signing, or usage error.
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// ABI encoding or decoding failed.
    #[error("ABI error: {0}")]
    Abi(#[from] alloy_dyn_abi::Error),
    /// The requested function name was not found in the ABI.
    #[error("unknown function: `{0}`")]
    UnknownFunction(String),
    /// The requested function selector was not found in the ABI.
    #[error("unknown function selector: {0}")]
    UnknownSelector(Selector),
    /// The contract returned no data — the address may not be a contract.
    #[error("contract call to `{0}` returned no data; the address might not be a contract")]
    ZeroData(String, #[source] alloy_dyn_abi::Error),
    /// The contract call reverted.  Contains the raw ABI-encoded revert data.
    ///
    /// Use [`as_decoded_error`] or [`as_decoded_interface_error`] to decode.
    ///
    /// [`as_decoded_error`]: ContractError::as_decoded_error
    /// [`as_decoded_interface_error`]: ContractError::as_decoded_interface_error
    #[error("contract call reverted")]
    Revert(Bytes),
    /// The requested event topic was not found in the ABI.
    #[error("unknown event topic: {0}")]
    UnknownEvent(B256),
    /// Contract was not deployed: the deployment transaction succeeded but
    /// `contract_address` was absent from the receipt.
    #[error("contract not deployed: deployment transaction did not produce a contract address")]
    ContractNotDeployed,

    /// Confirmation polling timed out without the transaction being indexed.
    ///
    /// Flattened from [`PendingTransactionError::ConfirmationTimeout`] via the
    /// [`From`] impl so callers can match it directly without nesting.
    #[error("timed out waiting for transaction confirmation")]
    ConfirmationTimeout,

    /// The transaction was confirmed on-chain but its execution did not succeed
    /// (reverted, ran out of energy, etc.). Carries the full receipt.
    ///
    /// Flattened from [`PendingTransactionError::Reverted`].
    #[error("transaction confirmed but execution failed: {:?}", .0.contract_result)]
    ExecutionFailed(Box<TransactionInfo>),
}

impl From<alloy_sol_types::Error> for ContractError {
    fn from(e: alloy_sol_types::Error) -> Self {
        Self::Abi(e.into())
    }
}

/// Flatten [`PendingTransactionError`] into `ContractError` so that
/// `pending.get_receipt().await?` works inside contract methods.
///
/// - [`PendingTransactionError::Transport`] → [`ContractError::Provider`]
/// - [`PendingTransactionError::ConfirmationTimeout`] → [`ContractError::ConfirmationTimeout`]
/// - [`PendingTransactionError::Reverted`] → [`ContractError::ExecutionFailed`]
impl From<PendingTransactionError> for ContractError {
    fn from(e: PendingTransactionError) -> Self {
        match e {
            PendingTransactionError::Transport(e) => Self::Provider(e),
            PendingTransactionError::ConfirmationTimeout => Self::ConfirmationTimeout,
            PendingTransactionError::Reverted(info) => Self::ExecutionFailed(info),
            // Forward any future variants added to PendingTransactionError as a
            // LocalUsageError so this From impl doesn't need updating on every
            // minor version of tronz-provider.
            _ => Self::Provider(ProviderError::local_usage_str(
                "unknown pending transaction error",
            )),
        }
    }
}

impl ContractError {
    /// Returns the raw ABI-encoded revert data if the error is a [`Revert`].
    ///
    /// [`Revert`]: ContractError::Revert
    pub fn as_revert_data(&self) -> Option<&Bytes> {
        if let Self::Revert(data) = self {
            Some(data)
        } else {
            None
        }
    }

    /// Attempt to ABI-decode the revert data into a specific [`SolError`] type.
    ///
    /// Returns `None` if the error is not a revert, or if decoding fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use alloy_sol_types::sol;
    /// sol! { error InsufficientBalance(uint256 have, uint256 need); }
    ///
    /// # fn example(err: tronz_contract::ContractError) {
    /// if let Some(e) = err.as_decoded_error::<InsufficientBalance>() {
    ///     println!("need {} but only have {}", e.need, e.have);
    /// }
    /// # }
    /// ```
    pub fn as_decoded_error<E: SolError>(&self) -> Option<E> {
        self.as_revert_data()
            .and_then(|data| E::abi_decode(data).ok())
    }

    /// Attempt to ABI-decode the revert data into one of the custom errors in a [`SolInterface`].
    ///
    /// Returns `None` if the error is not a revert, or if decoding fails.
    pub fn as_decoded_interface_error<I: SolInterface>(&self) -> Option<I> {
        self.as_revert_data()
            .and_then(|data| I::abi_decode(data).ok())
    }

    /// Build a [`ContractError`] from a failed output decode.
    ///
    /// Promotes empty output to [`ZeroData`] for a more helpful error message.
    ///
    /// [`ZeroData`]: ContractError::ZeroData
    pub(crate) fn decode_err(name: &str, data: &[u8], error: alloy_dyn_abi::Error) -> Self {
        if data.is_empty() {
            let short = name.split('(').next().unwrap_or(name);
            return Self::ZeroData(short.to_string(), error);
        }
        Self::Abi(error)
    }
}
