use anyhow::{Context, Result};
use tronz::{
    Address, U256,
    contract::{ContractExt as _, Trc721Ext as _, event::decode_logs, trc721::ITRC721},
    primitives::{Bytes, Log},
};

use super::{
    fixtures::TreTestContext,
    support::{
        DEPLOY_FEE_LIMIT, REVERTING_CALL_FEE_LIMIT, wait_for_confirmed_transaction,
        wait_for_transaction_receipt,
    },
};

// Generated with solc 0.8.35 and the adjacent Foundry config:
// forge build --root crates/tronz/tests/contracts --use 0.8.35
// jq -r '.bytecode.object' crates/tronz/tests/contracts/out/LocalNft.sol/LocalNft.json
const LOCAL_NFT_BYTECODE: &str = "6080604052348015600e575f5ffd5b507fada5013122d395ba3c54772283fb069b10426056ef8ca54750cb9bb552a59e7d80546001600160a01b031916339081179091555f81815260016020819052604080832082905551909291907fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef908290a46109748061008d5f395ff3fe608060405234801561000f575f5ffd5b50600436106100b1575f3560e01c806370a082311161006e57806370a082311461016c57806395d89b411461018d578063a22cb465146101b0578063b88d4fde146101c3578063c87b56dd146101d6578063e985e9c5146101e9575f5ffd5b806306fdde03146100b5578063081812fc146100f3578063095ea7b31461011e57806323b872dd1461013357806342842e0e146101465780636352211e14610159575b5f5ffd5b6100dd60405180604001604052806009815260200168131bd8d85b0813919560ba1b81525081565b6040516100ea9190610706565b60405180910390f35b610106610101366004610751565b610234565b6040516001600160a01b0390911681526020016100ea565b61013161012c366004610783565b61025a565b005b6101316101413660046107ab565b61033c565b6101316101543660046107ab565b610540565b610106610167366004610751565b610550565b61017f61017a3660046107e5565b6105aa565b6040519081526020016100ea565b6100dd604051806040016040528060048152602001631313919560e21b81525081565b6101316101be366004610805565b610609565b6101316101d136600461083e565b610674565b6100dd6101e4366004610751565b610686565b6102246101f73660046108d3565b6001600160a01b039182165f90815260036020908152604080832093909416825291909152205460ff1690565b60405190151581526020016100ea565b5f61023e82610550565b50505f908152600260205260409020546001600160a01b031690565b5f61026482610550565b9050336001600160a01b038216148061029f57506001600160a01b0381165f90815260036020908152604080832033845290915290205460ff165b6102e15760405162461bcd60e51b815260206004820152600e60248201526d1b9bdd08185d5d1a1bdc9a5e995960921b60448201526064015b60405180910390fd5b5f8281526002602052604080822080546001600160a01b0319166001600160a01b0387811691821790925591518593918516917f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b92591a4505050565b5f61034682610550565b9050836001600160a01b0316816001600160a01b0316146103975760405162461bcd60e51b815260206004820152600b60248201526a3bb937b7339037bbb732b960a91b60448201526064016102d8565b6001600160a01b0383166103de5760405162461bcd60e51b815260206004820152600e60248201526d1e995c9bc81c9958da5c1a595b9d60921b60448201526064016102d8565b336001600160a01b038216148061040a57505f828152600260205260409020546001600160a01b031633145b8061043757506001600160a01b0381165f90815260036020908152604080832033845290915290205460ff165b6104745760405162461bcd60e51b815260206004820152600e60248201526d1b9bdd08185d5d1a1bdc9a5e995960921b60448201526064016102d8565b5f82815260026020908152604080832080546001600160a01b031990811690915583835281842080546001600160a01b03898116919093161790558716835260019182905282208054919290916104cc908490610918565b90915550506001600160a01b0383165f90815260016020819052604082208054919290916104fb90849061092b565b909155505060405182906001600160a01b0380861691908716907fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef905f90a450505050565b61054b83838361033c565b505050565b5f818152602081905260408120546001600160a01b0316806105a45760405162461bcd60e51b815260206004820152600d60248201526c36b4b9b9b4b733903a37b5b2b760991b60448201526064016102d8565b92915050565b5f6001600160a01b0382166105ee5760405162461bcd60e51b815260206004820152600a6024820152693d32b9379037bbb732b960b11b60448201526064016102d8565b506001600160a01b03165f9081526001602052604090205490565b335f8181526003602090815260408083206001600160a01b03871680855290835292819020805460ff191686151590811790915590519081529192917f17307eab39ab6107e8899845ad3d59bd9653f200f220920489ca2b5937696c31910160405180910390a35050565b61067f85858561033c565b5050505050565b5f818152602081905260409020546060906001600160a01b03166106dc5760405162461bcd60e51b815260206004820152600d60248201526c36b4b9b9b4b733903a37b5b2b760991b60448201526064016102d8565b505060408051808201909152600e81526d697066733a2f2f6c6f63616c2f3160901b602082015290565b602081525f82518060208401525f5b818110156107325760208186018101516040868401015201610715565b505f604082850101526040601f19601f83011684010191505092915050565b5f60208284031215610761575f5ffd5b5035919050565b80356001600160a01b038116811461077e575f5ffd5b919050565b5f5f60408385031215610794575f5ffd5b61079d83610768565b946020939093013593505050565b5f5f5f606084860312156107bd575f5ffd5b6107c684610768565b92506107d460208501610768565b929592945050506040919091013590565b5f602082840312156107f5575f5ffd5b6107fe82610768565b9392505050565b5f5f60408385031215610816575f5ffd5b61081f83610768565b915060208301358015158114610833575f5ffd5b809150509250929050565b5f5f5f5f5f60808688031215610852575f5ffd5b61085b86610768565b945061086960208701610768565b935060408601359250606086013567ffffffffffffffff81111561088b575f5ffd5b8601601f8101881361089b575f5ffd5b803567ffffffffffffffff8111156108b1575f5ffd5b8860208284010111156108c2575f5ffd5b959894975092955050506020019190565b5f5f604083850312156108e4575f5ffd5b6108ed83610768565b91506108fb60208401610768565b90509250929050565b634e487b7160e01b5f52601160045260245ffd5b818103818111156105a4576105a4610904565b808201808211156105a4576105a461090456fea26469706673582212204675f4c11befa9e530443c78ea76dfff434bf5b0d9018d42c08eddc595203d2764736f6c63430008230033";

