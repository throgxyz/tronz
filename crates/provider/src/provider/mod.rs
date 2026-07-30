//! The high-level [`TronProvider`] trait and its concrete implementations.

pub mod builder;
pub mod erased;
pub mod pending;
pub mod root;
pub mod solidity;

use std::collections::HashMap;

use async_trait::async_trait;
use auto_impl::auto_impl;
pub use builder::{FilledProvider, ProviderBuilder};
pub use erased::DynProvider;
pub use pending::{PendingTransaction, PendingTransactionError};
pub use root::RootProvider;
pub use solidity::{SolidityProvider, SolidityProviderBuilder};
use tronz_primitives::{Address, B256, ResourceCode, Trx, TxId};

use crate::{
    builders::{
        AccountPermissionUpdateBuilder, CancelAllUnfreezeBuilder, ClearContractAbiBuilder,
        CreateAccountBuilder, DelegateBuilder, FreezeBuilder, FreezeV1Builder, SetAccountIdBuilder,
        TransferBuilder, UndelegateBuilder, UnfreezeBuilder, UnfreezeV1Builder,
        UpdateAccountBuilder, UpdateContractEnergyLimitBuilder, UpdateContractSettingBuilder,
        VoteBuilder, WithdrawBalanceBuilder, WithdrawExpireBuilder,
    },
    error::{Error, Result},
    transport::TronTransport,
    types::{
        AccountInfo, AccountNet, AccountResource, BlockInfo, ChainProperties, ConstantCallResult,
        DelegatedResource, DelegatedResourceIndex, MAX_RESULT_SIZE_IN_TX, NodeAddress, NodeInfo,
        RawTransaction, SignWeight, SignedTransaction, SmartContractInfo, TransactionInfo,
        TransactionRequest, TriggerSmartContract, WitnessInfo,
    },
};

/// The most recent price out of a `timestamp:sun,timestamp:sun,…` schedule.
pub(crate) fn latest_price(schedule: &str) -> Option<i64> {
    schedule.rsplit(',').find_map(|entry| entry.rsplit_once(':')?.1.trim().parse().ok())
}

/// Route a request's contract to the transport call that builds it, then apply
/// the request-level overrides the node does not know about.
///
/// Shared by [`TronProvider::build_transaction`] and `FilledProvider`'s
/// filler-aware override.
pub(crate) async fn build_via_transport(
    transport: &dyn TronTransport,
    mut req: TransactionRequest,
) -> Result<RawTransaction> {
    use crate::types::ContractType;

    let contract = req.contract.take().ok_or(Error::missing_field("contract"))?;

    let raw_result = match contract {
        ContractType::Transfer(c) => transport.transfer_trx(c).await,
        ContractType::TriggerSmartContract(c) => transport.trigger_smart_contract(c).await,
        ContractType::FreezeBalanceV1(c) => transport.freeze_balance_v1(c).await,
        ContractType::UnfreezeBalanceV1(c) => transport.unfreeze_balance_v1(c).await,
        ContractType::FreezeBalanceV2(c) => transport.freeze_balance_v2(c).await,
        ContractType::UnfreezeBalanceV2(c) => transport.unfreeze_balance_v2(c).await,
        ContractType::DelegateResource(c) => transport.delegate_resource(c).await,
        ContractType::UnDelegateResource(c) => transport.undelegate_resource(c).await,
        ContractType::WithdrawExpireUnfreeze(c) => transport.withdraw_expire_unfreeze(c).await,
        ContractType::CancelAllUnfreezeV2(c) => transport.cancel_all_unfreeze_v2(c).await,
        ContractType::WithdrawBalance(c) => transport.withdraw_balance(c).await,
        ContractType::AccountPermissionUpdate(c) => transport.account_permission_update(c).await,
        ContractType::CreateSmartContract(c) => transport.create_smart_contract(c).await,
        ContractType::AssetIssue(c) => transport.create_asset_issue(c).await,
        ContractType::TransferAsset(c) => transport.transfer_asset(c).await,
        ContractType::ParticipateAssetIssue(c) => transport.participate_asset_issue(c).await,
        ContractType::UnfreezeAsset(c) => transport.unfreeze_asset(c).await,
        ContractType::UpdateAsset(c) => transport.update_asset(c).await,
        ContractType::CreateAccount(c) => transport.create_account(c).await,
        ContractType::VoteWitness(c) => transport.vote_witness_account(c).await,
        ContractType::UpdateAccount(c) => transport.update_account(c).await,
        ContractType::ProposalCreate(c) => transport.proposal_create(c).await,
        ContractType::ProposalApprove(c) => transport.proposal_approve(c).await,
        ContractType::ProposalDelete(c) => transport.proposal_delete(c).await,
        ContractType::CreateWitness(c) => transport.create_witness(c).await,
        ContractType::UpdateWitness(c) => transport.update_witness(c).await,
        ContractType::UpdateBrokerage(c) => transport.update_brokerage(c).await,
        ContractType::SetAccountId(c) => transport.set_account_id(c).await,
        ContractType::ClearContractAbi(c) => transport.clear_contract_abi(c).await,
        ContractType::UpdateSetting(c) => transport.update_setting(c).await,
        ContractType::UpdateEnergyLimit(c) => transport.update_energy_limit(c).await,
        ContractType::ExchangeCreate(c) => transport.exchange_create(c).await,
        ContractType::ExchangeInject(c) => transport.exchange_inject(c).await,
        ContractType::ExchangeWithdraw(c) => transport.exchange_withdraw(c).await,
        ContractType::ExchangeTransaction(c) => transport.exchange_transaction(c).await,
        ContractType::MarketSellAsset(c) => transport.market_sell_asset(c).await,
        ContractType::MarketCancelOrder(c) => transport.market_cancel_order(c).await,
    };

    let mut raw = raw_result.map_err(Error::transport)?;
    raw.apply_request_fields(&req).map_err(|e| Error::Transport(e.into()))?;
    Ok(raw)
}

