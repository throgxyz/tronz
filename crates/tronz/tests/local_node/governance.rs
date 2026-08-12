use anyhow::{Context, Result};
use tronz::{
    TronProvider, Trx,
    providers::{
        ext::{GovernanceApi as _, WitnessApi as _},
        types::ProposalState,
    },
};

use super::{fixtures::TreTestContext, support::wait_for_confirmed_transaction};

const GENESIS_WITNESS_URL: &str = "https://sr.example.invalid";
const CANDIDATE_WITNESS_URL: &str = "https://candidate.example.invalid";
const CANDIDATE_REGISTRATION_FUNDING: Trx = Trx::from_sun_unchecked(10_000_000_000);
/// `MAINTENANCE_TIME_INTERVAL`, the first chain parameter.
const MAINTENANCE_INTERVAL_PARAMETER: i64 = 0;
const MAINTENANCE_INTERVAL_MS: i64 = 21_600_000;

pub(crate) async fn update_witness_and_manage_proposal(ctx: &TreTestContext) -> Result<()> {
    update_witness_metadata(ctx).await?;
    manage_proposal_lifecycle(ctx).await
}

async fn update_witness_metadata(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider.update_witness().url(GENESIS_WITNESS_URL).send().await?,
    )
    .await?;
    assert!(TronProvider::list_witnesses(&ctx.genesis_provider).await?.iter().any(|witness| {
        witness.address == ctx.genesis_account && witness.url == GENESIS_WITNESS_URL
    }));
    wait_for_confirmed_transaction(
        ctx.genesis_provider.update_brokerage().brokerage(20).send().await?,
    )
    .await?;
    assert_eq!(TronProvider::get_brokerage(&ctx.genesis_provider, ctx.genesis_account).await?, 20);
    Ok(())
}

async fn manage_proposal_lifecycle(ctx: &TreTestContext) -> Result<()> {
    let provider = &ctx.genesis_provider;
    wait_for_confirmed_transaction(
        provider
            .submit_proposal()
            .parameter(MAINTENANCE_INTERVAL_PARAMETER, MAINTENANCE_INTERVAL_MS)
            .send()
            .await?,
    )
    .await?;
    let proposal_id = provider
        .list_proposals()
        .await?
        .iter()
        .map(|proposal| proposal.proposal_id)
        .max()
        .context("governance proposal was not created")?;
    let proposal = provider.get_proposal_by_id(proposal_id).await?;
    assert_eq!(proposal.proposer_address, Some(ctx.genesis_account));
    assert_eq!(
        proposal.parameters.get(&MAINTENANCE_INTERVAL_PARAMETER),
        Some(&MAINTENANCE_INTERVAL_MS)
    );

    wait_for_confirmed_transaction(
        provider.approve_proposal().proposal_id(proposal_id).send().await?,
    )
    .await?;
    assert!(
        provider.get_proposal_by_id(proposal_id).await?.approvals.contains(&ctx.genesis_account)
    );

    wait_for_confirmed_transaction(
        provider.cancel_proposal().proposal_id(proposal_id).send().await?,
    )
    .await?;
    assert_eq!(provider.get_proposal_by_id(proposal_id).await?.state, ProposalState::Canceled);
    Ok(())
}

pub(crate) async fn fund_and_register_witness_candidate(ctx: &TreTestContext) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .send_trx()
            .to(ctx.secondary_account)
            .amount(CANDIDATE_REGISTRATION_FUNDING)
            .send()
            .await?,
    )
    .await?;
    wait_for_confirmed_transaction(
        ctx.secondary_provider.become_witness().url(CANDIDATE_WITNESS_URL).send().await?,
    )
    .await?;
    assert!(
        TronProvider::list_witnesses(&ctx.genesis_provider)
            .await?
            .iter()
            .any(|witness| witness.address == ctx.secondary_account
                && witness.url == CANDIDATE_WITNESS_URL)
    );
    Ok(())
}
