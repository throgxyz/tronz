//! [`ProviderLayer`], for wrapping a provider in behaviour of your own.

use crate::provider::TronProvider;

/// Wraps a provider in another provider.
///
/// [`TronProvider`] is implementable downstream. A wrapper supplies
/// [`root`](TronProvider::root), [`inner`](TronProvider::inner), and
/// [`inner_read`](crate::ContractReadProvider::inner_read) — three lines — and
/// overrides just the methods it cares about. Everything it leaves alone keeps
/// travelling down the stack, so in `Outer(Inner(provider))` a call `Outer` says
/// nothing about still reaches `Inner`'s version of it.
///
/// # What a layer does not see
///
/// The operation builders ([`send_trx`](TronProvider::send_trx) and friends) cannot
/// be overridden usefully — they name `Self` in their return type — but they route
/// back through [`send_transaction`](TronProvider::send_transaction) and
/// [`build_transaction`](TronProvider::build_transaction), which can.
///
/// Polling is the real gap: a
/// [`PendingTransaction`](crate::provider::PendingTransaction) holds the root
/// provider, so waiting on a receipt does not run the layers the transaction was
/// sent through. (Alloy's `PendingTransactionBuilder` holds a `RootProvider` for the
/// same reason.)
///
/// A layer is also the wrong home for behaviour that has to see *every* RPC —
/// metrics, rate limiting, request logging — because it would have to name all of
/// them. Use a [`GrpcMiddleware`](crate::transport::grpc::GrpcMiddleware) for that,
/// which sits below all of this where every call funnels through one place.
///
/// A layer is applied to a built provider:
///
/// ```no_run
/// # use tronz_provider::{ProviderBuilder, ProviderLayer, TronProvider};
/// # use tronz_provider::layers::LoggingLayer;
/// # async fn run() -> tronz_provider::Result<()> {
/// let provider =
///     ProviderBuilder::new().layer(LoggingLayer).connect("grpc.trongrid.io:50051").await?;
///
/// let _ = provider.get_now_block().await?;
/// # Ok(()) }
/// ```
///
/// [`ProviderBuilder::layer`](crate::ProviderBuilder::layer) puts layers between the
/// root and the fillers, so a layer sees transactions with their fields already
/// filled. Applying one by hand afterwards puts it outside the fillers instead,
/// where it sees the request as the caller wrote it.
///
/// Use [`Stack`] to compose two layers into one.
pub trait ProviderLayer<P: TronProvider> {
    /// The provider this layer produces.
    type Provider: TronProvider;

    /// Wrap `inner`.
    fn layer(&self, inner: P) -> Self::Provider;
}

impl<P: TronProvider> ProviderLayer<P> for crate::fillers::Identity {
    type Provider = P;

    fn layer(&self, inner: P) -> P {
        inner
    }
}

/// Two layers as one, applying `inner` first.
#[derive(Clone, Copy, Debug)]
pub struct Stack<Inner, Outer> {
    inner: Inner,
    outer: Outer,
}

impl<Inner, Outer> Stack<Inner, Outer> {
    /// Compose two layers.
    pub const fn new(inner: Inner, outer: Outer) -> Self {
        Self { inner, outer }
    }
}

impl<P, Inner, Outer> ProviderLayer<P> for Stack<Inner, Outer>
where
    P: TronProvider,
    Inner: ProviderLayer<P>,
    Outer: ProviderLayer<Inner::Provider>,
{
    type Provider = Outer::Provider;

    fn layer(&self, inner: P) -> Self::Provider {
        self.outer.layer(self.inner.layer(inner))
    }
}

pub use logging::{Logging, LoggingLayer};

mod logging {
    use async_trait::async_trait;

    use super::{ProviderLayer, TronProvider};
    use crate::{
        error::Result,
        provider::{PendingTransaction, RootProvider},
        types::{BlockInfo, SignedTransaction, TransactionRequest},
    };