/// The provider capabilities required for contract calls and event queries.
///
/// Both FullNode providers and [`SolidityProvider`] implement this trait.
/// FullNode implementations read the latest available state, while SolidityNode
/// implementations read solidified state.
#[async_trait]
#[auto_impl(&, Arc)]
pub trait ContractReadProvider: Send + Sync + 'static {
    /// Borrow the provider one step down the stack, if this one wraps another.
    ///
    /// The counterpart of [`TronProvider::inner`], and every method below asks it
    /// first, so a wrapper supplies this one method and inherits the rest. A
    /// provider at the bottom of a stack leaves it `None` and answers for itself.
    fn inner_read(&self) -> Option<&dyn ContractReadProvider> {
        None
    }

    /// The default caller used to populate `owner_address`, if one is known.
    ///
    /// FullNode providers return their attached signer's address. Read-only
    /// providers may return `None`, in which case a caller must be supplied by
    /// the contract call builder.
    fn default_caller(&self) -> Option<Address> {
        self.inner_read().and_then(ContractReadProvider::default_caller)
    }

    /// Execute a constant contract call.
    async fn call_contract(&self, params: TriggerSmartContract) -> Result<ConstantCallResult> {
        match self.inner_read() {
            Some(inner) => inner.call_contract(params).await,
            None => Err(bottom_of_the_stack("call_contract")),
        }
    }

    /// Estimate the energy consumed by a contract call.
    async fn estimate_contract_energy(&self, params: TriggerSmartContract) -> Result<i64> {
        match self.inner_read() {
            Some(inner) => inner.estimate_contract_energy(params).await,
            None => Err(bottom_of_the_stack("estimate_contract_energy")),
        }
    }

    /// Fetch a transaction's receipt for event decoding.
    async fn transaction_info(&self, tx_id: TxId) -> Result<Option<TransactionInfo>> {
        match self.inner_read() {
            Some(inner) => inner.transaction_info(tx_id).await,
            None => Err(bottom_of_the_stack("transaction_info")),
        }
    }

    /// Fetch all transaction receipts in a block for event decoding.
    async fn transaction_infos_by_block(&self, block_num: i64) -> Result<Vec<TransactionInfo>> {
        match self.inner_read() {
            Some(inner) => inner.transaction_infos_by_block(block_num).await,
            None => Err(bottom_of_the_stack("transaction_infos_by_block")),
        }
    }
}

