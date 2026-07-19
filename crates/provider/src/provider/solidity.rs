//! Read-only provider over a TRON SolidityNode.

use core::time::Duration;
use std::sync::Arc;

use tronz_primitives::{Address, ResourceCode, Trx, TxId};

use crate::{
    Result,
    error::ProviderError,
    provider::{ContractReadProvider, pending::PendingTransactionError},
    transport::{
        SolidityTransport,
        grpc::{RetryConfig, SolidityGrpcTransport, SolidityGrpcTransportBuilder},
    },
    types::{
        AccountInfo, BlockInfo, ConstantCallResult, DelegatedResource, DelegatedResourceIndex,
        SignedTransaction, TransactionInfo, TriggerSmartContract, WitnessInfo,
    },
};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(3);
const DEFAULT_POLL_ATTEMPTS: u32 = 20;

/// A read-only provider over `protocol.WalletSolidity`.
#[derive(Clone)]
pub struct SolidityProvider<T: SolidityTransport = SolidityGrpcTransport> {
    inner: Arc<T>,
}

impl<T: SolidityTransport> crate::provider::private::ContractReadSealed for SolidityProvider<T> {}

impl<T: SolidityTransport> ContractReadProvider for SolidityProvider<T> {
    async fn call_contract(&self, params: TriggerSmartContract) -> Result<ConstantCallResult> {
        SolidityProvider::trigger_constant_contract(self, params).await
    }

    async fn estimate_contract_energy(&self, params: TriggerSmartContract) -> Result<i64> {
        SolidityProvider::estimate_energy(self, params).await
    }

    async fn transaction_info(&self, tx_id: TxId) -> Result<Option<TransactionInfo>> {
        SolidityProvider::get_transaction_info(self, tx_id).await
    }

    async fn transaction_infos_by_block(&self, block_num: i64) -> Result<Vec<TransactionInfo>> {
        SolidityProvider::get_transaction_info_by_block_num(self, block_num).await
    }
}

impl<T: SolidityTransport> SolidityProvider<T> {
    /// Wrap an existing [`SolidityTransport`].
    pub fn new(transport: T) -> Self {
        Self { inner: Arc::new(transport) }
    }

    /// Borrow the underlying transport.
    pub fn transport(&self) -> &T {
        &self.inner
    }

    /// Fetch the latest solidified block.
    pub async fn get_now_block(&self) -> Result<BlockInfo> {
        self.inner.get_now_block().await.map_err(ProviderError::transport)
    }

    /// Fetch a solidified block by height.
    pub async fn get_block_by_number(&self, num: i64) -> Result<BlockInfo> {
        self.inner.get_block_by_number(num).await.map_err(ProviderError::transport)
    }

    /// Fetch solidified account state.
    pub async fn get_account(&self, address: Address) -> Result<AccountInfo> {
        self.inner.get_account(address).await.map_err(ProviderError::transport)
    }

    /// Fetch a transaction by id from solidified state.
    pub async fn get_transaction(&self, tx_id: TxId) -> Result<SignedTransaction> {
        self.inner.get_transaction_by_id(tx_id).await.map_err(ProviderError::transport)
    }

    /// Fetch a transaction's receipt from solidified state.
    ///
    /// Returns `None` until the transaction has solidified — this is the signal
    /// [`wait_for_transaction`](Self::wait_for_transaction) polls on.
    pub async fn get_transaction_info(&self, tx_id: TxId) -> Result<Option<TransactionInfo>> {
        self.inner.get_transaction_info(tx_id).await.map_err(ProviderError::transport)
    }

    /// Fetch all transaction receipts in a solidified block.
    pub async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> Result<Vec<TransactionInfo>> {
        self.inner
            .get_transaction_info_by_block_num(block_num)
            .await
            .map_err(ProviderError::transport)
    }

    /// Count transactions in a solidified block by block number.
    pub async fn get_transaction_count_by_block_num(&self, block_num: i64) -> Result<u64> {
        self.inner
            .get_transaction_count_by_block_num(block_num)
            .await
            .map_err(ProviderError::transport)
    }

    /// Execute a constant (read-only) contract call against solidified state.
    pub async fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> Result<ConstantCallResult> {
        self.inner.trigger_constant_contract(params).await.map_err(ProviderError::transport)
    }

