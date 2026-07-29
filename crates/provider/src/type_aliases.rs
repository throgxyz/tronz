//! Short names for the provider types the builder actually produces.
//!
//! `FilledProvider` carries its whole filler chain in its type, which is
//! unwieldy to write out in a struct field or a function signature. These name
//! the stacks [`ProviderBuilder`](crate::ProviderBuilder) assembles, so a caller
//! rarely has to. When even the filler chain should not be in the type, reach
//! for [`DynProvider`](crate::DynProvider).

use tronz_signer::TronWallet;

use crate::{
    FilledProvider,
    fillers::{EnergyFiller, Identity, JoinFill, WalletFiller},
};

/// The filler chain installed by [`ProviderBuilder::new`](crate::ProviderBuilder::new).
pub type RecommendedFillers = JoinFill<Identity, EnergyFiller>;

/// [`RecommendedFillers`] plus the wallet added by
/// [`ProviderBuilder::wallet`](crate::ProviderBuilder::wallet).
pub type WalletFillers<W = TronWallet> = JoinFill<RecommendedFillers, WalletFiller<W>>;

/// A read-only provider: the recommended fillers, no wallet.
pub type ReadProvider = FilledProvider<RecommendedFillers>;

/// A signing provider: the recommended fillers plus a wallet.
pub type WalletProvider<W = TronWallet> = FilledProvider<WalletFillers<W>>;

#[cfg(test)]
mod tests {
    use tronz_primitives::B256;

    use super::*;
    use crate::{ProviderBuilder, TronProvider, transport::mock::MockTransport, types::BlockInfo};
    #[tokio::test]
    async fn a_provider_reads_through_any_transport_under_one_name() {
        let mock = MockTransport::new();
        mock.push_ok("get_now_block", BlockInfo::new(7, B256::ZERO, 1));

        let provider: ReadProvider = ProviderBuilder::new().connect_transport(mock);

        assert_eq!(provider.get_now_block().await.unwrap().number, 7);
    }
}