/// A provider that wraps nothing has to answer the contract reads itself.
///
/// Only reachable from a hand-written provider that neither implements a read nor
/// reports an [`inner_read`](ContractReadProvider::inner_read) to pass it to;
/// [`RootProvider`] and [`SolidityProvider`] both answer all four.
fn bottom_of_the_stack(method: &'static str) -> Error {
    Error::local_usage_str(&format!(
        "`{method}` reached a provider that implements neither it nor `inner_read`"
    ))
}

/// The primary user-facing interface: reads, lazy operation builders, and
/// low-level send/broadcast.
///
/// Downstream crates may implement this to wrap a provider — see
/// [`ProviderLayer`](crate::ProviderLayer). A wrapper supplies [`root`](Self::root)
/// and [`inner`](Self::inner) and overrides only what it cares about; everything
/// else travels down the stack and reaches the node at the bottom.
#[async_trait]
pub trait TronProvider: ContractReadProvider {
    /// Borrow the provider this one is ultimately built on.
    ///
    /// This is the only method an implementation must supply. A wrapper forwards it
    /// to what it wraps.
    fn root(&self) -> &RootProvider;

    /// Borrow the provider one step down the stack, if this one wraps another.
    ///
    /// Every default method below asks this first and only reaches the transport
    /// once nothing is left underneath. So a wrapper — metrics, caching, rate
    /// limiting — supplies `inner` and overrides just the handful of methods it
    /// cares about: the rest keep going down the stack on their own rather than
    /// jumping to the root and stepping over whatever it wraps.
    ///
    /// Each wrapper costs one dynamic dispatch per call.
    fn inner(&self) -> Option<&dyn TronProvider> {
        None
    }

    /// Borrow the transport, through [`root`](Self::root).
    fn transport(&self) -> &dyn TronTransport {
        self.root().transport()
    }

    /// The attached signer's address, if any.
    fn signer_address(&self) -> Option<Address> {
        match self.inner() {
            Some(inner) => inner.signer_address(),
            None => self.root().signer_address(),
        }
    }

    /// Erase this provider's type.
    ///
    /// See [`DynProvider`] for when that is worth an extra pointer hop per call.
    fn erased(self) -> DynProvider
    where
        Self: Sized,
    {
        DynProvider::new(self)
    }

    /// Fetch the latest block.
    async fn get_now_block(&self) -> Result<BlockInfo> {
        match self.inner() {
            Some(inner) => inner.get_now_block().await,
            None => self.transport().get_now_block().await.map_err(Error::transport),
        }
    }

    /// Fetch a block by height, or `None` if the chain has not reached it.
    async fn get_block_by_number(&self, num: i64) -> Result<Option<BlockInfo>> {
        match self.inner() {
            Some(inner) => inner.get_block_by_number(num).await,
            None => self.transport().get_block_by_number(num).await.map_err(Error::transport),
        }
    }

    /// Fetch on-chain account state.
    async fn get_account(&self, address: Address) -> Result<AccountInfo> {
        match self.inner() {
            Some(inner) => inner.get_account(address).await,
            None => self.transport().get_account(address).await.map_err(Error::transport),
        }
    }

    /// Fetch account resource usage.
    async fn get_account_resource(&self, address: Address) -> Result<AccountResource> {
        match self.inner() {
            Some(inner) => inner.get_account_resource(address).await,
            None => self.transport().get_account_resource(address).await.map_err(Error::transport),
        }
    }

    /// Fetch a transaction by id, or `None` if the node has never seen it.
    async fn get_transaction(&self, tx_id: TxId) -> Result<Option<SignedTransaction>> {
        match self.inner() {
            Some(inner) => inner.get_transaction(tx_id).await,
            None => self.transport().get_transaction_by_id(tx_id).await.map_err(Error::transport),
        }
    }

    /// Fetch a transaction's receipt/info.
    ///
    /// Returns `None` if the node has not yet indexed the transaction.
    /// Use [`PendingTransaction::get_receipt`] to poll until confirmed.
    async fn get_transaction_info(&self, tx_id: TxId) -> Result<Option<TransactionInfo>> {
        self.transaction_info(tx_id).await
    }