    /// Estimate the energy a contract call would consume against solidified state.
    pub async fn estimate_energy(&self, params: TriggerSmartContract) -> Result<i64> {
        self.inner.estimate_energy(params).await.map_err(ProviderError::transport)
    }

    /// List all super representatives and candidates from solidified state.
    pub async fn list_witnesses(&self) -> Result<Vec<WitnessInfo>> {
        self.inner.list_witnesses().await.map_err(ProviderError::transport)
    }

    /// Fetch a paginated list of witnesses sorted by real-time vote count.
    pub async fn get_paginated_now_witness_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<WitnessInfo>> {
        self.inner
            .get_paginated_now_witness_list(offset, limit)
            .await
            .map_err(ProviderError::transport)
    }

    /// Query delegations between two accounts from solidified state (Stake 1.0, legacy).
    pub async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>> {
        self.inner.get_delegated_resource_v1(from, to).await.map_err(ProviderError::transport)
    }

    /// Query the delegation index for an account from solidified state (Stake 1.0, legacy).
    pub async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex> {
        self.inner.get_delegated_resource_index_v1(address).await.map_err(ProviderError::transport)
    }

    /// Query delegations between two accounts from solidified state (Stake 2.0).
    pub async fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>> {
        self.inner.get_delegated_resource(from, to).await.map_err(ProviderError::transport)
    }

    /// Query the delegation index for an account from solidified state (Stake 2.0).
    pub async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex> {
        self.inner.get_delegated_resource_index(address).await.map_err(ProviderError::transport)
    }

    /// Query the max amount still delegatable for a resource from solidified state.
    pub async fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> Result<Trx> {
        self.inner.get_can_delegate_max(address, resource).await.map_err(ProviderError::transport)
    }

    /// Query how many unfreeze operations are still available from solidified state.
    pub async fn get_available_unfreeze_count(&self, address: Address) -> Result<i64> {
        self.inner.get_available_unfreeze_count(address).await.map_err(ProviderError::transport)
    }

    /// Query the amount withdrawable at a timestamp from solidified state.
    pub async fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> Result<Trx> {
        self.inner
            .get_can_withdraw_unfreeze_amount(address, timestamp_ms)
            .await
            .map_err(ProviderError::transport)
    }

    /// Poll until `tx_id` has solidified, regardless of execution result.
    ///
    /// Defaults to every 3 s, up to 20 attempts (~60 s).
    pub async fn wait_for_transaction(
        &self,
        tx_id: TxId,
    ) -> std::result::Result<TransactionInfo, PendingTransactionError> {
        self.wait_for_transaction_with(tx_id, DEFAULT_POLL_INTERVAL, DEFAULT_POLL_ATTEMPTS).await
    }

    /// Poll for solidification with a custom interval and attempt count.
    pub async fn wait_for_transaction_with(
        &self,
        tx_id: TxId,
        interval: Duration,
        max_attempts: u32,
    ) -> std::result::Result<TransactionInfo, PendingTransactionError> {
        for attempt in 0..max_attempts {
            if attempt > 0 {
                tokio::time::sleep(interval).await;
            }
            match self.get_transaction_info(tx_id).await {
                Ok(Some(info)) => return Ok(info),
                Ok(None) => continue,
                Err(e) => return Err(PendingTransactionError::Transport(e)),
            }
        }
        Err(PendingTransactionError::ConfirmationTimeout)
    }

    /// Poll until `tx_id` has solidified and its execution succeeded.
    ///
    /// Fails with [`PendingTransactionError::Reverted`] (carrying the receipt)
    /// if the transaction solidified but reverted / ran out of energy.
    pub async fn wait_for_success(
        &self,
        tx_id: TxId,
    ) -> std::result::Result<TransactionInfo, PendingTransactionError> {
        self.wait_for_success_with(tx_id, DEFAULT_POLL_INTERVAL, DEFAULT_POLL_ATTEMPTS).await
    }

    /// Poll for solidified success with a custom interval and attempt count.
    pub async fn wait_for_success_with(
        &self,
        tx_id: TxId,
        interval: Duration,
        max_attempts: u32,
    ) -> std::result::Result<TransactionInfo, PendingTransactionError> {
        let info = self.wait_for_transaction_with(tx_id, interval, max_attempts).await?;
        if info.is_success() {
            Ok(info)
        } else {
            Err(PendingTransactionError::Reverted(Box::new(info)))
        }
    }
}

