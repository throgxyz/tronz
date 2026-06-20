//! In-memory [`MockTransport`] for testing provider logic without a live node.
//!
//! Available under the `mock` feature (and in this crate's own tests). Because
//! [`TronTransport`] is sealed, downstream crates cannot hand-roll their own
//! mock — this is the supported way to exercise [`crate::provider::RootProvider`] /
//! [`crate::provider::FilledProvider`] against canned responses.
//!
//! Each method has its own FIFO queue of typed responses, keyed by method name.
//! Push a response with [`push_ok`](MockTransport::push_ok) /
//! [`push_err`](MockTransport::push_err), then call the matching method through
//! a provider; the mock pops the next queued response (panicking if none was
//! queued or the type does not match).
//!
//! ```
//! # use tronz_provider::transport::{TronTransport, mock::MockTransport};
//! # tokio_test_block(async {
//! let mock = MockTransport::new();
//! mock.push_ok::<u64>("get_memo_fee", 1_000_000);
//! assert_eq!(mock.get_memo_fee().await.unwrap(), 1_000_000);
//! # });
//! # fn tokio_test_block<F: std::future::Future>(_f: F) {}
//! ```

use std::{
    any::{Any, type_name},
    collections::{HashMap, VecDeque},
    future::Future,
    sync::{Arc, Mutex},
};

use tronz_primitives::{Address, B256, ResourceCode, Trx, TxId};

use crate::{
    error::TransportErrorKind,
    transport::TronTransport,
    types::{
        AccountInfo, AccountNet, AccountPermissionUpdateContract, AccountResource, AssetInfo,
        AssetIssueContract, BlockInfo, CancelAllUnfreezeV2Contract, ChainProperties,
        ClearContractAbiContract, ConstantCallResult, CreateAccountContract, CreateSmartContract,
        CreateWitnessContract, DelegateResourceContract, DelegatedResource, DelegatedResourceIndex,
        ExchangeCreateContract, ExchangeInfo, ExchangeInjectContract, ExchangeTransactionContract,
        ExchangeWithdrawContract, FreezeBalanceV1Contract, FreezeBalanceV2Contract,
        MarketCancelOrderContract, MarketOrderInfo, MarketOrderPair, MarketPrice,
        MarketSellAssetContract, NodeAddress, NodeInfo, ParticipateAssetIssueContract,
        ProposalApproveContract, ProposalCreateContract, ProposalDeleteContract, ProposalInfo,
        RawTransaction, SetAccountIdContract, SignWeight, SignedTransaction, SmartContractInfo,
        TransactionInfo, TransferAssetContract, TransferContract, TriggerSmartContract,
        UnDelegateResourceContract, UnfreezeAssetContract, UnfreezeBalanceV1Contract,
        UnfreezeBalanceV2Contract, UpdateAccountContract, UpdateAssetContract,
        UpdateBrokerageContract, UpdateEnergyLimitContract, UpdateSettingContract,
        UpdateWitnessContract, VoteWitnessContract, WithdrawBalanceContract,
        WithdrawExpireUnfreezeContract, WitnessInfo,
    },
};

/// A boxed, type-erased `Result<T, TransportErrorKind>` for a single response.
type Entry = Box<dyn Any + Send>;

/// An in-memory [`TronTransport`] backed by per-method response queues.
///
/// Cheap to clone — clones share the same underlying queues, so a clone handed
/// to a provider still sees responses pushed through the original handle.
#[derive(Clone, Default)]
pub struct MockTransport {
    responses: Arc<Mutex<HashMap<&'static str, VecDeque<Entry>>>>,
}