    /// Query delegations between two accounts (Stake 1.0, legacy).
    async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>> {
        match self.inner() {
            Some(inner) => inner.get_delegated_resource_v1(from, to).await,
            None => {
                self.transport().get_delegated_resource_v1(from, to).await.map_err(Error::transport)
            }
        }
    }

    /// Query the delegation index for an account (Stake 1.0, legacy).
    async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex> {
        match self.inner() {
            Some(inner) => inner.get_delegated_resource_index_v1(address).await,
            None => self
                .transport()
                .get_delegated_resource_index_v1(address)
                .await
                .map_err(Error::transport),
        }
    }

    /// Query delegations between two accounts (Stake 2.0).
    async fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>> {
        match self.inner() {
            Some(inner) => inner.get_delegated_resource(from, to).await,
            None => {
                self.transport().get_delegated_resource(from, to).await.map_err(Error::transport)
            }
        }
    }

    /// Query the delegation index for an account (Stake 2.0).
    async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex> {
        match self.inner() {
            Some(inner) => inner.get_delegated_resource_index(address).await,
            None => self
                .transport()
                .get_delegated_resource_index(address)
                .await
                .map_err(Error::transport),
        }
    }

    /// Query the max amount still delegatable for a resource.
    async fn get_can_delegate_max(&self, address: Address, resource: ResourceCode) -> Result<Trx> {
        match self.inner() {
            Some(inner) => inner.get_can_delegate_max(address, resource).await,
            None => self
                .transport()
                .get_can_delegate_max(address, resource)
                .await
                .map_err(Error::transport),
        }
    }

    /// Query the pending (unclaimed) reward.
    async fn get_reward(&self, address: Address) -> Result<Trx> {
        match self.inner() {
            Some(inner) => inner.get_reward(address).await,
            None => self.transport().get_reward(address).await.map_err(Error::transport),
        }
    }

    /// Fetch chain parameters.
    async fn chain_parameters(&self) -> Result<HashMap<String, i64>> {
        match self.inner() {
            Some(inner) => inner.chain_parameters().await,
            None => self.transport().get_chain_parameters().await.map_err(Error::transport),
        }
    }

    /// Fetch contract metadata including the deployed runtime bytecode.
    async fn get_contract_info(&self, address: Address) -> Result<SmartContractInfo> {
        match self.inner() {
            Some(inner) => inner.get_contract_info(address).await,
            None => self.transport().get_contract_info(address).await.map_err(Error::transport),
        }
    }

    /// List all super representatives and candidates.
    async fn list_witnesses(&self) -> Result<Vec<WitnessInfo>> {
        match self.inner() {
            Some(inner) => inner.list_witnesses().await,
            None => self.transport().list_witnesses().await.map_err(Error::transport),
        }
    }

    /// Fetch a paginated list of witnesses sorted by real-time vote count.
    async fn get_paginated_now_witness_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<WitnessInfo>> {
        match self.inner() {
            Some(inner) => inner.get_paginated_now_witness_list(offset, limit).await,
            None => self
                .transport()
                .get_paginated_now_witness_list(offset, limit)
                .await
                .map_err(Error::transport),
        }
    }

    /// Fetch the bandwidth price schedule string.
    async fn get_bandwidth_prices(&self) -> Result<String> {
        match self.inner() {
            Some(inner) => inner.get_bandwidth_prices().await,
            None => self.transport().get_bandwidth_prices().await.map_err(Error::transport),
        }
    }

    /// Fetch the energy price schedule string.
    ///
    /// The node returns the whole history, as `timestamp:sun` pairs. For the price
    /// in force now, use [`get_energy_price`](Self::get_energy_price).
    async fn get_energy_prices(&self) -> Result<String> {
        match self.inner() {
            Some(inner) => inner.get_energy_prices().await,
            None => self.transport().get_energy_prices().await.map_err(Error::transport),
        }
    }

