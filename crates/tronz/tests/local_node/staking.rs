use anyhow::Result;
use tronz::{ResourceCode, TronProvider, Trx};

use super::{
    fixtures::TreTestContext,
    support::{assert_node_error_contains, wait_for_confirmed_transaction},
};

const STAKED_ENERGY: Trx = Trx::from_sun_unchecked(100_000_000);
const DELEGATED_ENERGY: Trx = Trx::from_sun_unchecked(10_000_000);

pub(crate) async fn stake_delegate_vote_and_unfreeze(ctx: &TreTestContext) -> Result<()> {
    reject_stake_v1(ctx).await?;
    stake_energy(ctx).await?;
    delegate_and_undelegate_energy(ctx).await?;
    vote_for_witness(ctx).await?;
    unfreeze_and_cancel(ctx).await
}

async fn reject_stake_v1(ctx: &TreTestContext) -> Result<()> {
    assert_node_error_contains(
        ctx.genesis_provider
            .freeze_balance_v1()
            .amount(Trx::from_sun_unchecked(1_000_000))
            .resource(ResourceCode::Bandwidth)
            .receiver(ctx.recipient_account)
            .send()
            .await,
        "Stake 1.0 while Stake 2.0 is enabled",
        "freeze v2 is open, old freeze is closed",
    )
}

async fn stake_energy(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .freeze_balance()
            .amount(STAKED_ENERGY)
            .resource(ResourceCode::Energy)
            .send()
            .await?,
    )
    .await?;
    let account = ctx.genesis_provider.get_account(ctx.genesis_account).await?;
    assert!(
        account
            .frozen_v2
            .iter()
            .any(|freeze| freeze.resource == ResourceCode::Energy && freeze.amount >= STAKED_ENERGY)
    );
    assert!(
        ctx.genesis_provider
            .get_can_delegate_max(ctx.genesis_account, ResourceCode::Energy)
            .await?
            >= DELEGATED_ENERGY
    );
    Ok(())
}

async fn delegate_and_undelegate_energy(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .delegate_resource()
            .to(ctx.recipient_account)
            .amount(DELEGATED_ENERGY)
            .resource(ResourceCode::Energy)
            .send()
            .await?,
    )
    .await?;
    assert!(
        !ctx.genesis_provider
            .get_delegated_resource(ctx.genesis_account, ctx.recipient_account)
            .await?
            .is_empty()
    );
    let delegation_index =
        ctx.genesis_provider.get_delegated_resource_index(ctx.genesis_account).await?;
    assert!(delegation_index.to_accounts.contains(&ctx.recipient_account));
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .undelegate_resource()
            .receiver(ctx.recipient_account)
            .amount(DELEGATED_ENERGY)
            .resource(ResourceCode::Energy)
            .send()
            .await?,
    )
    .await?;
    Ok(())
}

async fn vote_for_witness(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider.vote_witness().vote(ctx.genesis_account, 1).send().await?,
    )
    .await?;
    assert!(
        ctx.genesis_provider
            .get_account(ctx.genesis_account)
            .await?
            .votes
            .iter()
            .any(|vote| vote.vote_address == ctx.genesis_account)
    );
    Ok(())
}

async fn unfreeze_and_cancel(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .unfreeze_balance()
            .amount(STAKED_ENERGY)
            .resource(ResourceCode::Energy)
            .send()
            .await?,
    )
    .await?;
    assert!(!ctx.genesis_provider.get_account(ctx.genesis_account).await?.unfrozen_v2.is_empty());
    wait_for_confirmed_transaction(ctx.genesis_provider.cancel_all_unfreeze().send().await?)
        .await?;
    assert!(ctx.genesis_provider.get_account(ctx.genesis_account).await?.unfrozen_v2.is_empty());
    assert_node_error_contains(
        ctx.genesis_provider.withdraw_expire_unfreeze().send().await,
        "withdraw with no expired unfreeze balance",
        "no unFreeze balance to withdraw",
    )
}
