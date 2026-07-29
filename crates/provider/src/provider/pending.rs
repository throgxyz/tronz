//! Pending transaction handle with confirmation polling.

use core::{future::Future, time::Duration};

use thiserror::Error;
use tronz_primitives::TxId;

use crate::{
    error::ProviderError,
    provider::{RootProvider, SolidityProvider, TronProvider},
    types::TransactionInfo,
};

/// Errors that can occur while waiting for a pending transaction to be confirmed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PendingTransactionError {
    /// A transport or provider error occurred while polling.
    #[error(transparent)]
    Transport(#[from] ProviderError),

    /// The timeout passed without the transaction being indexed.
    ///
    /// [`last_error`](Self::ConfirmationTimeout::last_error) carries the last thing
    /// that went wrong while polling, if anything did: a node that was unreachable
    /// throughout reads as a timeout, and this is where the reason for it survives.
    #[error("timed out waiting for transaction confirmation{}", .last_error.as_ref().map(|e| format!(": {e}")).unwrap_or_default())]
    ConfirmationTimeout {
        /// The last failure seen while polling.
        last_error: Option<Box<ProviderError>>,
    },

    /// The transaction was confirmed on-chain but execution did not succeed
    /// (e.g. reverted or ran out of energy). Carries the full receipt.
    #[error("transaction confirmed but execution failed: {:?}", .0.contract_result)]
    Reverted(Box<TransactionInfo>),
}

/// Whether a polling failure leaves room for the next attempt to succeed.
///
/// A node that is unreachable, busy, or slow may well answer the next time, and the
/// transaction is on the chain either way — so polling continues until the timeout
/// runs out. `DeadlineExceeded` counts here even though a single RPC does not retry
/// it: one slow call says nothing about the next, and it is the timeout that decides
/// when to stop rather than the transport.
///
/// A malformed answer or a node that refused the call will not change on its own, and
/// returns straight away.
pub(crate) fn is_worth_reasking(err: &ProviderError) -> bool {
    matches!(
        err.as_transport_err(),
        Some(crate::TransportErrorKind::Rpc { .. } | crate::TransportErrorKind::Connect(_))
    )
}

/// How often to re-ask the node, and for how long, unless reconfigured.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(3);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Handle to a broadcast transaction; can be awaited to confirmation.
///
/// The polling schedule and whether execution must have succeeded are set
/// independently of *where* confirmation is read from, so any combination is
/// available without a method per combination:
///
/// ```no_run
/// # use core::time::Duration;
/// # use tronz_provider::{PendingTransaction, PendingTransactionError};
/// # async fn run(pending: PendingTransaction) -> Result<(), PendingTransactionError> {
/// let receipt = pending
///     .with_poll_interval(Duration::from_secs(1))
///     .with_timeout(Duration::from_secs(30))
///     .require_success()
///     .get_receipt()
///     .await?;
/// # let _ = receipt;
/// # Ok(()) }
/// ```
pub struct PendingTransaction {
    provider: RootProvider,
    tx_id: TxId,
    interval: Duration,
    timeout: Duration,
    require_success: bool,
}

impl PendingTransaction {
    /// Construct a handle for an already-broadcast transaction id, polling every
    /// 3 s for up to 60 s and accepting any execution result.
    pub fn new(provider: RootProvider, tx_id: TxId) -> Self {
        Self {
            provider,
            tx_id,
            interval: DEFAULT_POLL_INTERVAL,
            timeout: DEFAULT_TIMEOUT,
            require_success: false,
        }
    }

    /// The broadcast transaction's id.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// How long to wait between polls. A TRON block is ~3 s, so polling much
    /// faster than that mostly costs requests.
    pub const fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// How long to keep polling before giving up with
    /// [`ConfirmationTimeout`](PendingTransactionError::ConfirmationTimeout).
    ///
    /// A wall clock, not a request count: a slow node spends this budget on its
    /// RPCs, and the call returns within roughly `timeout` either way.
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Treat a confirmed-but-failed transaction as an error, reported as
    /// [`Reverted`](PendingTransactionError::Reverted) with the full receipt.
    ///
    /// Off by default: the receipt is returned whatever the execution result, and
    /// [`TransactionInfo::is_success`] tells them apart.
    pub const fn require_success(mut self) -> Self {
        self.require_success = true;
        self
    }

    /// The configured poll interval.
    pub const fn poll_interval(&self) -> Duration {
        self.interval
    }

    /// The configured timeout.
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Whether a failed execution is treated as an error.
    pub const fn requires_success(&self) -> bool {
        self.require_success
    }

