use core::{fmt::Display, time::Duration};

use anyhow::{Context, Result, ensure};
use tronz::{
    Trx,
    providers::{PendingTransaction, types::TransactionInfo},
};

const DEFAULT_FULL_NODE_GRPC_ENDPOINT: &str = "http://127.0.0.1:50051";
const DEFAULT_SOLIDITY_NODE_GRPC_ENDPOINT: &str = "http://127.0.0.1:50052";

// TRE mines on demand, so a sub-second poll keeps the suite responsive while the
// timeout still tolerates a slow container under CI load.
const CONFIRMATION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const DEPLOY_FEE_LIMIT: Trx = Trx::from_sun_unchecked(1_000_000_000);

pub(crate) const REVERTING_CALL_FEE_LIMIT: Trx = Trx::from_sun_unchecked(100_000_000);

pub(crate) fn full_node_grpc_endpoint() -> String {
    std::env::var("TRONZ_FULL_NODE_GRPC_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_FULL_NODE_GRPC_ENDPOINT.to_owned())
}

pub(crate) fn solidity_node_grpc_endpoint() -> String {
    std::env::var("TRONZ_SOLIDITY_NODE_GRPC_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_SOLIDITY_NODE_GRPC_ENDPOINT.to_owned())
}

pub(crate) fn configure_pending_transaction(pending: PendingTransaction) -> PendingTransaction {
    pending.with_poll_interval(CONFIRMATION_POLL_INTERVAL).with_timeout(CONFIRMATION_TIMEOUT)
}

pub(crate) async fn wait_for_confirmed_transaction(
    pending: PendingTransaction,
) -> Result<TransactionInfo> {
    configure_pending_transaction(pending)
        .require_success()
        .get_receipt()
        .await
        .context("transaction confirmation")
}

pub(crate) async fn wait_for_transaction_receipt(
    pending: PendingTransaction,
) -> Result<TransactionInfo> {
    configure_pending_transaction(pending).get_receipt().await.context("transaction receipt")
}

pub(crate) fn assert_node_error_contains<T, E: Display>(
    result: std::result::Result<T, E>,
    context: &str,
    expected: &str,
) -> Result<()> {
    let Err(error) = result else {
        anyhow::bail!("{context}: node unexpectedly accepted the request");
    };
    let error = error.to_string();
    ensure!(error.contains(expected), "{context}: unexpected node error: {error}");
    Ok(())
}
