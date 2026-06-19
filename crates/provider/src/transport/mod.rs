//! Transport abstraction over a TRON node's API.
//!
//! [`TronTransport`] is a domain-specific async trait; [`grpc`] provides the
//! default tonic-backed gRPC implementation targeting `grpc.trongrid.io:443`.

use core::future::Future;
use std::collections::HashMap;

use tronz_primitives::{Address, B256, ResourceCode, Trx, TxId};

use crate::types::{
    AccountInfo, AccountNet, AccountPermissionUpdateContract, AccountResource, AssetInfo,
    AssetIssueContract, BlockInfo, ChainProperties, ClearContractAbiContract, ConstantCallResult,
    CreateAccountContract, CreateSmartContract, CreateWitnessContract, DelegatedResource,
    DelegatedResourceIndex, FreezeBalanceV1Contract, FreezeBalanceV2Contract, NodeAddress,
    NodeInfo, ParticipateAssetIssueContract, ProposalApproveContract, ProposalCreateContract,
    ProposalDeleteContract, ProposalInfo, RawTransaction, SetAccountIdContract, SignWeight,
    SignedTransaction, SmartContractInfo, TransactionInfo, TransferAssetContract, TransferContract,
    TriggerSmartContract, UnDelegateResourceContract, UnfreezeAssetContract,
    UnfreezeBalanceV1Contract, UnfreezeBalanceV2Contract, UpdateAccountContract,
    UpdateAssetContract, UpdateBrokerageContract, UpdateEnergyLimitContract, UpdateSettingContract,
    UpdateWitnessContract, VoteWitnessContract, WithdrawBalanceContract,
    WithdrawExpireUnfreezeContract, WitnessInfo,
};

pub mod grpc;

