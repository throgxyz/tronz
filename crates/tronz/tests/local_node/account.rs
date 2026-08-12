use anyhow::Result;
use tronz::{
    TronProvider, Trx,
    providers::types::{ContractKind, OperationSet, Permission, PermissionKey},
};

use super::{fixtures::TreTestContext, support::wait_for_confirmed_transaction};

const SECONDARY_ACCOUNT_FUNDING: Trx = Trx::from_sun_unchecked(200_000_000);
const ACTIVE_PERMISSION_ID: i32 = 2;

pub(crate) async fn prepare_secondary_account(ctx: &TreTestContext) -> Result<()> {
    activate_secondary_account(ctx).await?;
    name_secondary_account(ctx).await
}

async fn activate_secondary_account(ctx: &TreTestContext) -> Result<()> {
    assert!(
        !ctx.genesis_provider.get_account(ctx.secondary_account).await?.is_activated,
        "secondary account already exists; the suite needs a fresh TRE node"
    );
    wait_for_confirmed_transaction(
        ctx.genesis_provider.create_account().account_address(ctx.secondary_account).send().await?,
    )
    .await?;
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .send_trx()
            .to(ctx.secondary_account)
            .amount(SECONDARY_ACCOUNT_FUNDING)
            .send()
            .await?,
    )
    .await?;
    assert!(ctx.genesis_provider.get_account(ctx.secondary_account).await?.is_activated);
    Ok(())
}

async fn name_secondary_account(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.secondary_provider.update_account_name().name("TronzSdkFixture").send().await?,
    )
    .await?;
    assert_eq!(
        ctx.genesis_provider.get_account(ctx.secondary_account).await?.name,
        "TronzSdkFixture"
    );
    // AccountInfo intentionally does not expose java-tron's account_id field, so
    // confirmation is the strongest assertion the public API allows here.
    wait_for_confirmed_transaction(
        ctx.secondary_provider.set_account_id().account_id("localfixture").send().await?,
    )
    .await?;
    Ok(())
}

pub(crate) async fn update_and_use_active_permission(ctx: &TreTestContext) -> Result<()> {
    grant_transfer_permission(ctx).await?;
    spend_with_transfer_permission(ctx).await
}

async fn grant_transfer_permission(ctx: &TreTestContext) -> Result<()> {
    let owner_permission = Permission {
        id: 0,
        permission_name: "owner".into(),
        threshold: 1,
        keys: vec![PermissionKey { address: ctx.secondary_account, weight: 1 }],
        operations: OperationSet::empty(),
    };
    let active_permission = Permission {
        id: ACTIVE_PERMISSION_ID,
        permission_name: "transfer".into(),
        threshold: 1,
        keys: vec![PermissionKey { address: ctx.secondary_account, weight: 1 }],
        operations: OperationSet::try_from([ContractKind::Transfer])?,
    };
    wait_for_confirmed_transaction(
        ctx.secondary_provider
            .update_permissions()
            .owner_permission(owner_permission)
            .actives(vec![active_permission])
            .send()
            .await?,
    )
    .await?;
    let permissions = ctx.genesis_provider.get_account(ctx.secondary_account).await?.permissions;
    assert!(permissions.allows(ACTIVE_PERMISSION_ID, ContractKind::Transfer));
    Ok(())
}

async fn spend_with_transfer_permission(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.secondary_provider
            .send_trx()
            .to(ctx.recipient_account)
            .amount(Trx::from_sun_unchecked(1))
            .permission_id(ACTIVE_PERMISSION_ID)
            .send()
            .await?,
    )
    .await?;
    Ok(())
}