impl SolidityProvider<SolidityGrpcTransport> {
    /// Connect with the default transport configuration.
    ///
    /// Use [`builder`](Self::builder) to customize it.
    pub async fn connect(uri: impl AsRef<str>) -> Result<Self> {
        let transport = SolidityGrpcTransport::connect(uri).await.map_err(ProviderError::from)?;
        Ok(Self::new(transport))
    }

    /// Start a pre-connect [`SolidityProviderBuilder`].
    pub fn builder() -> SolidityProviderBuilder {
        SolidityProviderBuilder::default()
    }
}

/// Pre-connect builder for a [`SolidityProvider`] over gRPC.
#[derive(Clone, Debug, Default)]
pub struct SolidityProviderBuilder {
    inner: SolidityGrpcTransportBuilder,
}

impl SolidityProviderBuilder {
    /// Override the connect (handshake) timeout.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.with_connect_timeout(timeout);
        self
    }

    /// Override the per-call request timeout.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.with_request_timeout(timeout);
        self
    }

    /// Override the retry policy.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.inner = self.inner.with_retry(retry);
        self
    }

    /// Add equivalent SolidityNode endpoints for client-side failover.
    pub fn with_endpoints(mut self, endpoints: Vec<String>) -> Self {
        self.inner = self.inner.with_endpoints(endpoints);
        self
    }

    /// Optionally set the TronGrid API key.
    pub fn maybe_api_key(mut self, key: Option<impl Into<String>>) -> Self {
        self.inner = self.inner.maybe_api_key(key);
        self
    }

    /// Connect using the accumulated configuration.
    pub async fn connect(
        self,
        uri: impl AsRef<str>,
    ) -> Result<SolidityProvider<SolidityGrpcTransport>> {
        let transport = self.inner.connect(uri).await.map_err(ProviderError::from)?;
        Ok(SolidityProvider::new(transport))
    }
}

#[cfg(test)]
mod tests {
    use tronz_primitives::Trx;

    use super::*;
    use crate::{
        error::TransportErrorKind,
        transport::mock::MockSolidityTransport,
        types::{ContractResult, TxStatus},
    };

    const NEAR_INSTANT: Duration = Duration::from_millis(1);

    fn info(status: TxStatus) -> TransactionInfo {
        TransactionInfo {
            tx_id: TxId::ZERO,
            block_number: 1,
            block_timestamp: 1,
            status,
            energy_usage: 0,
            energy_fee: Trx::ZERO,
            net_usage: 0,
            net_fee: Trx::ZERO,
            contract_result: ContractResult::Default,
            contract_address: None,
            logs: vec![],
            revert_reason: None,
        }
    }

    fn provider(mock: MockSolidityTransport) -> SolidityProvider<MockSolidityTransport> {
        SolidityProvider::new(mock)
    }

    #[tokio::test]
    async fn contract_read_capability_dispatches_receipt_queries() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Option<TransactionInfo>>(
            "get_transaction_info",
            Some(info(TxStatus::Success)),
        );

