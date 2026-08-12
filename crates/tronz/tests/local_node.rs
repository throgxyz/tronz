//! End-to-end coverage against a disposable TronBox Runtime Environment node.

use core::time::Duration;

use anyhow::{Context, Result, ensure};
use tronz::{
    Address, LocalSigner, ProviderBuilder, ResourceCode, TronProvider, Trx, U256,
    contract::{ContractExt as _, Trc20Ext as _, event::decode_logs, trc20::ITRC20},
    primitives::{B256, Bytes, TxId},
    providers::{
        PendingTransaction,
        ext::{ExchangeApi as _, GovernanceApi as _, MarketApi as _, Trc10Api as _},
    },
};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:50051";
const GENESIS_PRIVATE_KEY: &str =
    "0000000000000000000000000000000000000000000000000000000000000001";
const GENESIS_ADDRESS: &str = "TMVQGm1qAQYVdetCeGRRkTWYYrLXuHK2HC";
const RECIPIENT_ADDRESS: &str = "TVdyt1s88BdiCjKt6K2YuoSmpWScZYK1QF";
// Generated from tests/contracts/src/LocalToken.sol with the adjacent Foundry config.
const LOCAL_TOKEN_BYTECODE: &str = "6080604052348015600e575f5ffd5b50335f8181526020818152604080832064e8d4a510009081905590519081527fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef910160405180910390a361034e806100655f395ff3fe608060405234801561000f575f5ffd5b5060043610610060575f3560e01c806306fdde031461006457806318160ddd146100a4578063313ce567146100be57806370a08231146100d857806395d89b41146100f7578063a9059cbb1461011b575b5f5ffd5b61008e6040518060400160405280600b81526020016a2637b1b0b6102a37b5b2b760a91b81525081565b60405161009b9190610230565b60405180910390f35b6100b064e8d4a5100081565b60405190815260200161009b565b6100c6600681565b60405160ff909116815260200161009b565b6100b06100e6366004610296565b5f6020819052908152604090205481565b61008e604051806040016040528060058152602001641313d0d05360da1b81525081565b61012e6101293660046102b6565b61013e565b604051901515815260200161009b565b335f908152602081905260408120548211156101975760405162461bcd60e51b8152602060048201526014602482015273696e73756666696369656e742062616c616e636560601b604482015260640160405180910390fd5b335f90815260208190526040812080548492906101b59084906102f2565b90915550506001600160a01b0383165f90815260208190526040812080548492906101e1908490610305565b90915550506040518281526001600160a01b0384169033907fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef9060200160405180910390a35060015b92915050565b602081525f82518060208401525f5b8181101561025c576020818601810151604086840101520161023f565b505f604082850101526040601f19601f83011684010191505092915050565b80356001600160a01b0381168114610291575f5ffd5b919050565b5f602082840312156102a6575f5ffd5b6102af8261027b565b9392505050565b5f5f604083850312156102c7575f5ffd5b6102d08361027b565b946020939093013593505050565b634e487b7160e01b5f52601160045260245ffd5b8181038181111561022a5761022a6102de565b8082018082111561022a5761022a6102de56fea2646970667358221220bdf49fa38e3fbe73966b88719d9f15a51bfdb5c4ba75369331d74dc574d3a84e64736f6c63430008230033";

fn endpoint() -> String {
    std::env::var("TRONZ_LOCAL_GRPC").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned())
}

async fn confirmed(
    pending: PendingTransaction,
) -> Result<tronz::providers::types::TransactionInfo> {
    pending
        .with_poll_interval(Duration::from_millis(250))
        .with_timeout(Duration::from_secs(30))
        .require_success()
        .get_receipt()
        .await
        .context("transaction confirmation")
}

#[tokio::test]
#[ignore = "requires a local TRE node"]
async fn local_node_is_ready() -> Result<()> {
    let provider = ProviderBuilder::new().connect_grpc(&endpoint()).await?;
    ensure!(provider.get_now_block().await?.number > 0, "TRE has not produced a block");
    Ok(())
}

