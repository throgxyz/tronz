//! Pending transaction handle with confirmation polling.

use core::time::Duration;

use thiserror::Error;
use tronz_primitives::TxId;

use crate::{error::ProviderError, provider::TronProvider, types::TransactionInfo};

/// Errors that can occur while waiting for a pending transaction to be confirmed.
///
/// Separating polling errors from [`ProviderError`] keeps the two concerns
/// orthogonal: transport failures during polling are wrapped in [`Transport`],
/// while a clean timeout is its own variant.
///
/// [`Transport`]: PendingTransactionError::Transport
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PendingTransactionError {
    /// A transport or provider error occurred while polling.
    #[error(transparent)]
    Transport(#[from] ProviderError),

    /// Polling exhausted all attempts without the transaction being indexed.
    #[error("timed out waiting for transaction confirmation")]
    ConfirmationTimeout,
}

/// Handle to a broadcast transaction; can be awaited to confirmation.
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
    pub async fn await_confirmed_with(
        self,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo, PendingTransactionError> {
        for _ in 0..max_attempts {
            tokio::time::sleep(interval).await;
            match self.provider.get_transaction_info(self.tx_id).await {
                Ok(Some(info)) => return Ok(info),
                // Not yet indexed — keep polling.
                Ok(None) => continue,
                Err(e) => return Err(PendingTransactionError::Transport(e)),
            }
        }
        Err(PendingTransactionError::ConfirmationTimeout)
    }
}
