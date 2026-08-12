use anyhow::{Context, Result};
use tronz::{
    primitives::B256,
    providers::{
        ext::{ExchangeApi as _, MarketApi as _},
        types::ExchangeInfo,
    },
};

use super::{
    fixtures::TreTestContext,
    support::{assert_node_error_contains, wait_for_confirmed_transaction},
};

const TRX_TOKEN_ID: &str = "_";
const INITIAL_TRX_BALANCE: i64 = 1_000_000;
const INITIAL_ASSET_BALANCE: i64 = 1_000;
const INJECTED_TRX: i64 = 100_000;
const WITHDRAWN_TRX: i64 = 10_000;

pub(crate) async fn create_update_and_query_exchange(
    ctx: &TreTestContext,
    asset_id: &str,
) -> Result<()> {
    let exchange_id = create_exchange(ctx, asset_id).await?;
    inject_into_exchange(ctx, exchange_id).await?;
    withdraw_from_exchange(ctx, exchange_id).await?;
    reject_market_operations(ctx, asset_id).await
}

async fn create_exchange(ctx: &TreTestContext, asset_id: &str) -> Result<i64> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .exchange_create()
            .first_token_id(TRX_TOKEN_ID)
            .first_token_balance(INITIAL_TRX_BALANCE)
            .second_token_id(asset_id)
            .second_token_balance(INITIAL_ASSET_BALANCE)
            .send()
            .await?,
    )
    .await?;
    let exchange_id = ctx
        .genesis_provider
        .list_exchanges()
        .await?
        .iter()
        .map(|exchange| exchange.exchange_id)
        .max()
        .context("exchange was not created")?;
    let created = load_exchange(ctx, exchange_id).await?;
    assert_eq!(created.first_token_id, TRX_TOKEN_ID);
    assert_eq!(created.first_token_balance, INITIAL_TRX_BALANCE);
    assert_eq!(created.second_token_id, asset_id);
    assert_eq!(created.second_token_balance, INITIAL_ASSET_BALANCE);
    assert!(!ctx.genesis_provider.get_paginated_exchange_list(0, 10).await?.is_empty());
    Ok(exchange_id)
}

async fn inject_into_exchange(ctx: &TreTestContext, exchange_id: i64) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .exchange_inject()
            .exchange_id(exchange_id)
            .token_id(TRX_TOKEN_ID)
            .quant(INJECTED_TRX)
            .send()
            .await?,
    )
    .await?;
    let injected = load_exchange(ctx, exchange_id).await?;
    assert_eq!(injected.first_token_balance, INITIAL_TRX_BALANCE + INJECTED_TRX);
    assert_eq!(
        injected.second_token_balance,
        INITIAL_ASSET_BALANCE + scaled_asset_amount(INJECTED_TRX)
    );
    Ok(())
}

async fn withdraw_from_exchange(ctx: &TreTestContext, exchange_id: i64) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .exchange_withdraw()
            .exchange_id(exchange_id)
            .token_id(TRX_TOKEN_ID)
            .quant(WITHDRAWN_TRX)
            .send()
            .await?,
    )
    .await?;
    let withdrawn = load_exchange(ctx, exchange_id).await?;
    assert_eq!(withdrawn.first_token_balance, INITIAL_TRX_BALANCE + INJECTED_TRX - WITHDRAWN_TRX);
    assert_eq!(
        withdrawn.second_token_balance,
        INITIAL_ASSET_BALANCE + scaled_asset_amount(INJECTED_TRX)
            - scaled_asset_amount(WITHDRAWN_TRX)
    );
    Ok(())
}

/// TRE ships with the market transaction feature switched off, so both market
/// operations are expected to be rejected while the pair stays empty.
async fn reject_market_operations(ctx: &TreTestContext, asset_id: &str) -> Result<()> {
    assert_node_error_contains(
        ctx.genesis_provider
            .market_sell()
            .sell_token_id(asset_id)
            .sell_token_quantity(10)
            .buy_token_id(TRX_TOKEN_ID)
            .buy_token_quantity(1_000)
            .send()
            .await,
        "market sell while the committee feature is disabled",
        "Not support Market Transaction",
    )?;
    assert_node_error_contains(
        ctx.genesis_provider.market_cancel().order_id(B256::ZERO).send().await,
        "market cancel while the committee feature is disabled",
        "Not support Market Transaction",
    )?;
    assert!(
        ctx.genesis_provider.get_market_price_by_pair(TRX_TOKEN_ID, asset_id).await?.is_empty()
    );
    assert!(
        ctx.genesis_provider
            .get_market_order_list_by_pair(TRX_TOKEN_ID, asset_id)
            .await?
            .is_empty()
    );
    Ok(())
}

async fn load_exchange(ctx: &TreTestContext, exchange_id: i64) -> Result<ExchangeInfo> {
    ctx.genesis_provider.get_exchange_by_id(exchange_id).await?.context("missing exchange")
}

const fn scaled_asset_amount(trx_amount: i64) -> i64 {
    trx_amount * INITIAL_ASSET_BALANCE / INITIAL_TRX_BALANCE
}
