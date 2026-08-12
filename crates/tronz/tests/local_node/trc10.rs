use anyhow::{Context, Result};
use tronz::providers::ext::Trc10Api as _;

use super::{fixtures::TreTestContext, support::wait_for_confirmed_transaction};

const ASSET_NAME: &str = "LocalAsset";
const ASSET_TOTAL_SUPPLY: i64 = 1_000_000;
const ASSET_TRANSFER_AMOUNT: i64 = 10;

pub(crate) async fn issue_update_and_transfer_trc10(ctx: &TreTestContext) -> Result<String> {
    let asset_id = issue_asset(ctx).await?;
    transfer_asset(ctx, &asset_id).await?;
    update_asset(ctx, &asset_id).await?;
    Ok(asset_id)
}

async fn issue_asset(ctx: &TreTestContext) -> Result<String> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .issue_trc10()
            .name(ASSET_NAME)
            .abbr("LAS")
            .description("local integration fixture")
            .url("https://example.invalid")
            .total_supply(ASSET_TOTAL_SUPPLY)
            .start_offset_ms(60_000)
            .duration_ms(86_400_000)
            .send()
            .await?,
    )
    .await?;
    let assets = ctx.genesis_provider.get_asset_issue_by_account(ctx.genesis_account).await?;
    let asset_id = assets.first().context("TRC10 asset was not created")?.id.clone();
    assert!(ctx.genesis_provider.get_asset_info(&asset_id).await?.is_some());
    assert_eq!(
        ctx.genesis_provider.trc10_balance(ctx.genesis_account, &asset_id).await?,
        ASSET_TOTAL_SUPPLY
    );
    Ok(asset_id)
}

async fn transfer_asset(ctx: &TreTestContext, asset_id: &str) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .transfer_trc10()
            .to(ctx.recipient_account)
            .token_id(asset_id)
            .amount(ASSET_TRANSFER_AMOUNT)
            .send()
            .await?,
    )
    .await?;
    assert_eq!(
        ctx.genesis_provider.trc10_balance(ctx.recipient_account, asset_id).await?,
        ASSET_TRANSFER_AMOUNT
    );
    assert!(!ctx.genesis_provider.get_asset_issue_list(0, 10).await?.is_empty());
    assert!(ctx.genesis_provider.get_asset_issue_by_name(ASSET_NAME).await?.is_some());
    assert!(!ctx.genesis_provider.get_asset_issue_list_by_name(ASSET_NAME).await?.is_empty());
    Ok(())
}

async fn update_asset(ctx: &TreTestContext, asset_id: &str) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .update_trc10()
            .description("updated local integration fixture")
            .url("https://updated.example.invalid")
            .new_limit(1_000)
            .new_public_limit(10_000)
            .send()
            .await?,
    )
    .await?;
    assert!(ctx.genesis_provider.get_asset_info(asset_id).await?.is_some());
    Ok(())
}