    /// Traces every send through the provider it wraps.
    ///
    /// Also the worked example for [`ProviderLayer`]: it names the methods it cares
    /// about, plus the three that say what it wraps, and that is the whole layer.
    ///
    /// It traces both [`send_transaction`](TronProvider::send_transaction) and
    /// [`broadcast`](TronProvider::broadcast) because a send arrives at one or the
    /// other depending on where the layer sits. Installed through
    /// [`ProviderBuilder::layer`](crate::ProviderBuilder::layer) it sits under the
    /// fillers, which take `send_transaction` themselves and hand down the build and
    /// the broadcast; applied by hand to a built provider it sits above them and
    /// takes `send_transaction` first. Either way a send is traced once.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct LoggingLayer;

    impl<P: TronProvider> ProviderLayer<P> for LoggingLayer {
        type Provider = Logging<P>;

        fn layer(&self, inner: P) -> Logging<P> {
            Logging { inner }
        }
    }

    /// The provider [`LoggingLayer`] produces.
    #[derive(Clone, Debug)]
    pub struct Logging<P> {
        inner: P,
    }

    impl<P: TronProvider> crate::provider::ContractReadProvider for Logging<P> {
        fn inner_read(&self) -> Option<&dyn crate::provider::ContractReadProvider> {
            Some(&self.inner)
        }
    }

    #[async_trait]
    impl<P: TronProvider> TronProvider for Logging<P> {
        fn root(&self) -> &RootProvider {
            self.inner.root()
        }

        fn inner(&self) -> Option<&dyn TronProvider> {
            Some(&self.inner)
        }

        async fn get_now_block(&self) -> Result<BlockInfo> {
            let block = self.inner.get_now_block().await?;
            tracing::debug!(number = block.number, "read latest block");
            Ok(block)
        }

        async fn send_transaction(&self, req: TransactionRequest) -> Result<PendingTransaction> {
            tracing::debug!("sending transaction");
            self.inner.send_transaction(req).await
        }

        async fn broadcast(&self, tx: SignedTransaction) -> Result<PendingTransaction> {
            tracing::debug!(tx_id = %tx.raw.tx_id(), "broadcasting transaction");
            self.inner.broadcast(tx).await
        }
    }
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    use std::sync::{Arc, Mutex};

    use prost::Message as _;
    use tronz_primitives::{Address, B256, Trx};
    use tronz_signer::{LocalSigner, TronWallet};

    use super::*;
    use crate::{
        ProviderBuilder,
        error::Result,
        transport::mock::MockTransport,
        types::{BlockInfo, RawTransaction},
    };
    #[derive(Clone, Default)]
    struct Messages(Arc<Mutex<Vec<String>>>);

