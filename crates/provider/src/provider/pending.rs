//! Pending transaction handle with full-node receipt polling.

use core::time::Duration;

use thiserror::Error;
use tronz_primitives::TxId;

use crate::{
    error::ProviderError,
    observability::{OUTCOME_ERROR, OUTCOME_OK, OUTCOME_TIMEOUT, elapsed_ms},
    provider::TronProvider,
    types::TransactionInfo,
};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(3);
const DEFAULT_POLL_ATTEMPTS: u32 = 20;

/// Errors that can occur while waiting for a pending transaction.
///
/// Separating polling errors from [`ProviderError`] keeps the two concerns
/// orthogonal: transport failures during polling are wrapped in [`Transport`],
/// while a clean polling timeout is its own variant.
///
/// [`Transport`]: PendingTransactionError::Transport
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PendingTransactionError {
    /// A transport or provider error occurred while polling.
    #[error(transparent)]
    Transport(#[from] ProviderError),

    /// Polling exhausted all attempts before the full node indexed the receipt.
    ///
    /// The variant name is retained for compatibility. Full-node receipt
    /// availability proves inclusion, not TRON solidification.
    #[error("timed out waiting for transaction receipt")]
    ConfirmationTimeout,

    /// The transaction was included on-chain but execution did not succeed
    /// (e.g. reverted or ran out of energy). Carries the full receipt.
    #[error("transaction execution failed: {:?}", .0.contract_result)]
    Reverted(Box<TransactionInfo>),
}

/// Handle to a broadcast transaction; can be awaited until its receipt is indexed.
///
/// Owns a clone of the provider (cheap — all concrete providers are
/// `Arc`-backed), so no lifetime parameter is needed.
pub struct PendingTransaction<P: TronProvider> {
    provider: P,
    tx_id: TxId,
}

impl<P: TronProvider> PendingTransaction<P> {
    /// Construct a handle for an already-broadcast transaction id.
    pub fn new(provider: P, tx_id: TxId) -> Self {
        Self { provider, tx_id }
    }

    /// The broadcast transaction's id.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Poll until the full node has indexed the transaction in a block.
    ///
    /// Defaults to every 3 s, up to 20 attempts (~60 s total). Inclusion does
    /// not mean that the containing block has been solidified.
    pub async fn await_included(self) -> Result<TransactionInfo, PendingTransactionError> {
        self.await_inner(DEFAULT_POLL_INTERVAL, DEFAULT_POLL_ATTEMPTS, true).await
    }

    /// Poll until an included transaction receipt is available.
    ///
    /// This mirrors Alloy's `PendingTransactionBuilder::get_receipt`: it waits
    /// for inclusion, not TRON solidification.
    pub async fn get_receipt(self) -> Result<TransactionInfo, PendingTransactionError> {
        self.await_included().await
    }

    /// Poll for inclusion with a custom interval and attempt count.
    ///
    /// Returns the receipt as soon as the node has indexed the transaction,
    /// regardless of whether on-chain execution succeeded — inspect
    /// [`TransactionInfo::is_success`] (or use [`await_success`]) to assert it
    /// ran successfully.
    ///
    /// [`await_success`]: Self::await_success
    pub async fn await_included_with(
        self,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        self.await_inner(interval, max_attempts, false).await
    }

    /// Compatibility alias for [`await_included`](Self::await_included).
    ///
    /// This method retains its existing name and FullNode-only behavior. It
    /// waits until the FullNode Wallet service exposes a receipt, which proves
    /// inclusion but does not prove TRON solidification.
    pub async fn await_confirmed(self) -> Result<TransactionInfo, PendingTransactionError> {
        self.await_inner(DEFAULT_POLL_INTERVAL, DEFAULT_POLL_ATTEMPTS, true).await
    }

    /// Compatibility alias for [`await_included_with`](Self::await_included_with).
    ///
    /// Receipt availability through this FullNode-only API does not prove TRON
    /// solidification.
    pub async fn await_confirmed_with(
        self,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        self.await_inner(interval, max_attempts, false).await
    }

    /// Poll one transaction state with an explicit timeout log level policy.
    async fn await_inner(
        self,
        interval: Duration,
        max_attempts: u32,
        warn_on_timeout: bool,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        let tracing_enabled = if warn_on_timeout {
            tracing::enabled!(target: "tronz::transaction", tracing::Level::WARN)
        } else {
            tracing::enabled!(target: "tronz::transaction", tracing::Level::DEBUG)
        };
        let started_at = tracing_enabled.then(std::time::Instant::now);

        for attempt in 1..=max_attempts {
            // Check first so a transaction that already reached the requested
            // state returns immediately; only sleep between later attempts.
            if attempt > 1 {
                tokio::time::sleep(interval).await;
            }

            let result = self.provider.get_transaction_info(self.tx_id).await;

            match result {
                Ok(Some(info)) => {
                    trace!(
                        target: "tronz::transaction",
                        tx_id = %self.tx_id,
                        operation = "inclusion",
                        attempt,
                        found = true,
                        "transaction state poll completed"
                    );
                    debug!(
                        target: "tronz::transaction",
                        tx_id = %self.tx_id,
                        operation = "inclusion",
                        stage = "included",
                        attempt,
                        block_number = info.block_number,
                        execution_success = info.is_success(),
                        elapsed_ms = elapsed_ms(started_at),
                        outcome = OUTCOME_OK,
                        "transaction lifecycle advanced"
                    );
                    return Ok(info);
                }
                Ok(None) => {
                    trace!(
                        target: "tronz::transaction",
                        tx_id = %self.tx_id,
                        operation = "inclusion",
                        attempt,
                        found = false,
                        "transaction state poll completed"
                    );
                }
                Err(error) => {
                    debug!(
                        target: "tronz::transaction",
                        tx_id = %self.tx_id,
                        operation = "inclusion",
                        attempt,
                        elapsed_ms = elapsed_ms(started_at),
                        outcome = OUTCOME_ERROR,
                        "transaction state polling failed"
                    );
                    return Err(PendingTransactionError::Transport(error));
                }
            }
        }

        if warn_on_timeout {
            warn!(
                target: "tronz::transaction",
                tx_id = %self.tx_id,
                operation = "inclusion",
                attempts = max_attempts,
                elapsed_ms = elapsed_ms(started_at),
                outcome = OUTCOME_TIMEOUT,
                "transaction state polling timed out"
            );
        } else {
            debug!(
                target: "tronz::transaction",
                tx_id = %self.tx_id,
                operation = "inclusion",
                attempts = max_attempts,
                elapsed_ms = elapsed_ms(started_at),
                outcome = OUTCOME_TIMEOUT,
                "transaction state polling timed out"
            );
        }
        Err(PendingTransactionError::ConfirmationTimeout)
    }

    /// Like [`await_included`](Self::await_included), but additionally fails
    /// with [`PendingTransactionError::Reverted`] if the transaction was
    /// included but its on-chain execution did not succeed.
    pub async fn await_success(self) -> Result<TransactionInfo, PendingTransactionError> {
        let tx_id = self.tx_id;
        let info = self.await_included().await?;
        if info.is_success() {
            debug!(
                target: "tronz::transaction",
                %tx_id,
                operation = "execution",
                stage = "execution_success",
                block_number = info.block_number,
                outcome = OUTCOME_OK,
                "transaction execution succeeded"
            );
            Ok(info)
        } else {
            warn!(
                target: "tronz::transaction",
                %tx_id,
                operation = "execution",
                stage = "execution_failed",
                block_number = info.block_number,
                outcome = OUTCOME_ERROR,
                "transaction execution failed"
            );
            Err(PendingTransactionError::Reverted(Box::new(info)))
        }
    }
}

#[cfg(test)]
mod tests {
    use tronz_primitives::Trx;

    use super::*;
    use crate::{
        provider::RootProvider,
        test_utils::capture,
        transport::mock::MockTransport,
        types::{ContractResult, TxStatus},
    };

    fn pending(mock: MockTransport) -> PendingTransaction<RootProvider<MockTransport>> {
        PendingTransaction::new(RootProvider::new(mock), TxId::from([7; 32]))
    }

    fn transaction_info(status: TxStatus) -> TransactionInfo {
        TransactionInfo {
            tx_id: TxId::from([7; 32]),
            block_number: 42,
            block_timestamp: 1_234,
            status,
            energy_usage: 0,
            energy_fee: Trx::ZERO,
            net_usage: 0,
            net_fee: Trx::ZERO,
            contract_result: if status == TxStatus::Success {
                ContractResult::Success
            } else {
                ContractResult::Revert
            },
            contract_address: None,
            logs: Vec::new(),
            revert_reason: None,
        }
    }

    #[tokio::test]
    async fn custom_confirmation_timeout_is_debug() {
        let mock = MockTransport::new();
        mock.push_ok::<Option<TransactionInfo>>("get_transaction_info", None);
        let (subscriber, logs) = capture();
        let _guard = tracing::subscriber::set_default(subscriber);

        let result = pending(mock).await_confirmed_with(Duration::ZERO, 1).await;
        assert!(matches!(result, Err(PendingTransactionError::ConfirmationTimeout)));

        let output = logs.contents();
        assert!(output.contains("DEBUG tronz::transaction"), "{output}");
        assert!(output.contains("operation=\"inclusion\""), "{output}");
        assert!(output.contains("outcome=\"timeout\""), "{output}");
        assert!(!output.contains("stage="), "{output}");
        assert!(!output.contains("WARN tronz::transaction"), "{output}");
    }

    #[tokio::test]
    async fn default_inclusion_timeout_preserves_compatibility_error() {
        let mock = MockTransport::new();
        mock.push_ok::<Option<TransactionInfo>>("get_transaction_info", None);
        let (subscriber, logs) = capture();
        let _guard = tracing::subscriber::set_default(subscriber);

        let result = pending(mock).await_inner(Duration::ZERO, 1, true).await;
        assert!(matches!(result, Err(PendingTransactionError::ConfirmationTimeout)));

        let output = logs.contents();
        assert!(output.contains("WARN tronz::transaction"), "{output}");
        assert!(output.contains("operation=\"inclusion\""), "{output}");
        assert!(output.contains("outcome=\"timeout\""), "{output}");
    }

    #[tokio::test]
    async fn receipt_waits_for_inclusion() {
        let mock = MockTransport::new();
        mock.push_ok("get_transaction_info", Some(transaction_info(TxStatus::Success)));
        let (subscriber, logs) = capture();
        let _guard = tracing::subscriber::set_default(subscriber);

        let info = pending(mock).get_receipt().await.expect("receipt is included");
        assert_eq!(info.block_number, 42);

        let output = logs.contents();
        assert!(output.contains("stage=\"included\""), "{output}");
        assert!(!output.contains("stage=\"confirmed\""), "{output}");
    }

    #[tokio::test]
    async fn confirmed_compatibility_alias_uses_fullnode_receipt() {
        let mock = MockTransport::new();
        mock.push_ok("get_transaction_info", Some(transaction_info(TxStatus::Success)));
        let (subscriber, logs) = capture();
        let _guard = tracing::subscriber::set_default(subscriber);

        let info = pending(mock).await_confirmed().await.expect("receipt is included");
        assert_eq!(info.block_number, 42);

        let output = logs.contents();
        assert!(output.contains("operation=\"inclusion\""), "{output}");
        assert!(output.contains("stage=\"included\""), "{output}");
        assert!(!output.contains("stage=\"confirmed\""), "{output}");
    }

    #[tokio::test]
    async fn execution_failure_is_distinct_from_inclusion() {
        let mock = MockTransport::new();
        mock.push_ok("get_transaction_info", Some(transaction_info(TxStatus::Failed)));
        let (subscriber, logs) = capture();
        let _guard = tracing::subscriber::set_default(subscriber);

        let result = pending(mock).await_success().await;
        assert!(matches!(result, Err(PendingTransactionError::Reverted(_))));

        let output = logs.contents();
        assert!(output.contains("stage=\"included\""), "{output}");
        assert!(output.contains("execution_success=false"), "{output}");
        assert!(output.contains("stage=\"execution_failed\""), "{output}");
    }

    #[tokio::test]
    async fn execution_success_is_distinct_from_inclusion() {
        let mock = MockTransport::new();
        mock.push_ok("get_transaction_info", Some(transaction_info(TxStatus::Success)));
        let (subscriber, logs) = capture();
        let _guard = tracing::subscriber::set_default(subscriber);

        let info = pending(mock).await_success().await.expect("execution succeeds");
        assert!(info.is_success());

        let output = logs.contents();
        assert!(output.contains("stage=\"included\""), "{output}");
        assert!(output.contains("stage=\"execution_success\""), "{output}");
        assert!(!output.contains("stage=\"execution_failed\""), "{output}");
    }
}
