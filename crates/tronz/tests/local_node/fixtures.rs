use anyhow::{Result, ensure};
use tronz::{Address, LocalSigner, ProviderBuilder, SolidityProvider, providers::WalletProvider};

use super::support::{full_node_grpc_endpoint, solidity_node_grpc_endpoint};

const TRE_GENESIS_PRIVATE_KEY: &str =
    "0000000000000000000000000000000000000000000000000000000000000001";
const SECONDARY_ACCOUNT_PRIVATE_KEY: &str =
    "000000000000000000000000000000000000000000000000000000000000002a";
pub(crate) const TRE_GENESIS_ADDRESS: &str = "TMVQGm1qAQYVdetCeGRRkTWYYrLXuHK2HC";
pub(crate) const RECIPIENT_ACCOUNT_ADDRESS: &str = "TVdyt1s88BdiCjKt6K2YuoSmpWScZYK1QF";

pub(crate) struct TreTestContext {
    pub(crate) genesis_provider: WalletProvider,
    pub(crate) secondary_provider: WalletProvider,
    pub(crate) solidity_provider: SolidityProvider,
    pub(crate) genesis_account: Address,
    pub(crate) secondary_account: Address,
    pub(crate) recipient_account: Address,
    pub(crate) genesis_signer: LocalSigner,
    pub(crate) secondary_signer: LocalSigner,
}

impl TreTestContext {
    pub(crate) async fn set_up() -> Result<Self> {
        let genesis_signer = LocalSigner::from_hex(TRE_GENESIS_PRIVATE_KEY)?;
        let genesis_account: Address = TRE_GENESIS_ADDRESS.parse()?;
        ensure!(genesis_signer.address() == genesis_account, "TRE genesis fixture drifted");
        let genesis_provider = ProviderBuilder::new()
            .with_signer(genesis_signer.clone())
            .connect_grpc(&full_node_grpc_endpoint())
            .await?;

        let secondary_signer = LocalSigner::from_hex(SECONDARY_ACCOUNT_PRIVATE_KEY)?;
        let secondary_account = secondary_signer.address();
        let secondary_provider = ProviderBuilder::new()
            .with_signer(secondary_signer.clone())
            .connect_grpc(&full_node_grpc_endpoint())
            .await?;

        Ok(Self {
            genesis_provider,
            secondary_provider,
            solidity_provider: SolidityProvider::connect(&solidity_node_grpc_endpoint()).await?,
            genesis_account,
            secondary_account,
            recipient_account: RECIPIENT_ACCOUNT_ADDRESS.parse()?,
            genesis_signer,
            secondary_signer,
        })
    }
}
