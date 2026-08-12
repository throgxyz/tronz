use anyhow::Result;
use tronz::{
    Address, ResourceCode, SolidityProvider, TronProvider,
    primitives::{B256, TxId},
    providers::ext::{ExchangeApi as _, GovernanceApi as _, MarketApi as _},
};

use super::fixtures::TreTestContext;

pub(crate) async fn verify_chain_reads(ctx: &TreTestContext) -> Result<()> {
    verify_block_reads(ctx).await?;
    verify_account_reads(ctx).await?;

    let provider = &ctx.genesis_provider;
    assert!(provider.get_transaction(TxId::ZERO).await?.is_none());
    assert!(provider.get_transaction_info(TxId::ZERO).await?.is_none());
    assert!(!provider.chain_parameters().await?.is_empty());
    assert!(!TronProvider::list_witnesses(provider).await?.is_empty());
    assert!(!provider.get_paginated_now_witness_list(0, 10).await?.is_empty());
    Ok(())
}

async fn verify_block_reads(ctx: &TreTestContext) -> Result<()> {
    let provider = &ctx.genesis_provider;
    let head = provider.get_now_block().await?;
    assert_eq!(
        provider.get_block_by_number(head.number).await?.map(|block| block.hash),
        Some(head.hash)
    );
    assert_eq!(
        provider.get_block_by_id(head.hash).await?.map(|block| block.number),
        Some(head.number)
    );
    assert!(provider.get_block_by_number(i64::MAX).await?.is_none());
    assert!(provider.get_block_by_id(B256::ZERO).await?.is_none());
    assert!(!provider.get_blocks_by_latest_num(1).await?.is_empty());
    assert!(!provider.get_blocks_by_limit(head.number, head.number + 1).await?.is_empty());
    Ok(())
}

async fn verify_account_reads(ctx: &TreTestContext) -> Result<()> {
    let provider = &ctx.genesis_provider;
    assert!(provider.get_account(ctx.genesis_account).await?.is_activated);
    let account_resource = provider.get_account_resource(ctx.genesis_account).await?;
    let account_net = provider.get_account_net(ctx.genesis_account).await?;
    // Every activated account gets a free bandwidth allowance, and both endpoints
    // report it from the same chain parameter.
    assert!(account_resource.free_bandwidth_limit > 0);
    assert_eq!(account_resource.free_bandwidth_limit, account_net.free_net_limit);
    Ok(())
}

/// Covers the read surface that has no stable fixture value on TRE. Reaching the
/// end verifies transport support and successful decoding rather than chain state.
pub(crate) async fn verify_node_extension_reads(ctx: &TreTestContext) -> Result<()> {
    let provider = &ctx.genesis_provider;
    let genesis_account = ctx.genesis_account;

    let _ = provider.get_node_info().await?;
    let _ = provider.get_dynamic_properties().await?;
    let _ = provider.get_total_transactions().await?;
    let _ = provider.get_pending_size().await?;
    let _ = provider.get_pending_transactions().await?;
    assert!(provider.get_transaction_from_pending(TxId::ZERO).await.is_err());

    // TRE does not run the optional WalletExtension transaction-history service.
    assert!(provider.get_transactions_from(genesis_account, 0, 10).await.is_err());
    assert!(provider.get_transactions_to(genesis_account, 0, 10).await.is_err());

    let _ = provider.list_nodes().await?;
    let _ = provider.get_bandwidth_prices().await?;
    let _ = provider.get_energy_prices().await?;
    let _ = provider.get_energy_price().await?;
    let _ = provider.get_memo_fee().await?;
    let _ = provider.get_next_maintenance_time().await?;
    let _ = provider.get_burn_trx().await?;
    let _ = provider.get_reward(genesis_account).await?;
    let _ = TronProvider::get_brokerage(provider, genesis_account).await?;
    let _ = TronProvider::get_reward_info(provider, genesis_account).await?;
    let _ = provider.get_available_unfreeze_count(genesis_account).await?;
    let _ = provider.get_can_withdraw_unfreeze_amount(genesis_account, now_unix_ms()?).await?;
    Ok(())
}

/// Must run before [`super::governance`] and [`super::exchange`] create entries.
pub(crate) async fn verify_registries_are_empty(ctx: &TreTestContext) -> Result<()> {
    let provider = &ctx.genesis_provider;
    assert!(provider.list_proposals().await?.is_empty());
    assert!(provider.get_paginated_proposal_list(0, 10).await?.is_empty());
    assert!(provider.list_exchanges().await?.is_empty());
    assert!(provider.get_paginated_exchange_list(0, 10).await?.is_empty());
    assert!(provider.get_exchange_by_id(0).await?.is_none());
    assert!(provider.get_market_order_by_id(B256::ZERO).await?.is_none());
    assert!(provider.get_market_order_by_account(ctx.genesis_account).await?.is_empty());
    assert!(provider.get_market_pair_list().await?.is_empty());
    Ok(())
}

pub(crate) async fn verify_solidity_node_reads(
    solidity_provider: &SolidityProvider,
    genesis_account: Address,
    recipient_account: Address,
) -> Result<()> {
    let head = solidity_provider.get_now_block().await?;
    assert!(head.number > 0, "TRE has not solidified a block");
    assert_eq!(
        solidity_provider.get_block_by_number(head.number).await?.map(|block| block.hash),
        Some(head.hash)
    );
    assert!(solidity_provider.get_block_by_number(i64::MAX).await?.is_none());
    assert!(solidity_provider.get_account(genesis_account).await?.is_activated);
    assert!(solidity_provider.get_transaction(TxId::ZERO).await?.is_none());
    assert!(solidity_provider.get_transaction_info(TxId::ZERO).await?.is_none());
    let _ = solidity_provider.get_transaction_count_by_block_num(head.number).await?;
    let _ = solidity_provider.get_transaction_info_by_block_num(head.number).await?;
    assert!(!solidity_provider.list_witnesses().await?.is_empty());
    assert!(!solidity_provider.get_paginated_now_witness_list(0, 10).await?.is_empty());
    let _ = solidity_provider.get_delegated_resource_v1(genesis_account, recipient_account).await?;
    let _ = solidity_provider.get_delegated_resource_index_v1(genesis_account).await?;
    let _ = solidity_provider.get_delegated_resource(genesis_account, recipient_account).await?;
    let _ = solidity_provider.get_delegated_resource_index(genesis_account).await?;
    let _ = solidity_provider.get_can_delegate_max(genesis_account, ResourceCode::Energy).await?;
    let _ = solidity_provider.get_available_unfreeze_count(genesis_account).await?;
    let _ =
        solidity_provider.get_can_withdraw_unfreeze_amount(genesis_account, now_unix_ms()?).await?;
    Ok(())
}

fn now_unix_ms() -> Result<i64> {
    Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64)
}
