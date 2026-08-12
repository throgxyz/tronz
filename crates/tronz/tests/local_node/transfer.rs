use anyhow::Result;
use tronz::{
    TronProvider, TronSigner, Trx,
    providers::types::{SignedTransaction, TransactionInfo},
};

use super::{
    fixtures::TreTestContext,
    support::{configure_pending_transaction, wait_for_confirmed_transaction},
};

const TRANSFER_AMOUNT: Trx = Trx::from_sun_unchecked(1);

pub(crate) async fn send_and_validate_signed_transfers(ctx: &TreTestContext) -> Result<()> {
    let receipt = send_and_wait_for_solidification(ctx).await?;
    verify_transaction_lookups(ctx, &receipt).await?;
    sign_and_broadcast(ctx).await?;
    reject_unauthorized_signature(ctx).await
}

async fn send_and_wait_for_solidification(ctx: &TreTestContext) -> Result<TransactionInfo> {
    let before = ctx.genesis_provider.get_account(ctx.recipient_account).await?.balance;
    let pending = ctx
        .genesis_provider
        .send_trx()
        .to(ctx.recipient_account)
        .amount(TRANSFER_AMOUNT)
        .send()
        .await?;
    let pending = configure_pending_transaction(pending).require_success();
    let solidified_receipt = pending.get_solidified_receipt(&ctx.solidity_provider).await?;
    let receipt = pending.get_receipt().await?;
    assert_eq!(solidified_receipt.tx_id, receipt.tx_id);
    assert!(ctx.solidity_provider.get_transaction(receipt.tx_id).await?.is_some());
    assert!(
        ctx.solidity_provider.get_account(ctx.recipient_account).await?.balance
            >= before + TRANSFER_AMOUNT
    );
    assert_eq!(
        ctx.genesis_provider.get_account(ctx.recipient_account).await?.balance,
        before + TRANSFER_AMOUNT
    );
    Ok(receipt)
}

async fn verify_transaction_lookups(ctx: &TreTestContext, receipt: &TransactionInfo) -> Result<()> {
    let provider = &ctx.genesis_provider;
    assert!(provider.get_transaction(receipt.tx_id).await?.is_some());
    assert!(provider.get_transaction_info(receipt.tx_id).await?.is_some());
    assert!(provider.get_transaction_count_by_block_num(receipt.block_number).await? > 0);
    assert!(!provider.get_transaction_info_by_block_num(receipt.block_number).await?.is_empty());
    Ok(())
}

async fn sign_and_broadcast(ctx: &TreTestContext) -> Result<()> {
    let raw = ctx
        .genesis_provider
        .send_trx()
        .to(ctx.recipient_account)
        .amount(TRANSFER_AMOUNT)
        .build()
        .await?;
    let signature = ctx.genesis_signer.sign_hash(&raw.tx_id()).await?;
    let signed = SignedTransaction { raw, signatures: vec![signature] };
    assert!(ctx.genesis_provider.estimate_bandwidth(&signed) > 0);
    let sign_weight = ctx.genesis_provider.get_transaction_sign_weight(&signed).await?;
    assert!(sign_weight.current_weight >= sign_weight.required_weight);
    assert!(sign_weight.approved_list.contains(&ctx.genesis_account));
    assert!(
        ctx.genesis_provider
            .get_transaction_approved_list(&signed)
            .await?
            .contains(&ctx.genesis_account)
    );
    wait_for_confirmed_transaction(ctx.genesis_provider.broadcast(signed).await?).await?;
    Ok(())
}

/// The secondary key never holds a permission on the genesis account, so signing
/// a genesis transfer with it must fail weight validation and broadcast.
async fn reject_unauthorized_signature(ctx: &TreTestContext) -> Result<()> {
    let raw = ctx
        .genesis_provider
        .send_trx()
        .to(ctx.recipient_account)
        .amount(TRANSFER_AMOUNT)
        .build()
        .await?;
    let wrong_signature = ctx.secondary_signer.sign_hash(&raw.tx_id()).await?;
    let wrongly_signed = SignedTransaction { raw, signatures: vec![wrong_signature] };
    let sign_weight = ctx.genesis_provider.get_transaction_sign_weight(&wrongly_signed).await?;
    assert!(sign_weight.current_weight < sign_weight.required_weight);
    assert!(
        !ctx.genesis_provider
            .get_transaction_approved_list(&wrongly_signed)
            .await?
            .contains(&ctx.genesis_account)
    );
    assert!(ctx.genesis_provider.broadcast(wrongly_signed).await.is_err());
    Ok(())
}
