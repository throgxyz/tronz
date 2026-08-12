use anyhow::{Context, Result};
use tronz::{
    Address, TronProvider, U256,
    contract::{ContractExt as _, JsonAbi, Trc20Ext as _, event::decode_logs, trc20::ITRC20},
    primitives::Bytes,
};

use super::{
    fixtures::TreTestContext,
    support::{
        DEPLOY_FEE_LIMIT, REVERTING_CALL_FEE_LIMIT, configure_pending_transaction,
        wait_for_confirmed_transaction, wait_for_transaction_receipt,
    },
};

// Generated with solc 0.8.35 and the adjacent Foundry config:
// forge build --root crates/tronz/tests/contracts --use 0.8.35
// jq -r '.bytecode.object' crates/tronz/tests/contracts/out/LocalToken.sol/LocalToken.json
const LOCAL_TOKEN_BYTECODE: &str = "6080604052348015600e575f5ffd5b50335f8181526020818152604080832064e8d4a510009081905590519081527fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef910160405180910390a361034e806100655f395ff3fe608060405234801561000f575f5ffd5b5060043610610060575f3560e01c806306fdde031461006457806318160ddd146100a4578063313ce567146100be57806370a08231146100d857806395d89b41146100f7578063a9059cbb1461011b575b5f5ffd5b61008e6040518060400160405280600b81526020016a2637b1b0b6102a37b5b2b760a91b81525081565b60405161009b9190610230565b60405180910390f35b6100b064e8d4a5100081565b60405190815260200161009b565b6100c6600681565b60405160ff909116815260200161009b565b6100b06100e6366004610296565b5f6020819052908152604090205481565b61008e604051806040016040528060058152602001641313d0d05360da1b81525081565b61012e6101293660046102b6565b61013e565b604051901515815260200161009b565b335f908152602081905260408120548211156101975760405162461bcd60e51b8152602060048201526014602482015273696e73756666696369656e742062616c616e636560601b604482015260640160405180910390fd5b335f90815260208190526040812080548492906101b59084906102f2565b90915550506001600160a01b0383165f90815260208190526040812080548492906101e1908490610305565b90915550506040518281526001600160a01b0384169033907fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef9060200160405180910390a35060015b92915050565b602081525f82518060208401525f5b8181101561025c576020818601810151604086840101520161023f565b505f604082850101526040601f19601f83011684010191505092915050565b80356001600160a01b0381168114610291575f5ffd5b919050565b5f602082840312156102a6575f5ffd5b6102af8261027b565b9392505050565b5f5f604083850312156102c7575f5ffd5b6102d08361027b565b946020939093013593505050565b634e487b7160e01b5f52601160045260245ffd5b8181038181111561022a5761022a6102de565b8082018082111561022a5761022a6102de56fea2646970667358221220bdf49fa38e3fbe73966b88719d9f15a51bfdb5c4ba75369331d74dc574d3a84e64736f6c63430008230033";
// jq -r '.bytecode.object' \
//   crates/tronz/tests/contracts/out/LocalTokenAllowance.sol/LocalTokenAllowance.json
const LOCAL_TOKEN_ALLOWANCE_BYTECODE: &str = "6080604052348015600e575f5ffd5b50335f8181526020818152604080832064e8d4a510009081905590519081527fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef910160405180910390a3610586806100655f395ff3fe608060405234801561000f575f5ffd5b5060043610610090575f3560e01c8063313ce56711610063578063313ce5671461012457806370a082311461013e57806395d89b411461015d578063a9059cbb14610181578063dd62ed3e14610194575f5ffd5b806306fdde0314610094578063095ea7b3146100d457806318160ddd146100f757806323b872dd14610111575b5f5ffd5b6100be6040518060400160405280600b81526020016a2637b1b0b6102a37b5b2b760a91b81525081565b6040516100cb91906103fd565b60405180910390f35b6100e76100e2366004610463565b6101be565b60405190151581526020016100cb565b61010364e8d4a5100081565b6040519081526020016100cb565b6100e761011f36600461048b565b61022a565b61012c600681565b60405160ff90911681526020016100cb565b61010361014c3660046104c5565b5f6020819052908152604090205481565b6100be604051806040016040528060058152602001641313d0d05360da1b81525081565b6100e761018f366004610463565b6102de565b6101036101a23660046104e5565b600160209081525f928352604080842090915290825290205481565b335f8181526001602090815260408083206001600160a01b038716808552925280832085905551919290917f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925906102189086815260200190565b60405180910390a35060015b92915050565b6001600160a01b0383165f9081526001602090815260408083203384529091528120548281101561029b5760405162461bcd60e51b8152602060048201526016602482015275696e73756666696369656e7420616c6c6f77616e636560501b60448201526064015b60405180910390fd5b6102a5838261052a565b6001600160a01b0386165f9081526001602090815260408083203384529091529020556102d38585856102f3565b506001949350505050565b5f6102ea3384846102f3565b50600192915050565b6001600160a01b0383165f908152602081905260409020548111156103515760405162461bcd60e51b8152602060048201526014602482015273696e73756666696369656e742062616c616e636560601b6044820152606401610292565b6001600160a01b0383165f908152602081905260408120805483929061037890849061052a565b90915550506001600160a01b0382165f90815260208190526040812080548392906103a490849061053d565b92505081905550816001600160a01b0316836001600160a01b03167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef836040516103f091815260200190565b60405180910390a3505050565b602081525f82518060208401525f5b81811015610429576020818601810151604086840101520161040c565b505f604082850101526040601f19601f83011684010191505092915050565b80356001600160a01b038116811461045e575f5ffd5b919050565b5f5f60408385031215610474575f5ffd5b61047d83610448565b946020939093013593505050565b5f5f5f6060848603121561049d575f5ffd5b6104a684610448565b92506104b460208501610448565b929592945050506040919091013590565b5f602082840312156104d5575f5ffd5b6104de82610448565b9392505050565b5f5f604083850312156104f6575f5ffd5b6104ff83610448565b915061050d60208401610448565b90509250929050565b634e487b7160e01b5f52601160045260245ffd5b8181038181111561022457610224610516565b808201808211156102245761022461051656fea26469706673582212208edbe90c17a330499856126858046c0d1c0d4e5045bec06f61c739c8442b762d64736f6c63430008230033";