impl MockTransport {
    /// Create an empty mock with no queued responses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a successful response for `method`.
    ///
    /// `T` must be the method's `Ok` type; a mismatch panics when the method is
    /// later invoked. Returns `&self` for chaining.
    pub fn push_ok<T: Send + 'static>(&self, method: &'static str, value: T) -> &Self {
        self.push_result::<T>(method, Ok(value))
    }

    /// Queue an error response for `method`.
    ///
    /// `T` is the method's `Ok` type (needed to match the method's return
    /// signature); the queued value is the `Err`. Returns `&self` for chaining.
    pub fn push_err<T: Send + 'static>(
        &self,
        method: &'static str,
        err: TransportErrorKind,
    ) -> &Self {
        self.push_result::<T>(method, Err(err))
    }

    fn push_result<T: Send + 'static>(
        &self,
        method: &'static str,
        result: Result<T, TransportErrorKind>,
    ) -> &Self {
        self.responses
            .lock()
            .expect("MockTransport mutex poisoned")
            .entry(method)
            .or_default()
            .push_back(Box::new(result));
        self
    }

    /// Pop the next queued response for `method`, downcasting to its type.
    ///
    /// Panics if the queue is empty or the queued type does not match `T` —
    /// both indicate a test that did not set up the mock correctly.
    fn pop<T: 'static>(&self, method: &'static str) -> Result<T, TransportErrorKind> {
        let entry = self
            .responses
            .lock()
            .expect("MockTransport mutex poisoned")
            .get_mut(method)
            .and_then(VecDeque::pop_front);

        match entry {
            Some(boxed) => match boxed.downcast::<Result<T, TransportErrorKind>>() {
                Ok(result) => *result,
                Err(_) => panic!(
                    "MockTransport: queued response for method `{method}` is not of type \
                     Result<{}, TransportErrorKind>",
                    type_name::<T>()
                ),
            },
            None => panic!("MockTransport: no response queued for method `{method}`"),
        }
    }
}

/// Generates the [`TronTransport`] method bodies: each consumes its arguments,
/// pops the next queued response for its name, and returns it as a ready future.
macro_rules! mock_methods {
    ($( fn $name:ident(&self $(, $arg:ident : $ty:ty )* ) -> $ret:ty; )*) => {
        $(
            fn $name(&self $(, $arg : $ty )* ) -> impl Future<Output = Result<$ret, Self::Error>> + Send {
                let _ = ( $( $arg, )* );
                let result = self.pop::<$ret>(stringify!($name));
                async move { result }
            }
        )*
    };
}

impl super::private::Sealed for MockTransport {}

impl TronTransport for MockTransport {
    type Error = TransportErrorKind;