const TOKEN_ID: u64 = 1;

pub(crate) async fn deploy_approve_and_transfer_trc721(ctx: &TreTestContext) -> Result<()> {
    let address = deploy_local_nft(ctx).await?;
    verify_nft_metadata(ctx, address).await?;
    reject_unauthorized_transfer(ctx, address).await?;
    approve_recipient_account(ctx, address).await?;
    approve_all_and_safe_transfer(ctx, address).await?;
    safe_transfer_back_with_data(ctx, address).await?;
    transfer_to_recipient_account(ctx, address).await
}

async fn deploy_local_nft(ctx: &TreTestContext) -> Result<Address> {
    let receipt = wait_for_confirmed_transaction(
        ctx.genesis_provider
            .deploy(Bytes::from(hex::decode(LOCAL_NFT_BYTECODE)?))
            .name("LocalNft")
            .fee_limit(DEPLOY_FEE_LIMIT)
            .send()
            .await?,
    )
    .await?;
    receipt.contract_address.context("missing deployed NFT address")
}

async fn verify_nft_metadata(ctx: &TreTestContext, address: Address) -> Result<()> {
    let nft = ctx.genesis_provider.trc721(address).caller(ctx.genesis_account);
    assert_eq!(nft.name().await?, "Local NFT");
    assert_eq!(nft.symbol().await?, "LNFT");
    assert_eq!(nft.token_uri(U256::from(TOKEN_ID)).await?, "ipfs://local/1");
    assert_eq!(nft.owner_of(U256::from(TOKEN_ID)).await?, ctx.genesis_account);
    assert_eq!(nft.balance_of(ctx.genesis_account).await?, U256::from(1u64));
    Ok(())
}