    /// The energy price in force now, in sun per unit of energy.
    ///
    /// Reads the last entry of the schedule from
    /// [`get_energy_prices`](Self::get_energy_prices).
    async fn get_energy_price(&self) -> Result<i64> {
        let schedule = self.get_energy_prices().await?;
        latest_price(&schedule).ok_or_else(|| {
            Error::Transport(crate::error::TransportErrorKind::Malformed(format!(
                "cannot read an energy price out of {schedule:?}"
            )))
        })
    }

    /// Fetch the memo fee schedule.
    async fn get_memo_fee(&self) -> Result<u64> {
        match self.inner() {
            Some(inner) => inner.get_memo_fee().await,
            None => self.transport().get_memo_fee().await.map_err(Error::transport),
        }
    }

    /// Fetch the next maintenance time (unix ms).
    async fn get_next_maintenance_time(&self) -> Result<i64> {
        match self.inner() {
            Some(inner) => inner.get_next_maintenance_time().await,
            None => self.transport().get_next_maintenance_time().await.map_err(Error::transport),
        }
    }

    /// Fetch the total amount of TRX burned.
    async fn get_burn_trx(&self) -> Result<u64> {
        match self.inner() {
            Some(inner) => inner.get_burn_trx().await,
            None => self.transport().get_burn_trx().await.map_err(Error::transport),
        }
    }

    /// Fetch the total number of transactions ever processed.
    async fn get_total_transactions(&self) -> Result<u64> {
        match self.inner() {
            Some(inner) => inner.get_total_transactions().await,
            None => self.transport().get_total_transactions().await.map_err(Error::transport),
        }
    }

    /// Fetch basic info about the connected node.
    async fn get_node_info(&self) -> Result<NodeInfo> {
        match self.inner() {
            Some(inner) => inner.get_node_info().await,
            None => self.transport().get_node_info().await.map_err(Error::transport),
        }
    }

    /// List known gossip-network peer addresses.
    async fn list_nodes(&self) -> Result<Vec<NodeAddress>> {
        match self.inner() {
            Some(inner) => inner.list_nodes().await,
            None => self.transport().list_nodes().await.map_err(Error::transport),
        }
    }

    /// Fetch dynamic chain properties.
    async fn get_dynamic_properties(&self) -> Result<ChainProperties> {
        match self.inner() {
            Some(inner) => inner.get_dynamic_properties().await,
            None => self.transport().get_dynamic_properties().await.map_err(Error::transport),
        }
    }

    /// Fetch a block by its hash, or `None` if the node has no such block.
    async fn get_block_by_id(&self, block_id: B256) -> Result<Option<BlockInfo>> {
        match self.inner() {
            Some(inner) => inner.get_block_by_id(block_id).await,
            None => self.transport().get_block_by_id(block_id).await.map_err(Error::transport),
        }
    }

    /// Fetch the `count` most recent blocks.
    async fn get_blocks_by_latest_num(&self, count: i64) -> Result<Vec<BlockInfo>> {
        match self.inner() {
            Some(inner) => inner.get_blocks_by_latest_num(count).await,
            None => {
                self.transport().get_blocks_by_latest_num(count).await.map_err(Error::transport)
            }
        }
    }

    /// Fetch blocks in the range `[start, end)`.
    async fn get_blocks_by_limit(&self, start: i64, end: i64) -> Result<Vec<BlockInfo>> {
        match self.inner() {
            Some(inner) => inner.get_blocks_by_limit(start, end).await,
            None => {
                self.transport().get_blocks_by_limit(start, end).await.map_err(Error::transport)
            }
        }
    }

    /// Count transactions in a block by block number.
    async fn get_transaction_count_by_block_num(&self, block_num: i64) -> Result<u64> {
        match self.inner() {
            Some(inner) => inner.get_transaction_count_by_block_num(block_num).await,
            None => self
                .transport()
                .get_transaction_count_by_block_num(block_num)
                .await
                .map_err(Error::transport),
        }
    }

