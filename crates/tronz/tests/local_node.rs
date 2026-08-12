//! End-to-end coverage against a disposable TronBox Runtime Environment node.

use anyhow::{Context, Result};
use tronz::{Address, ProviderBuilder, SolidityProvider, TronProvider};

#[path = "local_node/account.rs"]
mod account;
#[path = "local_node/exchange.rs"]
mod exchange;
#[path = "local_node/fixtures.rs"]
mod fixtures;
#[path = "local_node/governance.rs"]
mod governance;
#[path = "local_node/reads.rs"]
mod reads;
#[path = "local_node/staking.rs"]
mod staking;
#[path = "local_node/support.rs"]
mod support;
#[path = "local_node/transfer.rs"]
mod transfer;
#[path = "local_node/trc10.rs"]
mod trc10;
#[path = "local_node/trc20.rs"]
mod trc20;
#[path = "local_node/trc721.rs"]
mod trc721;

use fixtures::{RECIPIENT_ACCOUNT_ADDRESS, TRE_GENESIS_ADDRESS, TreTestContext};
use support::{full_node_grpc_endpoint, solidity_node_grpc_endpoint};

#[tokio::test]
#[ignore = "requires a local TRE node"]
async fn full_node_is_ready() -> Result<()> {
    let full_node_provider =
        ProviderBuilder::new().connect_grpc(&full_node_grpc_endpoint()).await?;
    assert!(full_node_provider.get_now_block().await?.number > 0, "TRE has not produced a block");
    Ok(())
}

#[tokio::test]
#[ignore = "requires a local TRE Solidity node"]
async fn solidity_node_is_ready() -> Result<()> {
    let solidity_provider = SolidityProvider::connect(&solidity_node_grpc_endpoint()).await?;
    assert!(solidity_provider.get_now_block().await?.number > 0, "TRE has not solidified a block");
    Ok(())
}

#[tokio::test]
#[ignore = "requires a local TRE Solidity node"]
async fn reads_from_solidity_node() -> Result<()> {
    let genesis_account: Address = TRE_GENESIS_ADDRESS.parse()?;
    let recipient_account: Address = RECIPIENT_ACCOUNT_ADDRESS.parse()?;
    let solidity_provider = SolidityProvider::connect(&solidity_node_grpc_endpoint()).await?;
    reads::verify_solidity_node_reads(&solidity_provider, genesis_account, recipient_account).await
}

#[tokio::test]
#[ignore = "requires a fresh local TRE node"]
async fn full_node_scenario() -> Result<()> {
    let ctx = TreTestContext::set_up().await?;

    account::prepare_secondary_account(&ctx).await.context("secondary account setup")?;
    reads::verify_chain_reads(&ctx).await.context("chain reads")?;
    reads::verify_node_extension_reads(&ctx).await.context("node extension reads")?;
    // Must precede governance and exchange, which are what populate the registries.
    reads::verify_registries_are_empty(&ctx).await.context("empty registries")?;

    governance::update_witness_and_manage_proposal(&ctx).await.context("governance")?;
    transfer::send_and_validate_signed_transfers(&ctx).await.context("transfers")?;
    staking::stake_delegate_vote_and_unfreeze(&ctx).await.context("staking")?;
    trc20::deploy_and_transfer_trc20(&ctx).await.context("trc20")?;
    trc721::deploy_approve_and_transfer_trc721(&ctx).await.context("trc721")?;

    let asset_id = trc10::issue_update_and_transfer_trc10(&ctx).await.context("trc10")?;
    exchange::create_update_and_query_exchange(&ctx, &asset_id).await.context("exchange")?;

    account::update_and_use_active_permission(&ctx).await.context("active permission")?;

    // TRE rejects later account updates once an account holds witness permission,
    // so candidate registration goes last.
    governance::fund_and_register_witness_candidate(&ctx).await.context("witness candidate")?;
    Ok(())
}
