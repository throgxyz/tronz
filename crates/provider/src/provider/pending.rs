//! Pending transaction handle with confirmation polling.

use core::time::Duration;

use thiserror::Error;
use tronz_primitives::TxId;

use crate::{
    error::ProviderError,
    provider::{SolidityProvider, TronProvider},
    transport::SolidityTransport,
    types::TransactionInfo,
};

/// Errors that can occur while waiting for a pending transaction to be confirmed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PendingTransactionError {
    /// A transport or provider error occurred while polling.
    #[error(transparent)]
    Transport(#[from] ProviderError),

    /// Polling exhausted all attempts without the transaction being indexed.
    #[error("timed out waiting for transaction confirmation")]
    ConfirmationTimeout,

    /// The transaction was confirmed on-chain but execution did not succeed
    /// (e.g. reverted or ran out of energy). Carries the full receipt.
    #[error("transaction confirmed but execution failed: {:?}", .0.contract_result)]
    Reverted(Box<TransactionInfo>),
}

/// Handle to a broadcast transaction; can be awaited to confirmation.
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

    /// Poll until the transaction is confirmed. Defaults to every 3 s, up to
    /// 20 attempts (~60 s total).
    pub async fn await_confirmed(self) -> Result<TransactionInfo, PendingTransactionError> {
        self.await_confirmed_with(Duration::from_secs(3), 20).await
    }

    /// Alias for [`await_confirmed`](Self::await_confirmed) — mirrors alloy's
    /// `PendingTransactionBuilder::get_receipt`.
    pub async fn get_receipt(self) -> Result<TransactionInfo, PendingTransactionError> {
        self.await_confirmed().await
    }

    /// Poll for confirmation with a custom interval and attempt count.
    ///
    /// Returns once indexed, regardless of execution result. Use
    /// [`await_success`](Self::await_success) to require success.
    pub async fn await_confirmed_with(
        self,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        for attempt in 0..max_attempts {
            if attempt > 0 {
                tokio::time::sleep(interval).await;
            }
            match self.provider.get_transaction_info(self.tx_id).await {
                Ok(Some(info)) => return Ok(info),
                Ok(None) => continue,
                Err(e) => return Err(PendingTransactionError::Transport(e)),
            }
        }
        Err(PendingTransactionError::ConfirmationTimeout)
    }

    /// Like [`await_confirmed`](Self::await_confirmed), but additionally fails
    /// with [`PendingTransactionError::Reverted`] if the transaction was
    /// confirmed but its on-chain execution did not succeed.
    pub async fn await_success(self) -> Result<TransactionInfo, PendingTransactionError> {
        let info = self.await_confirmed().await?;
        if info.is_success() {
            Ok(info)
        } else {
            Err(PendingTransactionError::Reverted(Box::new(info)))
        }
    }

    /// Poll a [`SolidityProvider`] until this transaction has solidified.
    ///
    /// Unlike [`await_confirmed`](Self::await_confirmed), which only requires
    /// FullNode inclusion, this waits for irreversible state and returns the
    /// receipt regardless of execution result.
    pub async fn await_solidified<S: SolidityTransport>(
        &self,
        solidity: &SolidityProvider<S>,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        solidity.wait_for_transaction(self.tx_id).await
    }

    /// [`await_solidified`](Self::await_solidified) with a custom interval and
    /// attempt count.
    pub async fn await_solidified_with<S: SolidityTransport>(
        &self,
        solidity: &SolidityProvider<S>,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        solidity.wait_for_transaction_with(self.tx_id, interval, max_attempts).await
    }

    /// Like [`await_solidified`](Self::await_solidified), but additionally fails
    /// with [`PendingTransactionError::Reverted`] if the solidified transaction
    /// did not execute successfully.
    pub async fn await_solidified_success<S: SolidityTransport>(
        &self,
        solidity: &SolidityProvider<S>,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        solidity.wait_for_success(self.tx_id).await
    }

    /// [`await_solidified_success`](Self::await_solidified_success) with a custom
    /// interval and attempt count.
    pub async fn await_solidified_success_with<S: SolidityTransport>(
        &self,
        solidity: &SolidityProvider<S>,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        solidity.wait_for_success_with(self.tx_id, interval, max_attempts).await
    }
}