const TOTAL_SUPPLY: u64 = 1_000_000_000_000;
const TRANSFER_AMOUNT: u64 = 25;
const APPROVED_AMOUNT: u64 = 40;
const DELEGATED_AMOUNT: u64 = 15;

pub(crate) async fn deploy_and_transfer_trc20(ctx: &TreTestContext) -> Result<()> {
    let address = deploy_local_token(ctx).await?;
    verify_token_metadata(ctx, address).await?;
    transfer_and_verify(ctx, address).await?;
    update_contract_limits(ctx, address).await?;
    approve_and_transfer_from(ctx).await?;
    clear_deployed_abi(ctx, address).await
}

async fn deploy_local_token(ctx: &TreTestContext) -> Result<Address> {
    let deploy = ctx
        .genesis_provider
        .deploy(Bytes::from(hex::decode(LOCAL_TOKEN_BYTECODE)?))
        .abi(JsonAbi::parse(["function balanceOf(address owner) view returns (uint256)"])?)
        .name("LocalToken")
        .fee_limit(DEPLOY_FEE_LIMIT)
        .send()
        .await?;
    let deploy = configure_pending_transaction(deploy).require_success();
    let solidified = deploy.get_solidified_receipt(&ctx.solidity_provider).await?;
    let receipt = deploy.get_receipt().await?;
    assert_eq!(solidified.tx_id, receipt.tx_id);
    let address = receipt.contract_address.context("missing deployed contract address")?;
    let metadata = ctx.genesis_provider.get_contract_info(address).await?;
    assert!(metadata.runtime_bytecode.as_ref().is_some_and(|code| !code.is_empty()));
    assert!(!metadata.abi.entries.is_empty());
    Ok(address)
}

async fn verify_token_metadata(ctx: &TreTestContext, address: Address) -> Result<()> {
    let token = ctx.genesis_provider.trc20(address).caller(ctx.genesis_account);
    assert_eq!(token.name().await?, "Local Token");
    assert_eq!(token.symbol().await?, "LOCAL");
    assert_eq!(token.decimals().await?, 6);
    assert_eq!(token.total_supply().await?, U256::from(TOTAL_SUPPLY));
    assert_eq!(token.balance_of(ctx.genesis_account).await?, U256::from(TOTAL_SUPPLY));

    let solidified_token = ctx.solidity_provider.trc20(address).caller(ctx.genesis_account);
    assert_eq!(solidified_token.name().await?, "Local Token");
    assert_eq!(solidified_token.balance_of(ctx.genesis_account).await?, U256::from(TOTAL_SUPPLY));
    Ok(())
}

async fn transfer_and_verify(ctx: &TreTestContext, address: Address) -> Result<()> {
    let token = ctx.genesis_provider.trc20(address).caller(ctx.genesis_account);
    let amount = U256::from(TRANSFER_AMOUNT);
    assert!(token.transfer_call(ctx.recipient_account, amount).estimate_energy().await? > 0);
    let transfer =
        wait_for_confirmed_transaction(token.transfer(ctx.recipient_account, amount).await?)
            .await?;
    assert_eq!(token.balance_of(ctx.recipient_account).await?, amount);
    let events = decode_logs::<ITRC20::Transfer>(&transfer.logs).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value, amount);

    reject_transfer_beyond_balance(ctx, address).await
}