        let receipt =
            ContractReadProvider::transaction_info(&provider(mock), TxId::ZERO).await.unwrap();
        assert!(receipt.is_some_and(|info| info.is_success()));
    }

    fn witness(vote_count: i64) -> WitnessInfo {
        WitnessInfo {
            address: Address::ZERO,
            vote_count,
            url: "https://sr.example".to_string(),
            total_produced: 0,
            total_missed: 0,
            is_active: true,
        }
    }

    #[tokio::test]
    async fn list_witnesses_returns_solidified_witnesses() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Vec<WitnessInfo>>("list_witnesses", vec![witness(10), witness(20)]);

        let witnesses = provider(mock).list_witnesses().await.unwrap();
        assert_eq!(witnesses.len(), 2);
        assert_eq!(witnesses[1].vote_count, 20);
    }

    #[tokio::test]
    async fn get_paginated_now_witness_list_returns_witnesses() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Vec<WitnessInfo>>("get_paginated_now_witness_list", vec![witness(99)]);

        let witnesses = provider(mock).get_paginated_now_witness_list(0, 10).await.unwrap();
        assert_eq!(witnesses.len(), 1);
        assert_eq!(witnesses[0].vote_count, 99);
    }

    fn delegation(bandwidth: i64, energy: i64) -> DelegatedResource {
        DelegatedResource {
            from: Address::ZERO,
            to: Address::ZERO,
            bandwidth_amount: Trx::from_sun_unchecked(bandwidth),
            energy_amount: Trx::from_sun_unchecked(energy),
            bandwidth_expire_time_ms: 0,
            energy_expire_time_ms: 0,
        }
    }

    #[tokio::test]
    async fn get_delegated_resource_returns_solidified_delegations() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Vec<DelegatedResource>>(
            "get_delegated_resource",
            vec![delegation(1_000_000, 2_000_000)],
        );

        let delegations =
            provider(mock).get_delegated_resource(Address::ZERO, Address::ZERO).await.unwrap();
        assert_eq!(delegations.len(), 1);
        assert_eq!(delegations[0].energy_amount, Trx::from_sun_unchecked(2_000_000));
    }

    #[tokio::test]
    async fn get_can_delegate_max_returns_amount() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Trx>("get_can_delegate_max", Trx::from_sun_unchecked(5_000_000));

        let max =
            provider(mock).get_can_delegate_max(Address::ZERO, ResourceCode::Energy).await.unwrap();
        assert_eq!(max, Trx::from_sun_unchecked(5_000_000));
    }

    #[tokio::test]
    async fn get_available_unfreeze_count_returns_count() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<i64>("get_available_unfreeze_count", 3);

        let count = provider(mock).get_available_unfreeze_count(Address::ZERO).await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn wait_for_transaction_polls_past_none_then_returns_receipt() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Option<TransactionInfo>>("get_transaction_info", None).push_ok::<Option<
            TransactionInfo,
        >>(
            "get_transaction_info",
            Some(info(TxStatus::Failed)),
        );

        let receipt =
            provider(mock).wait_for_transaction_with(TxId::ZERO, NEAR_INSTANT, 5).await.unwrap();
        assert!(!receipt.is_success());
    }

    #[tokio::test]
    async fn wait_for_success_rejects_reverted_and_preserves_receipt() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Option<TransactionInfo>>(
            "get_transaction_info",
            Some(info(TxStatus::Failed)),
        );

        let err =
            provider(mock).wait_for_success_with(TxId::ZERO, NEAR_INSTANT, 5).await.unwrap_err();
        match err {
            PendingTransactionError::Reverted(receipt) => assert!(!receipt.is_success()),
            other => panic!("expected Reverted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn wait_for_success_returns_successful_receipt() {
        let mock = MockSolidityTransport::new();
        mock.push_ok::<Option<TransactionInfo>>(
            "get_transaction_info",
            Some(info(TxStatus::Success)),
        );

        let receipt =
            provider(mock).wait_for_success_with(TxId::ZERO, NEAR_INSTANT, 5).await.unwrap();
        assert!(receipt.is_success());
    }

    #[tokio::test]
    async fn wait_for_transaction_times_out_when_never_solidified() {
        let mock = MockSolidityTransport::new();
        for _ in 0..3 {
            mock.push_ok::<Option<TransactionInfo>>("get_transaction_info", None);
        }

        let err = provider(mock)
            .wait_for_transaction_with(TxId::ZERO, NEAR_INSTANT, 3)
            .await
            .unwrap_err();
        assert!(matches!(err, PendingTransactionError::ConfirmationTimeout));
    }

    #[tokio::test]
    async fn zero_attempts_times_out_without_polling() {
        let err = provider(MockSolidityTransport::new())
            .wait_for_transaction_with(TxId::ZERO, NEAR_INSTANT, 0)
            .await
            .unwrap_err();
        assert!(matches!(err, PendingTransactionError::ConfirmationTimeout));
    }

    #[tokio::test]
    async fn transport_error_propagates() {
        let mock = MockSolidityTransport::new();
        mock.push_err::<Option<TransactionInfo>>(
            "get_transaction_info",
            TransportErrorKind::Malformed("boom".to_owned()),
        );

        let err = provider(mock)
            .wait_for_transaction_with(TxId::ZERO, NEAR_INSTANT, 5)
            .await
            .unwrap_err();
        assert!(matches!(err, PendingTransactionError::Transport(_)));
    }
}
