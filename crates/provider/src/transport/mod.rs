//! Transport abstraction over a TRON node's API.
//!
//! [`TronTransport`] is a domain-specific async trait; [`grpc`] provides the
//! default tonic-backed gRPC implementation targeting `grpc.trongrid.io:443`.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use auto_impl::auto_impl;
use tronz_primitives::{Address, B256, ResourceCode, Trx, TxId};

use crate::{
    error::TransportResult,
    types::{
        AccountInfo, AccountNet, AccountPermissionUpdateContract, AccountResource, AssetInfo,
        AssetIssueContract, BlockInfo, ChainProperties, ClearContractAbiContract,
        ConstantCallResult, CreateAccountContract, CreateSmartContract, CreateWitnessContract,
        DelegatedResource, DelegatedResourceIndex, ExchangeCreateContract, ExchangeInfo,
        ExchangeInjectContract, ExchangeTransactionContract, ExchangeWithdrawContract,
        FreezeBalanceV1Contract, FreezeBalanceV2Contract, MarketCancelOrderContract,
        MarketOrderInfo, MarketOrderPair, MarketPrice, MarketSellAssetContract, NodeAddress,
        NodeInfo, ParticipateAssetIssueContract, ProposalApproveContract, ProposalCreateContract,
        ProposalDeleteContract, ProposalInfo, RawTransaction, SetAccountIdContract, SignWeight,
        SignedTransaction, SmartContractInfo, TransactionInfo, TransferAssetContract,
        TransferContract, TriggerSmartContract, UnDelegateResourceContract, UnfreezeAssetContract,
        UnfreezeBalanceV1Contract, UnfreezeBalanceV2Contract, UpdateAccountContract,
        UpdateAssetContract, UpdateBrokerageContract, UpdateEnergyLimitContract,
        UpdateSettingContract, UpdateWitnessContract, VoteWitnessContract, WithdrawBalanceContract,
        WithdrawExpireUnfreezeContract, WitnessInfo,
    },
};

pub mod grpc;

mod solidity;
pub use solidity::{DynSolidityTransport, SolidityTransport};

#[cfg(any(test, feature = "mock"))]
pub mod mock;

pub(crate) mod private {
    /// Sealed marker: only this crate may implement [`TronTransport`](super::TronTransport).
    ///
    /// New transport methods map directly to node RPCs and cannot have default
    /// implementations, so sealing keeps the SDK free to add them in minor
    /// releases without breaking downstream code. Tests use the in-crate
    /// `MockTransport` (feature `mock`).
    pub trait Sealed {}

    // Lets `auto_impl`'s forwarding impls satisfy the seal, which is what makes
    // `Arc<dyn TronTransport>` a transport in its own right.
    impl<T: ?Sized + Sealed> Sealed for &T {}
    impl<T: ?Sized + Sealed> Sealed for std::sync::Arc<T> {}
}

/// A [`TronTransport`] with its concrete type erased.
///
/// This is how [`RootProvider`](crate::RootProvider) holds its transport, which is
/// why no provider type mentions one. Constructing it by hand is only worth it to
/// share a single connection between providers, since `RootProvider::new` erases
/// whatever it is given:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use tronz_provider::{DynTransport, RootProvider};
/// # use tronz_provider::transport::grpc::{GrpcTransport, TRONGRID_MAINNET};
/// # async fn run() -> tronz_provider::Result<()> {
/// let transport: DynTransport = Arc::new(GrpcTransport::connect(TRONGRID_MAINNET).await?);
/// let read = RootProvider::new_erased(Arc::clone(&transport));
/// let write = RootProvider::new_erased(transport);
/// # let _ = (read, write);
/// # Ok(()) }
/// ```
pub type DynTransport = Arc<dyn TronTransport>;