async fn reject_transfer_beyond_balance(ctx: &TreTestContext, address: Address) -> Result<()> {
    let token = ctx.genesis_provider.trc20(address).caller(ctx.genesis_account);
    assert!(token.transfer_call(ctx.recipient_account, U256::MAX).call().await.is_err());
    let failed = token
        .transfer_call(ctx.recipient_account, U256::MAX)
        .fee_limit(REVERTING_CALL_FEE_LIMIT)
        .send()
        .await?;
    assert!(!wait_for_transaction_receipt(failed).await?.is_success());
    Ok(())
}

async fn update_contract_limits(ctx: &TreTestContext, address: Address) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .update_contract_setting()
            .contract_address(address)
            .consume_user_resource_percent(75)
            .send()
            .await?,
    )
    .await?;
    wait_for_confirmed_transaction(
        ctx.genesis_provider
            .update_contract_energy_limit()
            .contract_address(address)
            .origin_energy_limit(5_000_000)
            .send()
            .await?,
    )
    .await?;
    let metadata = ctx.genesis_provider.get_contract_info(address).await?;
    assert_eq!(metadata.consume_user_resource_percent, 75);
    assert_eq!(metadata.origin_energy_limit, 5_000_000);
    Ok(())
}

/// Runs last, because dropping the stored ABI is not reversible.
async fn clear_deployed_abi(ctx: &TreTestContext, address: Address) -> Result<()> {
    wait_for_confirmed_transaction(
        ctx.genesis_provider.clear_contract_abi().contract_address(address).send().await?,
    )
    .await?;
    assert!(ctx.genesis_provider.get_contract_info(address).await?.abi.entries.is_empty());
    Ok(())
}

/// The allowance flow needs a separate deployment because `LocalToken` has no
/// `approve`/`transferFrom`.
async fn approve_and_transfer_from(ctx: &TreTestContext) -> Result<()> {
    let address = deploy_allowance_token(ctx).await?;
    approve_secondary_account(ctx, address).await?;
    transfer_from_genesis_account(ctx, address).await?;
    reject_transfer_beyond_allowance(ctx, address).await
}

async fn deploy_allowance_token(ctx: &TreTestContext) -> Result<Address> {
    let deploy = wait_for_confirmed_transaction(
        ctx.genesis_provider
            .deploy(Bytes::from(hex::decode(LOCAL_TOKEN_ALLOWANCE_BYTECODE)?))
            .name("LocalTokenAllowance")
            .fee_limit(DEPLOY_FEE_LIMIT)
            .send()
            .await?,
    )
    .await?;
    deploy.contract_address.context("missing allowance token address")
}

async fn approve_secondary_account(ctx: &TreTestContext, address: Address) -> Result<()> {
    let token = ctx.genesis_provider.trc20(address).caller(ctx.genesis_account);
    assert_eq!(token.allowance(ctx.genesis_account, ctx.secondary_account).await?, U256::ZERO);
    let approved = U256::from(APPROVED_AMOUNT);
    let approval =
        wait_for_confirmed_transaction(token.approve(ctx.secondary_account, approved).await?)
            .await?;
    assert_eq!(token.allowance(ctx.genesis_account, ctx.secondary_account).await?, approved);
    let events = decode_logs::<ITRC20::Approval>(&approval.logs).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value, approved);
    Ok(())
}

async fn transfer_from_genesis_account(ctx: &TreTestContext, address: Address) -> Result<()> {
    let delegated_token = ctx.secondary_provider.trc20(address).caller(ctx.secondary_account);
    let amount = U256::from(DELEGATED_AMOUNT);
    let delegated = wait_for_confirmed_transaction(
        delegated_token.transfer_from(ctx.genesis_account, ctx.recipient_account, amount).await?,
    )
    .await?;

    let token = ctx.genesis_provider.trc20(address).caller(ctx.genesis_account);
    assert_eq!(
        token.allowance(ctx.genesis_account, ctx.secondary_account).await?,
        U256::from(APPROVED_AMOUNT - DELEGATED_AMOUNT)
    );
    assert_eq!(token.balance_of(ctx.recipient_account).await?, amount);
    let events = decode_logs::<ITRC20::Transfer>(&delegated.logs).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value, amount);
    Ok(())
}

async fn reject_transfer_beyond_allowance(ctx: &TreTestContext, address: Address) -> Result<()> {
    let remaining = U256::from(APPROVED_AMOUNT - DELEGATED_AMOUNT);
    let delegated_token = ctx.secondary_provider.trc20(address).caller(ctx.secondary_account);
    let over_allowance = delegated_token
        .transfer_from_call(ctx.genesis_account, ctx.recipient_account, remaining + U256::from(1))
        .fee_limit(REVERTING_CALL_FEE_LIMIT)
        .send()
        .await?;
    assert!(!wait_for_transaction_receipt(over_allowance).await?.is_success());

    let token = ctx.genesis_provider.trc20(address).caller(ctx.genesis_account);
    assert_eq!(token.allowance(ctx.genesis_account, ctx.secondary_account).await?, remaining);
    assert_eq!(token.balance_of(ctx.recipient_account).await?, U256::from(DELEGATED_AMOUNT));
    Ok(())
}
