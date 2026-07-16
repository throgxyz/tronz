use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use tracing_subscriber::fmt::MakeWriter;
use tronz_primitives::{Address, Trx, TxId};
use tronz_provider::{
    ProviderBuilder, TronProvider as _,
    transport::{
        TronTransport as _,
        grpc::{GrpcTransport, RetryConfig},
    },
    types::{SignedTransaction, TransferContract},
};
mod support;

use support::{ServerConfig, TestServer};
use tronz_signer::LocalSigner;

#[derive(Clone, Default)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().expect("captured log mutex poisoned").clone())
            .expect("tracing output is UTF-8")
    }
}

struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("captured log mutex poisoned").extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CapturedLogs {
    type Writer = CapturedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CapturedWriter(Arc::clone(&self.0))
    }
}

fn captured_tracing() -> (impl tracing::Subscriber + Send + Sync, CapturedLogs) {
    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(true)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    (subscriber, logs)
}

fn address(byte: u8) -> Address {
    Address::from_evm_bytes([byte; 20])
}

async fn transport(server: &TestServer, max_attempts: u32) -> GrpcTransport {
    GrpcTransport::builder()
        .with_retry(
            RetryConfig::default()
                .with_max_attempts(max_attempts)
                .with_initial_backoff(Duration::ZERO)
                .with_max_backoff(Duration::ZERO),
        )
        .connect(&server.endpoint)
        .await
        .expect("connect test transport")
}

async fn unsigned_transaction(transport: &GrpcTransport) -> SignedTransaction {
    let raw = transport
        .transfer_trx(TransferContract {
            owner_address: address(1),
            to_address: address(2),
            amount: Trx::from_sun_unchecked(1),
        })
        .await
        .expect("build test transaction");
    SignedTransaction { raw, signatures: Vec::new() }
}

#[tokio::test]
async fn retryable_rpc_preserves_behavior_when_tracing_is_disabled() {
    let _guard = tracing::subscriber::set_default(tracing::subscriber::NoSubscriber::default());
    let server =
        TestServer::start(ServerConfig { get_now_failures: 1, ..ServerConfig::default() }).await;
    let transport = transport(&server, 2).await;

    let block = transport.get_now_block().await.expect("retry succeeds");
    assert_eq!(block.number, 42);
    assert_eq!(server.get_now_calls(), 2);
}

#[tokio::test]
async fn retryable_rpc_records_canonical_fields_and_attempts() {
    let server =
        TestServer::start(ServerConfig { get_now_failures: 1, ..ServerConfig::default() }).await;
    let transport = transport(&server, 2).await;
    let (subscriber, logs) = captured_tracing();
    let _guard = tracing::subscriber::set_default(subscriber);

    let block = transport.get_now_block().await.expect("retry succeeds");
    assert_eq!(block.number, 42);
    assert_eq!(server.get_now_calls(), 2);

    let output = logs.contents();
    assert!(output.contains("tronz::rpc"), "{output}");
    assert!(output.contains("service=\"wallet\""), "{output}");
    assert!(output.contains("method=\"get_now_block2\""), "{output}");
    assert!(output.contains("retrying RPC request"), "{output}");
    assert!(output.contains("attempts=2"), "{output}");
    assert!(output.contains("outcome=\"ok\""), "{output}");
}

#[tokio::test]
async fn broadcast_rpc_is_observed_but_never_retried() {
    let server = TestServer::start(ServerConfig {
        broadcast_status: Some(tonic::Code::Unavailable),
        ..ServerConfig::default()
    })
    .await;
    let transport = transport(&server, 3).await;
    let transaction = unsigned_transaction(&transport).await;
    let (subscriber, logs) = captured_tracing();
    let _guard = tracing::subscriber::set_default(subscriber);

    let error = transport.broadcast_transaction(&transaction).await.unwrap_err();
    assert!(error.is_grpc());
    assert_eq!(server.broadcast_calls(), 1);

    let output = logs.contents();
    assert!(output.contains("method=\"broadcast_transaction\""), "{output}");
    assert!(output.contains("attempts=1"), "{output}");
    assert!(output.contains("grpc_code=\"unavailable\""), "{output}");
    assert!(!output.contains("retrying RPC request"), "{output}");
}

#[tokio::test]
async fn provider_records_broadcasted_stage() {
    let server = TestServer::start(ServerConfig::default()).await;
    let signer =
        LocalSigner::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
            .expect("valid test key");
    let provider = ProviderBuilder::new()
        .with_signer(signer)
        .on_grpc(&server.endpoint)
        .await
        .expect("connect test provider");
    let (subscriber, logs) = captured_tracing();
    let _guard = tracing::subscriber::set_default(subscriber);

    let pending = provider
        .send_trx()
        .to(address(2))
        .amount(Trx::from_sun_unchecked(1))
        .send()
        .await
        .expect("broadcast succeeds");
    assert_eq!(pending.tx_id(), TxId::from([9; 32]));

    let output = logs.contents();
    assert!(output.contains("operation=\"broadcast\""), "{output}");
    assert!(output.contains("stage=\"broadcasted\""), "{output}");
    assert!(output.contains("outcome=\"ok\""), "{output}");
}

#[tokio::test]
async fn provider_records_node_rejection_without_transaction_payload() {
    const SECRET: &str = "recognizable-signed-transaction-secret";

    let server =
        TestServer::start(ServerConfig { broadcast_result: false, ..ServerConfig::default() })
            .await;
    let signer =
        LocalSigner::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
            .expect("valid test key");
    let provider = ProviderBuilder::new()
        .with_signer(signer)
        .on_grpc(&server.endpoint)
        .await
        .expect("connect test provider");
    let (subscriber, logs) = captured_tracing();
    let _guard = tracing::subscriber::set_default(subscriber);

    let result = provider
        .send_trx()
        .to(address(2))
        .amount(Trx::from_sun_unchecked(1))
        .memo(SECRET.as_bytes())
        .send()
        .await;
    let error = match result {
        Ok(_) => panic!("node rejection should fail"),
        Err(error) => error,
    };
    assert!(error.is_node_error());

    let output = logs.contents();
    assert!(output.contains("operation=\"broadcast\""), "{output}");
    assert!(output.contains("outcome=\"node_error\""), "{output}");
    assert!(!output.contains("stage="), "{output}");
    assert!(!output.contains(SECRET), "{output}");
    assert!(!output.contains(&hex::encode(SECRET)), "{output}");
}