async fn reject_unauthorized_transfer(ctx: &TreTestContext, address: Address) -> Result<()> {
    let secondary_nft = ctx.secondary_provider.trc721(address).caller(ctx.secondary_account);
    let unauthorized = secondary_nft
        .transfer_from_call(ctx.genesis_account, ctx.secondary_account, U256::from(TOKEN_ID))
        .fee_limit(REVERTING_CALL_FEE_LIMIT)
        .send()
        .await?;
    assert!(!wait_for_transaction_receipt(unauthorized).await?.is_success());

    let nft = ctx.genesis_provider.trc721(address).caller(ctx.genesis_account);
    assert_eq!(nft.owner_of(U256::from(TOKEN_ID)).await?, ctx.genesis_account);
    Ok(())
}

async fn approve_recipient_account(ctx: &TreTestContext, address: Address) -> Result<()> {
    let nft = ctx.genesis_provider.trc721(address).caller(ctx.genesis_account);
    wait_for_confirmed_transaction(nft.approve(ctx.recipient_account, U256::from(TOKEN_ID)).await?)
        .await?;
    assert_eq!(nft.get_approved(U256::from(TOKEN_ID)).await?, ctx.recipient_account);
    Ok(())
}

async fn approve_all_and_safe_transfer(ctx: &TreTestContext, address: Address) -> Result<()> {
    let nft = ctx.genesis_provider.trc721(address).caller(ctx.genesis_account);
    wait_for_confirmed_transaction(nft.set_approval_for_all(ctx.secondary_account, true).await?)
        .await?;
    assert!(nft.is_approved_for_all(ctx.genesis_account, ctx.secondary_account).await?);

    let secondary_nft = ctx.secondary_provider.trc721(address).caller(ctx.secondary_account);
    let safe_transfer = wait_for_confirmed_transaction(
        secondary_nft
            .safe_transfer_from(ctx.genesis_account, ctx.secondary_account, U256::from(TOKEN_ID))
            .await?,
    )
    .await?;
    assert_eq!(nft.owner_of(U256::from(TOKEN_ID)).await?, ctx.secondary_account);
    assert_transfer_event(&safe_transfer.logs)
}

async fn safe_transfer_back_with_data(ctx: &TreTestContext, address: Address) -> Result<()> {
    let secondary_nft = ctx.secondary_provider.trc721(address).caller(ctx.secondary_account);
    wait_for_confirmed_transaction(
        secondary_nft
            .safe_transfer_from_with_data(
                ctx.secondary_account,
                ctx.genesis_account,
                U256::from(TOKEN_ID),
                Bytes::from_static(b"e2e"),
            )
            .await?,
    )
    .await?;
    let nft = ctx.genesis_provider.trc721(address).caller(ctx.genesis_account);
    assert_eq!(nft.owner_of(U256::from(TOKEN_ID)).await?, ctx.genesis_account);
    Ok(())
}

async fn transfer_to_recipient_account(ctx: &TreTestContext, address: Address) -> Result<()> {
    let nft = ctx.genesis_provider.trc721(address).caller(ctx.genesis_account);
    let transfer = wait_for_confirmed_transaction(
        nft.transfer_from(ctx.genesis_account, ctx.recipient_account, U256::from(TOKEN_ID)).await?,
    )
    .await?;
    assert_eq!(nft.owner_of(U256::from(TOKEN_ID)).await?, ctx.recipient_account);
    assert_eq!(nft.balance_of(ctx.genesis_account).await?, U256::ZERO);
    assert_eq!(nft.balance_of(ctx.recipient_account).await?, U256::from(1u64));
    assert_transfer_event(&transfer.logs)
}

fn assert_transfer_event(logs: &[Log]) -> Result<()> {
    let events = decode_logs::<ITRC721::Transfer>(logs).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tokenId, U256::from(TOKEN_ID));
    Ok(())
}