/// A low-level transport that maps each TRON node API endpoint to an async
/// method returning domain types.
///
/// Implementations are cheap to clone (typically an `Arc`-backed HTTP client)
/// and must be `Send + Sync + 'static` for use across spawned tasks.
pub trait TronTransport: Clone + Send + Sync + 'static {
    /// The transport's error type.  Must be convertible to
    /// [`crate::error::TransportErrorKind`] so that the provider layer can wrap it
    /// uniformly.
    type Error: std::error::Error + Into<crate::error::TransportErrorKind> + Send + Sync + 'static;

    // --- Block ---

    /// Fetch the latest block.
    fn get_now_block(&self) -> impl Future<Output = Result<BlockInfo, Self::Error>> + Send;

    /// Fetch a block by height.
    fn get_block_by_number(
        &self,
        num: i64,
    ) -> impl Future<Output = Result<BlockInfo, Self::Error>> + Send;

    // --- Account ---

    /// Fetch on-chain account state.
    fn get_account(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<AccountInfo, Self::Error>> + Send;

    /// Fetch account bandwidth/energy resource usage.
    fn get_account_resource(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<AccountResource, Self::Error>> + Send;

    // --- Transaction ---

    /// Broadcast a signed transaction.
    fn broadcast_transaction(
        &self,
        tx: &SignedTransaction,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Fetch a transaction by id.
    fn get_transaction_by_id(
        &self,
        tx_id: TxId,
    ) -> impl Future<Output = Result<SignedTransaction, Self::Error>> + Send;

    /// Fetch a transaction's post-confirmation info/receipt.
    ///
    /// Returns `None` if the node has not yet indexed the transaction.
    fn get_transaction_info(
        &self,
        tx_id: TxId,
    ) -> impl Future<Output = Result<Option<TransactionInfo>, Self::Error>> + Send;

    // --- Smart contracts ---

    /// Build an unsigned `RawTransaction` for a contract trigger (server fills TAPOS).
    fn trigger_smart_contract(
        &self,
        params: TriggerSmartContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Execute a constant (read-only) contract call.
    fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> impl Future<Output = Result<ConstantCallResult, Self::Error>> + Send;

    /// Estimate the energy a contract call would consume.
    fn estimate_energy(
        &self,
        params: TriggerSmartContract,
    ) -> impl Future<Output = Result<i64, Self::Error>> + Send;

    // --- Native contracts ---

    /// Build a TRX transfer transaction.
    fn transfer_trx(
        &self,
        params: TransferContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build an account-permission-update transaction.
    fn account_permission_update(
        &self,
        params: AccountPermissionUpdateContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build a smart-contract-deploy transaction.
    fn create_smart_contract(
        &self,
        params: CreateSmartContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    // --- Staking ---

    /// Build a freeze (stake) transaction (Stake 1.0, legacy).
    fn freeze_balance_v1(
        &self,
        params: FreezeBalanceV1Contract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build an unfreeze (unstake) transaction (Stake 1.0, legacy).
    fn unfreeze_balance_v1(
        &self,
        params: UnfreezeBalanceV1Contract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build a freeze (stake) transaction.
    fn freeze_balance_v2(
        &self,
        params: FreezeBalanceV2Contract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build an unfreeze (unstake) transaction.
    fn unfreeze_balance_v2(
        &self,
        params: UnfreezeBalanceV2Contract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build a delegate-resource transaction.
    fn delegate_resource(
        &self,
        params: crate::types::DelegateResourceContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build an undelegate-resource transaction.
    fn undelegate_resource(
        &self,
        params: UnDelegateResourceContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build a withdraw-expire-unfreeze transaction.
    fn withdraw_expire_unfreeze(
        &self,
        params: WithdrawExpireUnfreezeContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build a cancel-all-unfreeze transaction.
    fn cancel_all_unfreeze_v2(
        &self,
        params: crate::types::CancelAllUnfreezeV2Contract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build a withdraw-balance (claim rewards) transaction.
    fn withdraw_balance(
        &self,
        params: WithdrawBalanceContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    // --- Resource queries ---

    /// Query delegations between two accounts (Stake 1.0, legacy).
    fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> impl Future<Output = Result<Vec<DelegatedResource>, Self::Error>> + Send;

    /// Query the full delegation index for an account (Stake 1.0, legacy).
    fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<DelegatedResourceIndex, Self::Error>> + Send;

    /// Query delegations between two accounts (Stake 2.0).
    fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> impl Future<Output = Result<Vec<DelegatedResource>, Self::Error>> + Send;

    /// Query the full delegation index for an account (Stake 2.0).
    fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<DelegatedResourceIndex, Self::Error>> + Send;

    /// Query the max amount still delegatable for a resource.
    fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> impl Future<Output = Result<Trx, Self::Error>> + Send;

    /// Query the pending (unclaimed) reward for an account.
    fn get_reward(&self, address: Address)
    -> impl Future<Output = Result<Trx, Self::Error>> + Send;

    // --- Network ---

    /// Fetch the chain parameters.
    fn get_chain_parameters(
        &self,
    ) -> impl Future<Output = Result<HashMap<String, i64>, Self::Error>> + Send;

    /// Fetch metadata for a deployed contract.
    fn get_contract(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<SmartContractInfo, Self::Error>> + Send;

    /// Fetch contract metadata including the deployed runtime bytecode.
    ///
    /// Like [`get_contract`](Self::get_contract) but also populates
    /// [`SmartContractInfo::runtime_bytecode`].
    fn get_contract_info(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<SmartContractInfo, Self::Error>> + Send;

    /// List all super representatives and candidates.
    fn list_witnesses(&self) -> impl Future<Output = Result<Vec<WitnessInfo>, Self::Error>> + Send;

    // --- Governance ---

    /// Submit a chain-parameter governance proposal.
    fn proposal_create(
        &self,
        params: ProposalCreateContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Approve or revoke approval for a governance proposal.
    fn proposal_approve(
        &self,
        params: ProposalApproveContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Cancel a governance proposal.
    fn proposal_delete(
        &self,
        params: ProposalDeleteContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// List all on-chain proposals.
    fn list_proposals(&self)
    -> impl Future<Output = Result<Vec<ProposalInfo>, Self::Error>> + Send;

    /// Fetch a paginated list of proposals.
    fn get_paginated_proposal_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<ProposalInfo>, Self::Error>> + Send;

    /// Fetch a single proposal by its ID.
    fn get_proposal_by_id(
        &self,
        proposal_id: i64,
    ) -> impl Future<Output = Result<ProposalInfo, Self::Error>> + Send;

    // --- TRC10 ---

    /// Build a TRC10 token issuance transaction.
    fn create_asset_issue(
        &self,
        params: AssetIssueContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build a TRC10 token transfer transaction.
    fn transfer_asset(
        &self,
        params: TransferAssetContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Fetch metadata for a TRC10 token by its numeric ID.
    ///
    /// Returns `None` if no token with that ID exists.
    fn get_asset_issue_by_id(
        &self,
        token_id: &str,
    ) -> impl Future<Output = Result<Option<AssetInfo>, Self::Error>> + Send;

    /// Fetch all TRC10 tokens issued by `address`.
    fn get_asset_issue_by_account(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Vec<AssetInfo>, Self::Error>> + Send;

    /// Fetch a paginated list of all TRC10 tokens on-chain.
    fn get_paginated_asset_issue_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<AssetInfo>, Self::Error>> + Send;

    /// Fetch a TRC10 token by name.
    ///
    /// Returns `None` if no token with that name exists.
    ///
    /// Token names are not unique after the `ALLOW_SAME_TOKEN_NAME` proposal;
    /// use [`get_asset_issue_list_by_name`](Self::get_asset_issue_list_by_name)
    /// if multiple tokens share the same name.
    fn get_asset_issue_by_name(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<AssetInfo>, Self::Error>> + Send;

    /// Fetch all TRC10 tokens with a given name.
    fn get_asset_issue_list_by_name(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Vec<AssetInfo>, Self::Error>> + Send;

    /// Build a participate-in-ICO transaction (buy TRC10 tokens with TRX).
    fn participate_asset_issue(
        &self,
        params: ParticipateAssetIssueContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build an unfreeze-asset transaction (release frozen TRC10 supply).
    fn unfreeze_asset(
        &self,
        params: UnfreezeAssetContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Build an update-asset transaction (change TRC10 metadata).
    fn update_asset(
        &self,
        params: UpdateAssetContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    // --- Account management ---

    /// Activate a new account on-chain.
    fn create_account(
        &self,
        params: CreateAccountContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Vote for super representatives.
    fn vote_witness_account(
        &self,
        params: VoteWitnessContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Update an account's on-chain name.
    fn update_account(
        &self,
        params: UpdateAccountContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Set a short alphanumeric account ID (on-chain alias).
    fn set_account_id(
        &self,
        params: SetAccountIdContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Clear the ABI of a deployed smart contract.
    fn clear_contract_abi(
        &self,
        params: ClearContractAbiContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Update the caller-energy-percentage setting on a smart contract.
    fn update_setting(
        &self,
        params: UpdateSettingContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Update the per-call origin energy limit on a smart contract.
    fn update_energy_limit(
        &self,
        params: UpdateEnergyLimitContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    // --- Staking queries ---

    /// Query how much TRX can be withdrawn from expired unfreeze windows.
    ///
    /// `timestamp_ms` is the reference time (unix milliseconds); pass the
    /// current time to check what is withdrawable right now.
    fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> impl Future<Output = Result<Trx, Self::Error>> + Send;

    /// Query how many more unfreeze operations the account can initiate
    /// (TRON caps concurrent unfreeze windows to 32).
    fn get_available_unfreeze_count(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<i64, Self::Error>> + Send;

    // --- Pricing / fees ---

    /// Fetch the historical bandwidth price schedule (colon-separated pairs).
    fn get_bandwidth_prices(&self) -> impl Future<Output = Result<String, Self::Error>> + Send;

    /// Fetch the historical energy price schedule (colon-separated pairs).
    fn get_energy_prices(&self) -> impl Future<Output = Result<String, Self::Error>> + Send;

    /// Fetch the memo-attach fee schedule.
    fn get_memo_fee(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    // --- Network / chain ---

    /// Fetch the next maintenance-cycle timestamp (unix ms).
    fn get_next_maintenance_time(&self) -> impl Future<Output = Result<i64, Self::Error>> + Send;

    /// Fetch the total amount of TRX that has been burned.
    fn get_burn_trx(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    /// Fetch the total number of transactions ever processed.
    fn get_total_transactions(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    /// Fetch basic info about the connected node.
    fn get_node_info(&self) -> impl Future<Output = Result<NodeInfo, Self::Error>> + Send;

    /// List all known gossip-network peer addresses.
    fn list_nodes(&self) -> impl Future<Output = Result<Vec<NodeAddress>, Self::Error>> + Send;

    /// Fetch dynamic chain properties (head block id, number, timestamp).
    fn get_dynamic_properties(
        &self,
    ) -> impl Future<Output = Result<ChainProperties, Self::Error>> + Send;

    // --- Block queries ---

    /// Fetch a block by its hash (block id).
    fn get_block_by_id(
        &self,
        block_id: B256,
    ) -> impl Future<Output = Result<BlockInfo, Self::Error>> + Send;

    /// Fetch the `count` most recent blocks.
    fn get_blocks_by_latest_num(
        &self,
        count: i64,
    ) -> impl Future<Output = Result<Vec<BlockInfo>, Self::Error>> + Send;

    /// Fetch blocks in the range `[start, end)`.
    fn get_blocks_by_limit(
        &self,
        start: i64,
        end: i64,
    ) -> impl Future<Output = Result<Vec<BlockInfo>, Self::Error>> + Send;

    /// Count transactions in a given block by block number.
    fn get_transaction_count_by_block_num(
        &self,
        block_num: i64,
    ) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    // --- Transaction history ---

    /// Fetch paginated transactions sent *from* an address.
    fn get_transactions_from(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<RawTransaction>, Self::Error>> + Send;

    /// Fetch paginated transactions sent *to* an address.
    fn get_transactions_to(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<RawTransaction>, Self::Error>> + Send;

    /// Fetch all transaction infos included in a given block.
    fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> impl Future<Output = Result<Vec<TransactionInfo>, Self::Error>> + Send;

    // --- Pending pool ---

    /// Fetch the number of pending (unconfirmed) transactions.
    fn get_pending_size(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    /// Fetch a single pending transaction by id.
    fn get_transaction_from_pending(
        &self,
        tx_id: TxId,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Fetch all pending transactions.
    fn get_pending_transactions(
        &self,
    ) -> impl Future<Output = Result<Vec<RawTransaction>, Self::Error>> + Send;

    // --- Multi-sig ---

    /// Query the sign-weight status for a transaction (how many sigs are
    /// present and whether the threshold is met).
    fn get_transaction_sign_weight(
        &self,
        tx: &RawTransaction,
    ) -> impl Future<Output = Result<SignWeight, Self::Error>> + Send;

    /// Fetch the list of addresses that have already signed a transaction.
    fn get_transaction_approved_list(
        &self,
        tx: &RawTransaction,
    ) -> impl Future<Output = Result<Vec<Address>, Self::Error>> + Send;

    // --- Account net ---

    /// Fetch bandwidth and energy net-usage for an account.
    fn get_account_net(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<AccountNet, Self::Error>> + Send;

    // --- Witness ---

    /// Apply to become a super representative candidate.
    fn create_witness(
        &self,
        params: CreateWitnessContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Update a super representative's public URL.
    fn update_witness(
        &self,
        params: UpdateWitnessContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Update a super representative's brokerage ratio.
    fn update_brokerage(
        &self,
        params: UpdateBrokerageContract,
    ) -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    /// Fetch the brokerage ratio (0–100) for a super representative.
    fn get_brokerage(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    /// Fetch the unclaimed reward amount for an address (alias for
    /// [`crate::provider::TronProvider::get_reward`]).
    ///
    /// Unlike [`crate::provider::TronProvider::get_reward`] which returns [`Trx`], this returns the
    /// raw sun value.
    fn get_reward_info(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<u64, Self::Error>> + Send;
}