    mock_methods! {
        fn get_now_block(&self) -> BlockInfo;
        fn get_block_by_number(&self, num: i64) -> BlockInfo;
        fn get_account(&self, address: Address) -> AccountInfo;
        fn get_account_resource(&self, address: Address) -> AccountResource;
        fn broadcast_transaction(&self, tx: &SignedTransaction) -> ();
        fn get_transaction_by_id(&self, tx_id: TxId) -> SignedTransaction;
        fn get_transaction_info(&self, tx_id: TxId) -> Option<TransactionInfo>;
        fn trigger_smart_contract(&self, params: TriggerSmartContract) -> RawTransaction;
        fn trigger_constant_contract(&self, params: TriggerSmartContract) -> ConstantCallResult;
        fn estimate_energy(&self, params: TriggerSmartContract) -> i64;
        fn transfer_trx(&self, params: TransferContract) -> RawTransaction;
        fn account_permission_update(&self, params: AccountPermissionUpdateContract) -> RawTransaction;
        fn create_smart_contract(&self, params: CreateSmartContract) -> RawTransaction;
        fn freeze_balance_v1(&self, params: FreezeBalanceV1Contract) -> RawTransaction;
        fn unfreeze_balance_v1(&self, params: UnfreezeBalanceV1Contract) -> RawTransaction;
        fn freeze_balance_v2(&self, params: FreezeBalanceV2Contract) -> RawTransaction;
        fn unfreeze_balance_v2(&self, params: UnfreezeBalanceV2Contract) -> RawTransaction;
        fn delegate_resource(&self, params: DelegateResourceContract) -> RawTransaction;
        fn undelegate_resource(&self, params: UnDelegateResourceContract) -> RawTransaction;
        fn withdraw_expire_unfreeze(&self, params: WithdrawExpireUnfreezeContract) -> RawTransaction;
        fn cancel_all_unfreeze_v2(&self, params: CancelAllUnfreezeV2Contract) -> RawTransaction;
        fn withdraw_balance(&self, params: WithdrawBalanceContract) -> RawTransaction;
        fn get_delegated_resource_v1(&self, from: Address, to: Address) -> Vec<DelegatedResource>;
        fn get_delegated_resource_index_v1(&self, address: Address) -> DelegatedResourceIndex;
        fn get_delegated_resource(&self, from: Address, to: Address) -> Vec<DelegatedResource>;
        fn get_delegated_resource_index(&self, address: Address) -> DelegatedResourceIndex;
        fn get_can_delegate_max(&self, address: Address, resource: ResourceCode) -> Trx;
        fn get_reward(&self, address: Address) -> Trx;
        fn get_chain_parameters(&self) -> HashMap<String, i64>;
        fn get_contract(&self, address: Address) -> SmartContractInfo;
        fn get_contract_info(&self, address: Address) -> SmartContractInfo;
        fn list_witnesses(&self) -> Vec<WitnessInfo>;
        fn proposal_create(&self, params: ProposalCreateContract) -> RawTransaction;
        fn proposal_approve(&self, params: ProposalApproveContract) -> RawTransaction;
        fn proposal_delete(&self, params: ProposalDeleteContract) -> RawTransaction;
        fn list_proposals(&self) -> Vec<ProposalInfo>;
        fn get_paginated_proposal_list(&self, offset: i64, limit: i64) -> Vec<ProposalInfo>;
        fn get_proposal_by_id(&self, proposal_id: i64) -> ProposalInfo;
        fn create_asset_issue(&self, params: AssetIssueContract) -> RawTransaction;
        fn transfer_asset(&self, params: TransferAssetContract) -> RawTransaction;
        fn get_asset_issue_by_id(&self, token_id: &str) -> Option<AssetInfo>;
        fn get_asset_issue_by_account(&self, address: Address) -> Vec<AssetInfo>;
        fn get_paginated_asset_issue_list(&self, offset: i64, limit: i64) -> Vec<AssetInfo>;
        fn get_asset_issue_by_name(&self, name: &str) -> Option<AssetInfo>;
        fn get_asset_issue_list_by_name(&self, name: &str) -> Vec<AssetInfo>;
        fn participate_asset_issue(&self, params: ParticipateAssetIssueContract) -> RawTransaction;
        fn unfreeze_asset(&self, params: UnfreezeAssetContract) -> RawTransaction;
        fn update_asset(&self, params: UpdateAssetContract) -> RawTransaction;
        fn create_account(&self, params: CreateAccountContract) -> RawTransaction;
        fn vote_witness_account(&self, params: VoteWitnessContract) -> RawTransaction;
        fn update_account(&self, params: UpdateAccountContract) -> RawTransaction;
        fn set_account_id(&self, params: SetAccountIdContract) -> RawTransaction;
        fn clear_contract_abi(&self, params: ClearContractAbiContract) -> RawTransaction;
        fn update_setting(&self, params: UpdateSettingContract) -> RawTransaction;
        fn update_energy_limit(&self, params: UpdateEnergyLimitContract) -> RawTransaction;
        fn get_can_withdraw_unfreeze_amount(&self, address: Address, timestamp_ms: i64) -> Trx;
        fn get_available_unfreeze_count(&self, address: Address) -> i64;
        fn get_bandwidth_prices(&self) -> String;
        fn get_energy_prices(&self) -> String;
        fn get_memo_fee(&self) -> u64;
        fn get_next_maintenance_time(&self) -> i64;
        fn get_burn_trx(&self) -> u64;
        fn get_total_transactions(&self) -> u64;
        fn get_node_info(&self) -> NodeInfo;
        fn list_nodes(&self) -> Vec<NodeAddress>;
        fn get_dynamic_properties(&self) -> ChainProperties;
        fn get_block_by_id(&self, block_id: B256) -> BlockInfo;
        fn get_blocks_by_latest_num(&self, count: i64) -> Vec<BlockInfo>;
        fn get_blocks_by_limit(&self, start: i64, end: i64) -> Vec<BlockInfo>;
        fn get_transaction_count_by_block_num(&self, block_num: i64) -> u64;
        fn get_transactions_from(&self, address: Address, offset: i64, limit: i64) -> Vec<RawTransaction>;
        fn get_transactions_to(&self, address: Address, offset: i64, limit: i64) -> Vec<RawTransaction>;
        fn get_transaction_info_by_block_num(&self, block_num: i64) -> Vec<TransactionInfo>;
        fn get_pending_size(&self) -> u64;
        fn get_transaction_from_pending(&self, tx_id: TxId) -> RawTransaction;
        fn get_pending_transactions(&self) -> Vec<RawTransaction>;
        fn get_transaction_sign_weight(&self, tx: &SignedTransaction) -> SignWeight;
        fn get_transaction_approved_list(&self, tx: &SignedTransaction) -> Vec<Address>;
        fn get_account_net(&self, address: Address) -> AccountNet;
        fn create_witness(&self, params: CreateWitnessContract) -> RawTransaction;
        fn update_witness(&self, params: UpdateWitnessContract) -> RawTransaction;
        fn update_brokerage(&self, params: UpdateBrokerageContract) -> RawTransaction;
        fn get_brokerage(&self, address: Address) -> u64;
        fn get_reward_info(&self, address: Address) -> u64;
        fn exchange_create(&self, params: ExchangeCreateContract) -> RawTransaction;
        fn exchange_inject(&self, params: ExchangeInjectContract) -> RawTransaction;
        fn exchange_withdraw(&self, params: ExchangeWithdrawContract) -> RawTransaction;
        fn exchange_transaction(&self, params: ExchangeTransactionContract) -> RawTransaction;
        fn list_exchanges(&self) -> Vec<ExchangeInfo>;
        fn get_paginated_exchange_list(&self, offset: i64, limit: i64) -> Vec<ExchangeInfo>;
        fn get_exchange_by_id(&self, exchange_id: i64) -> Option<ExchangeInfo>;
        fn market_sell_asset(&self, params: MarketSellAssetContract) -> RawTransaction;
        fn market_cancel_order(&self, params: MarketCancelOrderContract) -> RawTransaction;
        fn get_market_order_by_id(&self, order_id: &[u8]) -> Option<MarketOrderInfo>;
        fn get_market_order_by_account(&self, address: Address) -> Vec<MarketOrderInfo>;
        fn get_market_price_by_pair(&self, sell_token_id: &str, buy_token_id: &str) -> Vec<MarketPrice>;
        fn get_market_order_list_by_pair(&self, sell_token_id: &str, buy_token_id: &str) -> Vec<MarketOrderInfo>;
        fn get_market_pair_list(&self) -> Vec<MarketOrderPair>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RootProvider;

    #[tokio::test]
    async fn returns_queued_ok_responses_in_fifo_order() {
        let mock = MockTransport::new();
        mock.push_ok::<u64>("get_memo_fee", 1)
            .push_ok::<u64>("get_memo_fee", 2);

        assert_eq!(mock.get_memo_fee().await.unwrap(), 1);
        assert_eq!(mock.get_memo_fee().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn returns_queued_err_responses() {
        let mock = MockTransport::new();
        mock.push_err::<u64>(
            "get_burn_trx",
            TransportErrorKind::Malformed("boom".to_owned()),
        );

        let err = mock.get_burn_trx().await.unwrap_err();
        assert!(matches!(err, TransportErrorKind::Malformed(_)));
    }

    #[tokio::test]
    async fn delegates_through_root_provider() {
        let mock = MockTransport::new();
        mock.push_ok::<u64>("get_total_transactions", 42);

        // Exercises the real provider -> transport delegation path.
        let provider = RootProvider::new(mock);
        assert_eq!(
            provider.transport().get_total_transactions().await.unwrap(),
            42
        );
    }

    #[tokio::test]
    #[should_panic(expected = "no response queued for method `get_memo_fee`")]
    async fn panics_when_no_response_queued() {
        let mock = MockTransport::new();
        let _ = mock.get_memo_fee().await;
    }
}