    /// Poll a FullNode until the transaction is indexed, then return its receipt.
    ///
    /// FullNode inclusion is not final; use
    /// [`get_solidified_receipt`](Self::get_solidified_receipt) to wait for
    /// irreversible state.
    pub async fn get_receipt(self) -> Result<TransactionInfo, PendingTransactionError> {
        let mut last_error = None;
        let poll = async {
            loop {
                match self.provider.get_transaction_info(self.tx_id).await {
                    Ok(Some(info)) => return Ok(info),
                    Ok(None) => tokio::time::sleep(self.interval).await,
                    // A node that cannot be reached says nothing about the
                    // transaction, and the timeout is what bounds the wait — so keep
                    // asking until it runs out, and keep the reason in case it does.
                    Err(err) if is_worth_reasking(&err) => {
                        last_error = Some(Box::new(err));
                        tokio::time::sleep(self.interval).await;
                    }
                    Err(err) => return Err(PendingTransactionError::Transport(err)),
                }
            }
        };

        match tokio::time::timeout(self.timeout, poll).await {
            Ok(result) => self.check(result?),
            Err(_elapsed) => Err(PendingTransactionError::ConfirmationTimeout { last_error }),
        }
    }

    /// Poll a [`SolidityProvider`] until the transaction has solidified, then
    /// return its receipt.
    ///
    /// Solidified state is irreversible, so this is the one to wait for when a
    /// reorg would matter. It trails the head by roughly a minute.
    pub async fn get_solidified_receipt(
        &self,
        solidity: &SolidityProvider,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        let poll = solidity.wait_for_transaction_with(self.tx_id, self.interval, u32::MAX);
        self.deadline(poll).await
    }

    /// Run `poll` under the configured timeout, as a wall clock: the RPCs and the
    /// waits between them are all spent from the same budget.
    async fn deadline(
        &self,
        poll: impl Future<Output = Result<TransactionInfo, PendingTransactionError>>,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        match tokio::time::timeout(self.timeout, poll).await {
            Ok(result) => self.check(result?),
            Err(_elapsed) => Err(PendingTransactionError::ConfirmationTimeout { last_error: None }),
        }
    }

    fn check(&self, info: TransactionInfo) -> Result<TransactionInfo, PendingTransactionError> {
        if self.require_success && !info.is_success() {
            return Err(PendingTransactionError::Reverted(Box::new(info)));
        }
        Ok(info)
    }
}

/// Superseded by the configurable [`PendingTransaction`] above. Each of these
/// spells out one point in the interval × timeout × success × finality space that
/// the setters now cover.
impl PendingTransaction {
    /// Renamed, to match alloy and to read the same as the solidified variant.
    #[deprecated(since = "0.5.0", note = "renamed to `get_receipt`")]
    pub async fn await_confirmed(self) -> Result<TransactionInfo, PendingTransactionError> {
        self.get_receipt().await
    }

    /// Replaced by `with_poll_interval` and `with_timeout`.
    #[deprecated(since = "0.5.0", note = "use `with_poll_interval` and `with_timeout`")]
    pub async fn await_confirmed_with(
        self,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        self.with_poll_interval(interval).with_attempts(max_attempts).get_receipt().await
    }

    /// Replaced by `require_success`.
    #[deprecated(since = "0.5.0", note = "use `require_success().get_receipt()`")]
    pub async fn await_success(self) -> Result<TransactionInfo, PendingTransactionError> {
        self.require_success().get_receipt().await
    }

    /// Renamed to `get_solidified_receipt`.
    #[deprecated(since = "0.5.0", note = "renamed to `get_solidified_receipt`")]
    pub async fn await_solidified(
        &self,
        solidity: &SolidityProvider,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        self.get_solidified_receipt(solidity).await
    }

    /// Replaced by `with_poll_interval` and `with_timeout`.
    #[deprecated(since = "0.5.0", note = "use `with_poll_interval` and `with_timeout`")]
    pub async fn await_solidified_with(
        &self,
        solidity: &SolidityProvider,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        solidity.wait_for_transaction_with(self.tx_id, interval, max_attempts).await
    }

    /// Replaced by `require_success`.
    #[deprecated(since = "0.5.0", note = "use `require_success().get_solidified_receipt()`")]
    pub async fn await_solidified_success(
        &self,
        solidity: &SolidityProvider,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        solidity.wait_for_success(self.tx_id).await
    }

    /// Replaced by the setters.
    #[deprecated(since = "0.5.0", note = "use the `with_*` setters and `require_success`")]
    pub async fn await_solidified_success_with(
        &self,
        solidity: &SolidityProvider,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        solidity.wait_for_success_with(self.tx_id, interval, max_attempts).await
    }

    /// Bridges the deprecated attempt-based callers onto the timeout knob.
    const fn with_attempts(self, attempts: u32) -> Self {
        let timeout = Duration::from_millis(self.interval.as_millis() as u64 * attempts as u64);
        self.with_timeout(timeout)
    }
}

