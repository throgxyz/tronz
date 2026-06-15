//! Transport abstraction over a TRON node's API.
//!
//! [`TronTransport`] is a domain-specific async trait; [`grpc`] provides the
//! default tonic-backed gRPC implementation targeting `grpc.trongrid.io:443`.

use core::future::Future;
use std::collections::HashMap;

use tronz_primitives::{Address, ResourceCode, Trx, TxId};

use crate::types::{
    AccountInfo, AccountPermissionUpdateContract, AccountResource, AssetInfo, AssetIssueContract,
    BlockInfo, ConstantCallResult, CreateAccountContract, CreateSmartContract, DelegatedResource,
    DelegatedResourceIndex, FreezeBalanceV2Contract, RawTransaction, SignedTransaction,
    SmartContractInfo, TransactionInfo, TransferAssetContract, TransferContract,
    TriggerSmartContract, UnDelegateResourceContract, UnfreezeBalanceV2Contract,
    UpdateAccountContract, VoteWitnessContract, WithdrawBalanceContract,
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
    /// [`crate::error::TransportError`] so that the provider layer can wrap it
    /// uniformly.
    type Error: std::error::Error + Into<crate::error::TransportError> + Send + Sync + 'static;

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
    fn get_transaction_info(
        &self,
        tx_id: TxId,
    ) -> impl Future<Output = Result<TransactionInfo, Self::Error>> + Send;

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

    /// Query delegations between two accounts.
    fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> impl Future<Output = Result<Vec<DelegatedResource>, Self::Error>> + Send;

    /// Query the full delegation index for an account.
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
    fn get_asset_issue_by_id(
        &self,
        token_id: &str,
    ) -> impl Future<Output = Result<AssetInfo, Self::Error>> + Send;

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
}