/// A low-level transport that maps each TRON node API endpoint to an async
/// method returning domain types.
///
/// Implementations must be `Send + Sync + 'static` for use across spawned tasks.
/// `Clone` is deliberately not required: providers store the transport erased, so
/// [`DynTransport`] is a transport in its own right.
///
/// Methods are boxed by `async_trait`, so every call — erased or not — costs one
/// allocation. That is what makes the trait object-safe, and it is invisible next
/// to the network round-trip it wraps.
///
/// This trait is **sealed** — only `tronz` may implement it. For tests, use the
/// `MockTransport` provided under the `mock` feature.
#[async_trait]
#[auto_impl(&, Arc)]
pub trait TronTransport: Send + Sync + 'static + private::Sealed {
    // --- Block ---

    /// Fetch the latest block.
    async fn get_now_block(&self) -> TransportResult<BlockInfo>;

    /// Fetch a block by height.
    async fn get_block_by_number(&self, num: i64) -> TransportResult<Option<BlockInfo>>;

    // --- Account ---

    /// Fetch on-chain account state.
    async fn get_account(&self, address: Address) -> TransportResult<AccountInfo>;

    /// Fetch account bandwidth/energy resource usage.
    async fn get_account_resource(&self, address: Address) -> TransportResult<AccountResource>;

    // --- Transaction ---

    /// Broadcast a signed transaction.
    async fn broadcast_transaction(&self, tx: &SignedTransaction) -> TransportResult<()>;

    /// Fetch a transaction by id.
    async fn get_transaction_by_id(
        &self,
        tx_id: TxId,
    ) -> TransportResult<Option<SignedTransaction>>;

    /// Fetch a transaction's post-confirmation info/receipt.
    ///
    /// Returns `None` if the node has not yet indexed the transaction.
    async fn get_transaction_info(&self, tx_id: TxId) -> TransportResult<Option<TransactionInfo>>;

    // --- Smart contracts ---

    /// Build an unsigned `RawTransaction` for a contract trigger (server fills TAPOS).
    async fn trigger_smart_contract(
        &self,
        params: TriggerSmartContract,
    ) -> TransportResult<RawTransaction>;

    /// Execute a constant (read-only) contract call.
    async fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> TransportResult<ConstantCallResult>;

    /// Estimate the energy a contract call would consume.
    async fn estimate_energy(&self, params: TriggerSmartContract) -> TransportResult<i64>;

    // --- Native contracts ---

    /// Build a TRX transfer transaction.
    async fn transfer_trx(&self, params: TransferContract) -> TransportResult<RawTransaction>;

    /// Build an account-permission-update transaction.
    async fn account_permission_update(
        &self,
        params: AccountPermissionUpdateContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a smart-contract-deploy transaction.
    async fn create_smart_contract(
        &self,
        params: CreateSmartContract,
    ) -> TransportResult<RawTransaction>;

    // --- Staking ---

    /// Build a freeze (stake) transaction (Stake 1.0, legacy).
    async fn freeze_balance_v1(
        &self,
        params: FreezeBalanceV1Contract,
    ) -> TransportResult<RawTransaction>;

    /// Build an unfreeze (unstake) transaction (Stake 1.0, legacy).
    async fn unfreeze_balance_v1(
        &self,
        params: UnfreezeBalanceV1Contract,
    ) -> TransportResult<RawTransaction>;

    /// Build a freeze (stake) transaction.
    async fn freeze_balance_v2(
        &self,
        params: FreezeBalanceV2Contract,
    ) -> TransportResult<RawTransaction>;

    /// Build an unfreeze (unstake) transaction.
    async fn unfreeze_balance_v2(
        &self,
        params: UnfreezeBalanceV2Contract,
    ) -> TransportResult<RawTransaction>;

    /// Build a delegate-resource transaction.
    async fn delegate_resource(
        &self,
        params: crate::types::DelegateResourceContract,
    ) -> TransportResult<RawTransaction>;

    /// Build an undelegate-resource transaction.
    async fn undelegate_resource(
        &self,
        params: UnDelegateResourceContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a withdraw-expire-unfreeze transaction.
    async fn withdraw_expire_unfreeze(
        &self,
        params: WithdrawExpireUnfreezeContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a cancel-all-unfreeze transaction.
    async fn cancel_all_unfreeze_v2(
        &self,
        params: crate::types::CancelAllUnfreezeV2Contract,
    ) -> TransportResult<RawTransaction>;

    /// Build a withdraw-balance (claim rewards) transaction.
    async fn withdraw_balance(
        &self,
        params: WithdrawBalanceContract,
    ) -> TransportResult<RawTransaction>;

    // --- Resource queries ---

    /// Query delegations between two accounts (Stake 1.0, legacy).
    async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> TransportResult<Vec<DelegatedResource>>;

    /// Query the full delegation index for an account (Stake 1.0, legacy).
    async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> TransportResult<DelegatedResourceIndex>;

    /// Query delegations between two accounts (Stake 2.0).
    async fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> TransportResult<Vec<DelegatedResource>>;

    /// Query the full delegation index for an account (Stake 2.0).
    async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> TransportResult<DelegatedResourceIndex>;

    /// Query the max amount still delegatable for a resource.
    async fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> TransportResult<Trx>;

    /// Query the pending (unclaimed) reward for an account.
    async fn get_reward(&self, address: Address) -> TransportResult<Trx>;

    // --- Network ---

    /// Fetch the chain parameters.
    async fn get_chain_parameters(&self) -> TransportResult<HashMap<String, i64>>;

    /// Fetch metadata for a deployed contract.
    async fn get_contract(&self, address: Address) -> TransportResult<SmartContractInfo>;

    /// Fetch contract metadata including the deployed runtime bytecode.
    ///
    /// Like [`get_contract`](Self::get_contract) but also populates
    /// [`SmartContractInfo::runtime_bytecode`].
    async fn get_contract_info(&self, address: Address) -> TransportResult<SmartContractInfo>;

    /// List all super representatives and candidates.
    async fn list_witnesses(&self) -> TransportResult<Vec<WitnessInfo>>;

    /// Fetch a paginated list of witnesses sorted by real-time vote count.
    async fn get_paginated_now_witness_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<WitnessInfo>>;

    // --- Governance ---

    /// Submit a chain-parameter governance proposal.
    async fn proposal_create(
        &self,
        params: ProposalCreateContract,
    ) -> TransportResult<RawTransaction>;

    /// Approve or revoke approval for a governance proposal.
    async fn proposal_approve(
        &self,
        params: ProposalApproveContract,
    ) -> TransportResult<RawTransaction>;

    /// Cancel a governance proposal.
    async fn proposal_delete(
        &self,
        params: ProposalDeleteContract,
    ) -> TransportResult<RawTransaction>;

    /// List all on-chain proposals.
    async fn list_proposals(&self) -> TransportResult<Vec<ProposalInfo>>;

    /// Fetch a paginated list of proposals.
    async fn get_paginated_proposal_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<ProposalInfo>>;

    /// Fetch a single proposal by its ID.
    async fn get_proposal_by_id(&self, proposal_id: i64) -> TransportResult<ProposalInfo>;

    // --- TRC10 ---

    /// Build a TRC10 token issuance transaction.
    async fn create_asset_issue(
        &self,
        params: AssetIssueContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a TRC10 token transfer transaction.
    async fn transfer_asset(
        &self,
        params: TransferAssetContract,
    ) -> TransportResult<RawTransaction>;

    /// Fetch metadata for a TRC10 token by its numeric ID.
    ///
    /// Returns `None` if no token with that ID exists.
    async fn get_asset_issue_by_id(&self, token_id: &str) -> TransportResult<Option<AssetInfo>>;

    /// Fetch all TRC10 tokens issued by `address`.
    async fn get_asset_issue_by_account(&self, address: Address)
    -> TransportResult<Vec<AssetInfo>>;

    /// Fetch a paginated list of all TRC10 tokens on-chain.
    async fn get_paginated_asset_issue_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<AssetInfo>>;

    /// Fetch a TRC10 token by name.
    ///
    /// Returns `None` if no token with that name exists.
    ///
    /// Token names are not unique after the `ALLOW_SAME_TOKEN_NAME` proposal;
    /// use [`get_asset_issue_list_by_name`](Self::get_asset_issue_list_by_name)
    /// if multiple tokens share the same name.
    async fn get_asset_issue_by_name(&self, name: &str) -> TransportResult<Option<AssetInfo>>;

    /// Fetch all TRC10 tokens with a given name.
    async fn get_asset_issue_list_by_name(&self, name: &str) -> TransportResult<Vec<AssetInfo>>;

    /// Build a participate-in-ICO transaction (buy TRC10 tokens with TRX).
    async fn participate_asset_issue(
        &self,
        params: ParticipateAssetIssueContract,
    ) -> TransportResult<RawTransaction>;

    /// Build an unfreeze-asset transaction (release frozen TRC10 supply).
    async fn unfreeze_asset(
        &self,
        params: UnfreezeAssetContract,
    ) -> TransportResult<RawTransaction>;

    /// Build an update-asset transaction (change TRC10 metadata).
    async fn update_asset(&self, params: UpdateAssetContract) -> TransportResult<RawTransaction>;

    // --- Account management ---

    /// Activate a new account on-chain.
    async fn create_account(
        &self,
        params: CreateAccountContract,
    ) -> TransportResult<RawTransaction>;

    /// Vote for super representatives.
    async fn vote_witness_account(
        &self,
        params: VoteWitnessContract,
    ) -> TransportResult<RawTransaction>;

    /// Update an account's on-chain name.
    async fn update_account(
        &self,
        params: UpdateAccountContract,
    ) -> TransportResult<RawTransaction>;

    /// Set a short alphanumeric account ID (on-chain alias).
    async fn set_account_id(&self, params: SetAccountIdContract)
    -> TransportResult<RawTransaction>;

    /// Clear the ABI of a deployed smart contract.
    async fn clear_contract_abi(
        &self,
        params: ClearContractAbiContract,
    ) -> TransportResult<RawTransaction>;

    /// Update the caller-energy-percentage setting on a smart contract.
    async fn update_setting(
        &self,
        params: UpdateSettingContract,
    ) -> TransportResult<RawTransaction>;

    /// Update the per-call origin energy limit on a smart contract.
    async fn update_energy_limit(
        &self,
        params: UpdateEnergyLimitContract,
    ) -> TransportResult<RawTransaction>;

    // --- Staking queries ---

    /// Query how much TRX can be withdrawn from expired unfreeze windows.
    ///
    /// `timestamp_ms` is the reference time (unix milliseconds); pass the
    /// current time to check what is withdrawable right now.
    async fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> TransportResult<Trx>;

    /// Query how many more unfreeze operations the account can initiate
    /// (TRON caps concurrent unfreeze windows to 32).
    async fn get_available_unfreeze_count(&self, address: Address) -> TransportResult<i64>;

    // --- Pricing / fees ---

    /// Fetch the historical bandwidth price schedule (colon-separated pairs).
    async fn get_bandwidth_prices(&self) -> TransportResult<String>;

    /// Fetch the historical energy price schedule (colon-separated pairs).
    async fn get_energy_prices(&self) -> TransportResult<String>;

    /// Fetch the memo-attach fee schedule.
    async fn get_memo_fee(&self) -> TransportResult<u64>;

    // --- Network / chain ---

    /// Fetch the next maintenance-cycle timestamp (unix ms).
    async fn get_next_maintenance_time(&self) -> TransportResult<i64>;

    /// Fetch the total amount of TRX that has been burned.
    async fn get_burn_trx(&self) -> TransportResult<u64>;

    /// Fetch the total number of transactions ever processed.
    async fn get_total_transactions(&self) -> TransportResult<u64>;

    /// Fetch basic info about the connected node.
    async fn get_node_info(&self) -> TransportResult<NodeInfo>;

    /// List all known gossip-network peer addresses.
    async fn list_nodes(&self) -> TransportResult<Vec<NodeAddress>>;

    /// Fetch dynamic chain properties (head block id, number, timestamp).
    async fn get_dynamic_properties(&self) -> TransportResult<ChainProperties>;

    // --- Block queries ---

    /// Fetch a block by its hash (block id).
    async fn get_block_by_id(&self, block_id: B256) -> TransportResult<Option<BlockInfo>>;

    /// Fetch the `count` most recent blocks.
    async fn get_blocks_by_latest_num(&self, count: i64) -> TransportResult<Vec<BlockInfo>>;

    /// Fetch blocks in the range `[start, end)`.
    async fn get_blocks_by_limit(&self, start: i64, end: i64) -> TransportResult<Vec<BlockInfo>>;

    /// Count transactions in a given block by block number.
    async fn get_transaction_count_by_block_num(&self, block_num: i64) -> TransportResult<u64>;

    // --- Transaction history ---

    /// Fetch paginated transactions sent *from* an address.
    async fn get_transactions_from(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<RawTransaction>>;

    /// Fetch paginated transactions sent *to* an address.
    async fn get_transactions_to(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<RawTransaction>>;

    /// Fetch all transaction infos included in a given block.
    async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> TransportResult<Vec<TransactionInfo>>;

    // --- Pending pool ---

    /// Fetch the number of pending (unconfirmed) transactions.
    async fn get_pending_size(&self) -> TransportResult<u64>;

    /// Fetch a single pending transaction by id.
    async fn get_transaction_from_pending(&self, tx_id: TxId) -> TransportResult<RawTransaction>;

    /// Fetch all pending transactions.
    async fn get_pending_transactions(&self) -> TransportResult<Vec<RawTransaction>>;

    // --- Multi-sig ---

    /// Query the sign-weight status for a transaction (how many sigs are
    /// present and whether the threshold is met).
    ///
    /// Takes a [`SignedTransaction`] so the already-collected signatures are
    /// included; the node uses them to compute `current_weight` and the
    /// approved-address list.
    async fn get_transaction_sign_weight(
        &self,
        tx: &SignedTransaction,
    ) -> TransportResult<SignWeight>;

    /// Fetch the list of addresses that have already signed a transaction.
    async fn get_transaction_approved_list(
        &self,
        tx: &SignedTransaction,
    ) -> TransportResult<Vec<Address>>;

    // --- Account net ---

    /// Fetch bandwidth and energy net-usage for an account.
    async fn get_account_net(&self, address: Address) -> TransportResult<AccountNet>;

    // --- Witness ---

    /// Apply to become a super representative candidate.
    async fn create_witness(
        &self,
        params: CreateWitnessContract,
    ) -> TransportResult<RawTransaction>;

    /// Update a super representative's public URL.
    async fn update_witness(
        &self,
        params: UpdateWitnessContract,
    ) -> TransportResult<RawTransaction>;

    /// Update a super representative's brokerage ratio.
    async fn update_brokerage(
        &self,
        params: UpdateBrokerageContract,
    ) -> TransportResult<RawTransaction>;

    /// Fetch the brokerage ratio (0–100) for a super representative.
    async fn get_brokerage(&self, address: Address) -> TransportResult<u64>;

    /// Fetch the unclaimed reward amount for an address (alias for
    /// [`crate::provider::TronProvider::get_reward`]).
    ///
    /// Unlike [`crate::provider::TronProvider::get_reward`] which returns [`Trx`], this returns the
    /// raw sun value.
    async fn get_reward_info(&self, address: Address) -> TransportResult<u64>;

    // --- DEX (built-in Bancor exchange) ---

    /// Build a transaction that creates a new TRC10 exchange pair.
    async fn exchange_create(
        &self,
        params: ExchangeCreateContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a transaction that injects liquidity into an exchange pair.
    async fn exchange_inject(
        &self,
        params: ExchangeInjectContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a transaction that withdraws liquidity from an exchange pair.
    async fn exchange_withdraw(
        &self,
        params: ExchangeWithdrawContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a transaction that executes a swap on an exchange pair.
    async fn exchange_transaction(
        &self,
        params: ExchangeTransactionContract,
    ) -> TransportResult<RawTransaction>;

    /// List all exchange pairs on-chain.
    async fn list_exchanges(&self) -> TransportResult<Vec<ExchangeInfo>>;

    /// Fetch a paginated list of exchange pairs.
    async fn get_paginated_exchange_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<ExchangeInfo>>;

    /// Fetch a single exchange pair by its ID.
    ///
    /// Returns `None` if no exchange with that ID exists.
    async fn get_exchange_by_id(&self, exchange_id: i64) -> TransportResult<Option<ExchangeInfo>>;

    // --- Market (order-book DEX) ---

    /// Build a transaction that places a limit sell order.
    async fn market_sell_asset(
        &self,
        params: MarketSellAssetContract,
    ) -> TransportResult<RawTransaction>;

    /// Build a transaction that cancels an open market order.
    async fn market_cancel_order(
        &self,
        params: MarketCancelOrderContract,
    ) -> TransportResult<RawTransaction>;

    /// Fetch a market order by its ID.
    ///
    /// Returns `None` if no order with that ID exists.
    async fn get_market_order_by_id(
        &self,
        order_id: B256,
    ) -> TransportResult<Option<MarketOrderInfo>>;

    /// Fetch all market orders placed by `address`.
    async fn get_market_order_by_account(
        &self,
        address: Address,
    ) -> TransportResult<Vec<MarketOrderInfo>>;

    /// Fetch the price levels for a trading pair.
    async fn get_market_price_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> TransportResult<Vec<MarketPrice>>;

    /// Fetch all open orders for a trading pair.
    async fn get_market_order_list_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> TransportResult<Vec<MarketOrderInfo>>;

    /// Fetch all active trading pairs on the order-book DEX.
    async fn get_market_pair_list(&self) -> TransportResult<Vec<MarketOrderPair>>;
}