#[tokio::test]
#[ignore = "requires a fresh local TRE node"]
async fn exercises_full_node_end_to_end() -> Result<()> {
    let signer = LocalSigner::from_hex(GENESIS_PRIVATE_KEY)?;
    let owner: Address = GENESIS_ADDRESS.parse()?;
    let recipient: Address = RECIPIENT_ADDRESS.parse()?;
    ensure!(signer.address() == owner, "TRE genesis fixture drifted");
    let provider = ProviderBuilder::new().with_signer(signer).connect_grpc(&endpoint()).await?;

    // Chain, block, node, account and negative-lookup RPCs.
    let head = provider.get_now_block().await?;
    ensure!(provider.get_block_by_number(head.number).await?.is_some());
    ensure!(provider.get_block_by_id(head.hash).await?.is_some());
    ensure!(!provider.get_blocks_by_latest_num(1).await?.is_empty());
    ensure!(!provider.get_blocks_by_limit(head.number, head.number + 1).await?.is_empty());
    ensure!(provider.get_transaction(TxId::ZERO).await?.is_none());
    ensure!(provider.get_transaction_info(TxId::ZERO).await?.is_none());
    ensure!(provider.get_account(owner).await?.is_activated);
    let fresh =
        LocalSigner::from_hex("000000000000000000000000000000000000000000000000000000000000002a")?;
    ensure!(!provider.get_account(fresh.address()).await?.is_activated);
    let _ = provider.get_account_resource(owner).await?;
    let _ = provider.get_account_net(owner).await?;
    ensure!(!provider.chain_parameters().await?.is_empty());
    ensure!(!provider.list_witnesses().await?.is_empty());
    let _ = provider.get_node_info().await?;
    let _ = provider.get_dynamic_properties().await?;
    let _ = provider.get_total_transactions().await?;
    let _ = provider.get_pending_size().await?;
    let _ = provider.list_nodes().await?;
    let _ = provider.get_bandwidth_prices().await?;
    let _ = provider.get_energy_prices().await?;
    let _ = provider.get_energy_price().await?;
    let _ = provider.get_memo_fee().await?;
    let _ = provider.get_next_maintenance_time().await?;
    let _ = provider.get_burn_trx().await?;
    let _ = provider.get_reward(owner).await?;
    let _ = provider.get_brokerage(owner).await?;
    let _ = provider.get_reward_info(owner).await?;
    let _ = provider.get_available_unfreeze_count(owner).await?;
    let now_ms =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64;
    let _ = provider.get_can_withdraw_unfreeze_amount(owner, now_ms).await?;
    ensure!(provider.list_proposals().await?.is_empty());
    ensure!(provider.list_exchanges().await?.is_empty());
    ensure!(provider.get_exchange_by_id(0).await?.is_none());
    ensure!(provider.get_market_order_by_id(B256::ZERO).await?.is_none());
    ensure!(provider.get_market_order_by_account(owner).await?.is_empty());
    ensure!(provider.get_market_pair_list().await?.is_empty());

    // Signed TRX transfer, transaction lookup, receipt and block transaction queries.
    let before = provider.get_account(recipient).await?.balance;
    let trx_receipt = confirmed(
        provider.send_trx().to(recipient).amount(Trx::from_sun_unchecked(1)).send().await?,
    )
    .await?;
    ensure!(provider.get_account(recipient).await?.balance == before + Trx::from_sun_unchecked(1));
    ensure!(provider.get_transaction(trx_receipt.tx_id).await?.is_some());
    ensure!(provider.get_transaction_info(trx_receipt.tx_id).await?.is_some());
    ensure!(provider.get_transaction_count_by_block_num(trx_receipt.block_number).await? > 0);
    ensure!(
        !provider.get_transaction_info_by_block_num(trx_receipt.block_number).await?.is_empty()
    );

    // Stake 2.0, voting power, delegation and delegation lookup APIs.
    let stake = Trx::from_sun_unchecked(100_000_000);
    confirmed(provider.freeze_balance().amount(stake).resource(ResourceCode::Energy).send().await?)
        .await?;
    let account = provider.get_account(owner).await?;
    ensure!(account.frozen_v2.iter().any(|f| f.resource == ResourceCode::Energy));
    let _ = provider.get_can_delegate_max(owner, ResourceCode::Energy).await?;
    let delegated = Trx::from_sun_unchecked(10_000_000);
    confirmed(
        provider
            .delegate_resource()
            .to(recipient)
            .amount(delegated)
            .resource(ResourceCode::Energy)
            .send()
            .await?,
    )
    .await?;
    ensure!(!provider.get_delegated_resource(owner, recipient).await?.is_empty());
    let _ = provider.get_delegated_resource_index(owner).await?;
    confirmed(
        provider
            .undelegate_resource()
            .receiver(recipient)
            .amount(delegated)
            .resource(ResourceCode::Energy)
            .send()
            .await?,
    )
    .await?;
    confirmed(provider.vote_witness().vote(owner, 1).send().await?).await?;
    ensure!(provider.get_account(owner).await?.votes.iter().any(|v| v.vote_address == owner));

    // Contract deploy, metadata, constant calls, energy estimation, state change and event decode.
    let bytecode = Bytes::from(hex::decode(LOCAL_TOKEN_BYTECODE)?);
    let deploy_receipt = confirmed(
        provider
            .deploy(bytecode)
            .name("LocalToken")
            .fee_limit(Trx::from_sun_unchecked(1_000_000_000))
            .send()
            .await?,
    )
    .await?;
    let contract = deploy_receipt.contract_address.context("missing deployed contract address")?;
    let metadata = provider.get_contract_info(contract).await?;
    ensure!(metadata.runtime_bytecode.as_ref().is_some_and(|code| !code.is_empty()));
    let token = provider.trc20(contract).caller(owner);
    ensure!(token.name().await? == "Local Token");
    ensure!(token.symbol().await? == "LOCAL");
    ensure!(token.decimals().await? == 6);
    ensure!(token.total_supply().await? == U256::from(1_000_000_000_000u64));
    ensure!(token.balance_of(owner).await? == U256::from(1_000_000_000_000u64));
    ensure!(token.transfer_call(recipient, U256::from(25u64)).estimate_energy().await? > 0);
    let token_receipt = confirmed(token.transfer(recipient, U256::from(25u64)).await?).await?;
    ensure!(token.balance_of(recipient).await? == U256::from(25u64));
    let events =
        decode_logs::<ITRC20::Transfer>(&token_receipt.logs).collect::<Result<Vec<_>, _>>()?;
    ensure!(events.len() == 1 && events[0].value == U256::from(25u64));

    // TRC10 creation, discovery, balance and transfer.
    confirmed(
        provider
            .issue_trc10()
            .name("LocalAsset")
            .abbr("LAS")
            .description("local integration fixture")
            .url("https://example.invalid")
            .total_supply(1_000_000)
            .start_offset_ms(60_000)
            .duration_ms(86_400_000)
            .send()
            .await?,
    )
    .await?;
    let assets = provider.get_asset_issue_by_account(owner).await?;
    let asset = assets.first().context("TRC10 asset was not created")?;
    ensure!(provider.get_asset_info(&asset.id).await?.is_some());
    ensure!(provider.trc10_balance(owner, &asset.id).await? == 1_000_000);
    confirmed(provider.transfer_trc10().to(recipient).token_id(&asset.id).amount(10).send().await?)
        .await?;
    ensure!(provider.trc10_balance(recipient, &asset.id).await? == 10);
    ensure!(!provider.get_asset_issue_list(0, 10).await?.is_empty());
    ensure!(provider.get_asset_issue_by_name("LocalAsset").await?.is_some());
    ensure!(!provider.get_asset_issue_list_by_name("LocalAsset").await?.is_empty());

    // Complete the Stake 2.0 lifecycle by entering the unbonding queue.
    confirmed(
        provider.unfreeze_balance().amount(stake).resource(ResourceCode::Energy).send().await?,
    )
    .await?;
    ensure!(!provider.get_account(owner).await?.unfrozen_v2.is_empty());

    Ok(())
}