    impl tracing::field::Visit for Messages {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.0.lock().unwrap().push(format!("{value:?}"));
            }
        }
    }

    impl tracing::Subscriber for Messages {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::Id {
            tracing::Id::from_u64(1)
        }

        fn record(&self, _: &tracing::Id, _: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _: &tracing::Id, _: &tracing::Id) {}

        fn event(&self, event: &tracing::Event<'_>) {
            event.record(&mut self.clone());
        }

        fn enter(&self, _: &tracing::Id) {}

        fn exit(&self, _: &tracing::Id) {}
    }

    const KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    fn sending_mock() -> MockTransport {
        let tx = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw {
                contract: vec![crate::proto::transaction::Contract::default()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let raw = RawTransaction::from_node_encoded(tx.encode_to_vec(), &[]).unwrap();

        let mock = MockTransport::new();
        mock.push_ok("transfer_trx", raw);
        mock.push_ok("broadcast_transaction", ());
        mock
    }
    async fn trace_a_send<P: TronProvider>(provider: P, owner: Address) -> Vec<String> {
        let messages = Messages::default();
        let recorded = messages.0.clone();
        let _guard = tracing::subscriber::set_default(messages);

        provider
            .send_trx()
            .from(owner)
            .to(Address::from_evm_bytes([9; 20]))
            .amount(Trx::from_sun_unchecked(1))
            .send()
            .await
            .unwrap();

        recorded.lock().unwrap().clone()
    }
    #[tokio::test]
    async fn the_logging_layer_traces_a_send_it_was_given_by_the_builder() {
        let signer = LocalSigner::from_hex(KEY).unwrap();
        let owner = signer.address();
        let provider = ProviderBuilder::default()
            .layer(LoggingLayer)
            .wallet(TronWallet::new(signer))
            .connect_transport(sending_mock());

        assert_eq!(trace_a_send(provider, owner).await, vec!["broadcasting transaction"]);
    }
    #[tokio::test]
    async fn a_hand_applied_layer_keeps_the_signer_visible_to_contract_calls() {
        use crate::provider::ContractReadProvider;

        let signer = LocalSigner::from_hex(KEY).unwrap();
        let owner = signer.address();
        let provider = LoggingLayer.layer(
            ProviderBuilder::default()
                .wallet(TronWallet::new(signer))
                .connect_transport(MockTransport::new()),
        );

        assert_eq!(provider.default_caller(), Some(owner));
    }
    #[tokio::test]
    async fn the_logging_layer_traces_a_send_when_wrapped_round_a_built_provider() {
        let signer = LocalSigner::from_hex(KEY).unwrap();
        let owner = signer.address();
        let provider = LoggingLayer.layer(
            ProviderBuilder::default()
                .wallet(TronWallet::new(signer))
                .connect_transport(sending_mock()),
        );

        assert_eq!(trace_a_send(provider, owner).await, vec!["sending transaction"]);
    }
    #[tokio::test]
    async fn a_layer_forwards_what_it_does_not_override() {
        let mock = MockTransport::new();
        mock.push_ok("get_now_block", BlockInfo::new(9, B256::ZERO, 1));
        mock.push_ok("get_next_maintenance_time", 42i64);

        let provider = LoggingLayer.layer(ProviderBuilder::new().connect_transport(mock));

        assert_eq!(provider.get_now_block().await.unwrap().number, 9);
        assert_eq!(provider.get_next_maintenance_time().await.unwrap(), 42);
    }
    #[tokio::test]
    async fn an_outer_layer_does_not_step_over_an_inner_one() {
        struct Answering;
        struct Answers<P>(P);

        impl<P: TronProvider> ProviderLayer<P> for Answering {
            type Provider = Answers<P>;

            fn layer(&self, inner: P) -> Answers<P> {
                Answers(inner)
            }
        }

        impl<P: TronProvider> crate::provider::ContractReadProvider for Answers<P> {
            fn inner_read(&self) -> Option<&dyn crate::provider::ContractReadProvider> {
                Some(&self.0)
            }
        }

        #[async_trait::async_trait]
        impl<P: TronProvider> TronProvider for Answers<P> {
            fn root(&self) -> &crate::provider::RootProvider {
                self.0.root()
            }

            fn inner(&self) -> Option<&dyn TronProvider> {
                Some(&self.0)
            }

            async fn get_next_maintenance_time(&self) -> Result<i64> {
                Ok(7)
            }
        }
        let provider = ProviderBuilder::default()
            .layer(LoggingLayer)
            .layer(Answering)
            .connect_transport(MockTransport::new());

        assert_eq!(provider.get_next_maintenance_time().await.unwrap(), 7);
    }
    #[tokio::test]
    async fn stacked_layers_compose() {
        let mock = MockTransport::new();
        mock.push_ok("get_now_block", BlockInfo::new(3, B256::ZERO, 1));

        let stack = Stack::new(LoggingLayer, LoggingLayer);
        let provider = stack.layer(ProviderBuilder::new().connect_transport(mock));

        assert_eq!(provider.get_now_block().await.unwrap().number, 3);
    }
}