#[cfg(test)]
mod tests {

    use tronz_rpc_types::test_utils::transaction_info as info;

    use super::*;
    use crate::{provider::RootProvider, transport::mock::MockTransport, types::TxStatus};
    fn pending(replies: Vec<Option<TransactionInfo>>) -> PendingTransaction {
        let transport = MockTransport::new();
        for reply in replies {
            transport.push_ok("get_transaction_info", reply);
        }
        PendingTransaction::new(RootProvider::new(transport), TxId::ZERO)
            .with_poll_interval(Duration::from_millis(1))
    }
    #[tokio::test]
    async fn the_timeout_bounds_the_total_wait() {
        let started = std::time::Instant::now();

        let err = pending(vec![None])
            .with_poll_interval(Duration::from_secs(30))
            .with_timeout(Duration::from_millis(50))
            .get_receipt()
            .await
            .unwrap_err();

        assert!(matches!(err, PendingTransactionError::ConfirmationTimeout { .. }));
        assert!(started.elapsed() < Duration::from_secs(5), "waited {:?}", started.elapsed());
    }
    #[test]
    fn an_attempt_count_becomes_the_equivalent_timeout() {
        let handle = pending(vec![]).with_poll_interval(Duration::from_secs(3)).with_attempts(20);

        assert_eq!(handle.timeout(), Duration::from_secs(60));
    }

    #[tokio::test]
    async fn polling_continues_until_the_node_has_indexed_the_transaction() {
        let handle = pending(vec![None, None, Some(info(TxStatus::Success))]);
        assert_eq!(handle.get_receipt().await.unwrap().block_number, 1);
    }

    #[tokio::test]
    async fn a_failed_execution_is_returned_as_a_receipt_by_default() {
        let handle = pending(vec![Some(info(TxStatus::Failed))]);
        let receipt = handle.get_receipt().await.unwrap();
        assert!(!receipt.is_success());
    }

    #[tokio::test]
    async fn require_success_turns_a_failed_execution_into_an_error() {
        let handle = pending(vec![Some(info(TxStatus::Failed))]).require_success();
        let err = handle.get_receipt().await.unwrap_err();
        assert!(matches!(err, PendingTransactionError::Reverted(_)));
    }

    #[test]
    fn the_setters_are_readable_back() {
        let handle = pending(vec![])
            .with_poll_interval(Duration::from_secs(1))
            .with_timeout(Duration::from_secs(5))
            .require_success();

        assert_eq!(handle.poll_interval(), Duration::from_secs(1));
        assert_eq!(handle.timeout(), Duration::from_secs(5));
        assert!(handle.requires_success());
    }
    #[tokio::test]
    async fn a_node_that_stumbles_is_asked_again() {
        let transport = MockTransport::new();
        transport.push_err::<Option<TransactionInfo>>(
            "get_transaction_info",
            crate::TransportErrorKind::rpc(crate::RpcStatusCode::Unavailable, "busy"),
        );
        transport.push_ok("get_transaction_info", Some(info(TxStatus::Success)));
        let provider = RootProvider::new(transport);

        let receipt = PendingTransaction::new(provider, TxId::ZERO)
            .with_poll_interval(Duration::from_millis(1))
            .get_receipt()
            .await
            .expect("the second ask succeeded");

        assert!(receipt.is_success());
    }
    #[tokio::test]
    async fn a_malformed_answer_is_returned_at_once() {
        let transport = MockTransport::new();
        transport.push_err::<Option<TransactionInfo>>(
            "get_transaction_info",
            crate::TransportErrorKind::Malformed("nonsense".into()),
        );
        let provider = RootProvider::new(transport);

        let err = PendingTransaction::new(provider, TxId::ZERO)
            .with_poll_interval(Duration::from_millis(1))
            .get_receipt()
            .await
            .expect_err("nothing to wait for");

        assert!(matches!(err, PendingTransactionError::Transport(_)), "{err}");
    }
    #[tokio::test]
    async fn a_timeout_carries_the_last_thing_that_went_wrong() {
        let transport = MockTransport::new();
        for _ in 0..50 {
            transport.push_err::<Option<TransactionInfo>>(
                "get_transaction_info",
                crate::TransportErrorKind::rpc(crate::RpcStatusCode::Unavailable, "still busy"),
            );
        }
        let provider = RootProvider::new(transport);

        let err = PendingTransaction::new(provider, TxId::ZERO)
            .with_poll_interval(Duration::from_millis(10))
            .with_timeout(Duration::from_millis(100))
            .get_receipt()
            .await
            .expect_err("never confirmed");

        let PendingTransactionError::ConfirmationTimeout { last_error } = err else {
            panic!("expected a timeout, got {err}");
        };
        assert!(last_error.expect("a reason").to_string().contains("still busy"));
    }
}