    /// Fetch paginated transactions sent *from* an address.
    async fn get_transactions_from(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<RawTransaction>> {
        match self.inner() {
            Some(inner) => inner.get_transactions_from(address, offset, limit).await,
            None => self
                .transport()
                .get_transactions_from(address, offset, limit)
                .await
                .map_err(Error::transport),
        }
    }

    /// Fetch paginated transactions sent *to* an address.
    async fn get_transactions_to(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<RawTransaction>> {
        match self.inner() {
            Some(inner) => inner.get_transactions_to(address, offset, limit).await,
            None => self
                .transport()
                .get_transactions_to(address, offset, limit)
                .await
                .map_err(Error::transport),
        }
    }

    /// Fetch transaction infos for all transactions in a block.
    async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> Result<Vec<TransactionInfo>> {
        self.transaction_infos_by_block(block_num).await
    }

    /// Fetch the number of pending transactions.
    async fn get_pending_size(&self) -> Result<u64> {
        match self.inner() {
            Some(inner) => inner.get_pending_size().await,
            None => self.transport().get_pending_size().await.map_err(Error::transport),
        }
    }

    /// Fetch a single pending transaction by id.
    async fn get_transaction_from_pending(&self, tx_id: TxId) -> Result<RawTransaction> {
        match self.inner() {
            Some(inner) => inner.get_transaction_from_pending(tx_id).await,
            None => {
                self.transport().get_transaction_from_pending(tx_id).await.map_err(Error::transport)
            }
        }
    }

    /// Fetch all pending transactions.
    async fn get_pending_transactions(&self) -> Result<Vec<RawTransaction>> {
        match self.inner() {
            Some(inner) => inner.get_pending_transactions().await,
            None => self.transport().get_pending_transactions().await.map_err(Error::transport),
        }
    }

    /// Query sign-weight for a transaction: how much signature weight has been
    /// collected so far and whether the permission threshold is met.
    ///
    /// Pass the partially- or fully-signed [`SignedTransaction`] so the node can
    /// count the already-attached signatures.
    async fn get_transaction_sign_weight(&self, tx: &SignedTransaction) -> Result<SignWeight> {
        match self.inner() {
            Some(inner) => inner.get_transaction_sign_weight(tx).await,
            None => {
                self.transport().get_transaction_sign_weight(tx).await.map_err(Error::transport)
            }
        }
    }

    /// Fetch addresses that have already signed a transaction.
    async fn get_transaction_approved_list(&self, tx: &SignedTransaction) -> Result<Vec<Address>> {
        match self.inner() {
            Some(inner) => inner.get_transaction_approved_list(tx).await,
            None => {
                self.transport().get_transaction_approved_list(tx).await.map_err(Error::transport)
            }
        }
    }

    /// Fetch bandwidth/energy net-usage for an account.
    async fn get_account_net(&self, address: Address) -> Result<AccountNet> {
        match self.inner() {
            Some(inner) => inner.get_account_net(address).await,
            None => self.transport().get_account_net(address).await.map_err(Error::transport),
        }
    }

    /// Fetch the brokerage ratio for a super representative.
    async fn get_brokerage(&self, address: Address) -> Result<u64> {
        match self.inner() {
            Some(inner) => inner.get_brokerage(address).await,
            None => self.transport().get_brokerage(address).await.map_err(Error::transport),
        }
    }

    /// Fetch the unclaimed reward (raw sun) for an address.
    async fn get_reward_info(&self, address: Address) -> Result<u64> {
        match self.inner() {
            Some(inner) => inner.get_reward_info(address).await,
            None => self.transport().get_reward_info(address).await.map_err(Error::transport),
        }
    }

    /// Build a TRX transfer.
    fn send_trx(&self) -> TransferBuilder<'_, Self>
    where
        Self: Sized,
    {
        TransferBuilder::new(self)
    }

    /// Build a stake (freeze) operation (Stake 1.0, legacy).
    fn freeze_balance_v1(&self) -> FreezeV1Builder<'_, Self>
    where
        Self: Sized,
    {
        FreezeV1Builder::new(self)
    }

    /// Build an unstake (unfreeze) operation (Stake 1.0, legacy).
    fn unfreeze_balance_v1(&self) -> UnfreezeV1Builder<'_, Self>
    where
        Self: Sized,
    {
        UnfreezeV1Builder::new(self)
    }

    /// Build a stake (freeze) operation (Stake 2.0).
    fn freeze_balance(&self) -> FreezeBuilder<'_, Self>
    where
        Self: Sized,
    {
        FreezeBuilder::new(self)
    }

    /// Build an unstake (unfreeze) operation (Stake 2.0).
    fn unfreeze_balance(&self) -> UnfreezeBuilder<'_, Self>
    where
        Self: Sized,
    {
        UnfreezeBuilder::new(self)
    }

    /// Build a delegate-resource operation.
    fn delegate_resource(&self) -> DelegateBuilder<'_, Self>
    where
        Self: Sized,
    {
        DelegateBuilder::new(self)
    }

    /// Build an undelegate-resource operation.
    fn undelegate_resource(&self) -> UndelegateBuilder<'_, Self>
    where
        Self: Sized,
    {
        UndelegateBuilder::new(self)
    }

    /// Build a withdraw-expire-unfreeze operation.
    fn withdraw_expire_unfreeze(&self) -> WithdrawExpireBuilder<'_, Self>
    where
        Self: Sized,
    {
        WithdrawExpireBuilder::new(self)
    }

    /// Build a cancel-all-unfreeze operation.
    fn cancel_all_unfreeze(&self) -> CancelAllUnfreezeBuilder<'_, Self>
    where
        Self: Sized,
    {
        CancelAllUnfreezeBuilder::new(self)
    }

    /// Build a claim-rewards operation.
    fn claim_rewards(&self) -> WithdrawBalanceBuilder<'_, Self>
    where
        Self: Sized,
    {
        WithdrawBalanceBuilder::new(self)
    }

    /// Update account permissions (multisig).
    fn update_permissions(&self) -> AccountPermissionUpdateBuilder<'_, Self>
    where
        Self: Sized,
    {
        AccountPermissionUpdateBuilder::new(self)
    }

    /// Query how much TRX can be withdrawn from expired unfreeze windows.
    ///
    /// `timestamp_ms` is the reference time (unix milliseconds).
    /// Pass the current time to check what is withdrawable right now.
    async fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> Result<Trx> {
        match self.inner() {
            Some(inner) => inner.get_can_withdraw_unfreeze_amount(address, timestamp_ms).await,
            None => self
                .transport()
                .get_can_withdraw_unfreeze_amount(address, timestamp_ms)
                .await
                .map_err(Error::transport),
        }
    }

    /// Query how many more unfreeze operations the account can still initiate.
    ///
    /// TRON allows at most 32 concurrent unfreeze windows per account.
    async fn get_available_unfreeze_count(&self, address: Address) -> Result<i64> {
        match self.inner() {
            Some(inner) => inner.get_available_unfreeze_count(address).await,
            None => self
                .transport()
                .get_available_unfreeze_count(address)
                .await
                .map_err(Error::transport),
        }
    }

    /// Activate a new account on-chain.
    fn create_account(&self) -> CreateAccountBuilder<'_, Self>
    where
        Self: Sized,
    {
        CreateAccountBuilder::new(self)
    }

    /// Vote for super representatives.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use tronz_provider::TronProvider as _;
    /// # async fn run(provider: impl tronz_provider::TronProvider, sr: tronz_primitives::Address) -> tronz_provider::Result<()> {
    /// let pending = provider.vote_witness().vote(sr, 100).send().await?;
    /// # Ok(()) }
    /// ```
    fn vote_witness(&self) -> VoteBuilder<'_, Self>
    where
        Self: Sized,
    {
        VoteBuilder::new(self)
    }

    /// Update the account's on-chain name.
    fn update_account_name(&self) -> UpdateAccountBuilder<'_, Self>
    where
        Self: Sized,
    {
        UpdateAccountBuilder::new(self)
    }

    /// Set a short alphanumeric on-chain account ID (alias).
    ///
    /// Can only be done once per account. The ID must be unique network-wide.
    fn set_account_id(&self) -> SetAccountIdBuilder<'_, Self>
    where
        Self: Sized,
    {
        SetAccountIdBuilder::new(self)
    }

    /// Clear the ABI of a deployed smart contract.
    ///
    /// Only the contract owner can call this.
    fn clear_contract_abi(&self) -> ClearContractAbiBuilder<'_, Self>
    where
        Self: Sized,
    {
        ClearContractAbiBuilder::new(self)
    }

    /// Update the caller-energy-percentage setting on a smart contract.
    ///
    /// Only the contract owner can call this.
    fn update_contract_setting(&self) -> UpdateContractSettingBuilder<'_, Self>
    where
        Self: Sized,
    {
        UpdateContractSettingBuilder::new(self)
    }

    /// Update the per-call origin energy limit on a smart contract.
    ///
    /// Only the contract owner can call this.
    fn update_contract_energy_limit(&self) -> UpdateContractEnergyLimitBuilder<'_, Self>
    where
        Self: Sized,
    {
        UpdateContractEnergyLimitBuilder::new(self)
    }

    /// Estimate the energy a contract call would consume.
    ///
    /// Mirrors [`estimate_gas`] in alloy: no state change, no signer required.
    /// Use this before [`send_transaction`] to set an appropriate `fee_limit`.
    ///
    /// [`estimate_gas`]: https://alloy.rs
    /// [`send_transaction`]: TronProvider::send_transaction
    async fn estimate_energy(&self, params: TriggerSmartContract) -> Result<i64> {
        self.estimate_contract_energy(params).await
    }

    /// Estimate the bandwidth (bytes) a signed transaction will consume on-chain.
    ///
    /// Includes java-tron's fixed transaction-result allowance.
    fn estimate_bandwidth(&self, tx: &SignedTransaction) -> u64 {
        tx.encoded_len() + MAX_RESULT_SIZE_IN_TX
    }

    /// Fill, sign, and broadcast a pre-built request.
    ///
    /// The default implementation returns [`Error::no_signer`] — a signer filler
    /// (e.g. `WalletFiller`) must be in the filler chain for this to succeed.
    async fn send_transaction(&self, req: TransactionRequest) -> Result<PendingTransaction> {
        match self.inner() {
            Some(inner) => inner.send_transaction(req).await,
            None => Err(Error::no_signer()),
        }
    }

    /// Ask the node to construct the transaction **without signing or
    /// broadcasting it**.
    ///
    /// Returns the unsigned [`RawTransaction`]. Sign it once (single-sig) or
    /// collect several signatures (multisig) and submit through
    /// [`broadcast`](Self::broadcast). For the common single-signer case prefer
    /// [`send_transaction`](Self::send_transaction), which fills, signs, and
    /// broadcasts in one step.
    ///
    /// The default implementation runs no fillers, so a client-side fee limit or
    /// TAPOS override must already be present on `req`. `FilledProvider` runs its
    /// filler chain first.
    async fn build_transaction(&self, req: TransactionRequest) -> Result<RawTransaction> {
        match self.inner() {
            Some(inner) => inner.build_transaction(req).await,
            None => build_via_transport(self.transport(), req).await,
        }
    }

    /// Broadcast an already-signed transaction.
    ///
    /// A broadcast whose outcome is left open reports [`Error::Broadcast`], carrying
    /// the transaction's id — the same as
    /// [`send_transaction`](Self::send_transaction) does.
    async fn broadcast(&self, tx: SignedTransaction) -> Result<PendingTransaction> {
        if let Some(inner) = self.inner() {
            return inner.broadcast(tx).await;
        }

        let tx_id = tx.raw.tx_id();
        self.transport()
            .broadcast_transaction(&tx)
            .await
            .map_err(|source| Error::broadcast(tx_id, source))?;
        Ok(PendingTransaction::new(self.root().clone(), tx_id))
    }
}

#[cfg(test)]
mod tests {
    use prost::Message as _;
    use tronz_rpc_types::{proto, types::RawTransaction};

    use super::*;
    use crate::{RootProvider, transport::mock::MockTransport};

    #[test]
    fn a_bandwidth_estimate_adds_the_result_allowance_to_the_wire_size() {
        let encoded = proto::Transaction {
            raw_data: Some(proto::transaction::Raw::default()),
            ..Default::default()
        }
        .encode_to_vec();
        let tx = SignedTransaction {
            raw: RawTransaction::from_node_encoded(encoded, &[]).unwrap(),
            signatures: Vec::new(),
        };
        let provider = RootProvider::new(MockTransport::new());

        assert_eq!(provider.estimate_bandwidth(&tx), tx.encoded_len() + 64);
    }
}
