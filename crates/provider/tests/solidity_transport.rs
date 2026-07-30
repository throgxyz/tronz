//! gRPC integration tests for [`SolidityGrpcTransport`].

use prost::Message as _;
use tonic::Status;
use tronz_primitives::{Address, Bytes, Trx, TxId};
use tronz_provider::{
    SolidityTransport,
    transport::grpc::{RetryConfig, SolidityGrpcTransport},
    types::{RawTransaction, TriggerSmartContract},
};
use tronz_provider_test_support::{Handle, pb, spawn};

async fn connect(addr: std::net::SocketAddr) -> SolidityGrpcTransport {
    SolidityGrpcTransport::connect(format!("http://{addr}")).await.expect("connect")
}

fn trigger() -> TriggerSmartContract {
    TriggerSmartContract {
        owner_address: Address::from_evm_bytes([1u8; 20]),
        contract_address: Address::from_evm_bytes([2u8; 20]),
        call_value: Trx::ZERO,
        data: Bytes::new(),
        call_token_value: Trx::ZERO,
        token_id: 0,
    }
}

fn block(number: i64, timestamp: i64) -> pb::BlockExtention {
    pb::BlockExtention {
        block_header: Some(pb::BlockHeader {
            raw_data: Some(pb::block_header::Raw { number, timestamp, ..Default::default() }),
            ..Default::default()
        }),
        blockid: vec![7u8; 32],
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn decodes_account_over_real_grpc() {
    let (addr, handle): (_, Handle) = spawn().await;
    handle.push_account(Ok(pb::Account {
        address: vec![0x41; 21],
        balance: 1_234_000,
        account_name: b"alice".to_vec(),
        ..Default::default()
    }));

    let transport = connect(addr).await;
    let account = transport.get_account(Address::from_evm_bytes([9u8; 20])).await.unwrap();

    assert_eq!(account.balance, Trx::from_sun(1_234_000).unwrap());
    assert_eq!(account.name, "alice");
    assert_eq!(handle.seen_methods(), vec!["GetAccount"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn decodes_blocks() {
    let (addr, handle) = spawn().await;
    handle.push_now_block(Ok(block(42, 1_234)));
    handle.push_block_by_num(Ok(block(7, 99)));

    let transport = connect(addr).await;

    let now = transport.get_now_block().await.unwrap();
    assert_eq!(now.number, 42);
    assert_eq!(now.timestamp, 1_234);

    let by_num = transport.get_block_by_number(7).await.unwrap().expect("block 7 exists");
    assert_eq!(by_num.number, 7);
    assert_eq!(handle.seen_methods(), vec!["GetNowBlock2", "GetBlockByNum2"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn decodes_transaction_by_id() {
    let (addr, handle) = spawn().await;
    let response = pb::Transaction {
        raw_data: Some(pb::transaction::Raw {
            expiration: 2_000,
            timestamp: 1_000,
            ..Default::default()
        }),
        ..Default::default()
    };
    let requested =
        RawTransaction::from_node_encoded(response.encode_to_vec(), &[]).unwrap().tx_id();
    handle.push_transaction(Ok(response));

    let transport = connect(addr).await;
    let transaction =
        transport.get_transaction_by_id(requested).await.unwrap().expect("the node knows this one");

    assert_eq!(transaction.raw.expiration, 2_000);
    assert_eq!(transaction.raw.timestamp, 1_000);
    assert!(transaction.signatures.is_empty());
    assert_eq!(handle.seen_methods(), vec!["GetTransactionById"]);
}

/// TRON answers an id it does not know with an empty message, which is an absence
/// rather than a node misbehaving.
#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_transaction_id_is_absent_rather_than_an_error() {
    let (addr, handle) = spawn().await;
    handle.push_transaction(Ok(pb::Transaction::default()));

    let transport = connect(addr).await;
    let found = transport.get_transaction_by_id(TxId::from([9u8; 32])).await.unwrap();

    assert!(found.is_none());
}

/// Likewise a height the chain has not reached.
#[tokio::test(flavor = "multi_thread")]
async fn a_block_the_chain_has_not_reached_is_absent_rather_than_an_error() {
    let (addr, handle) = spawn().await;
    handle.push_block_by_num(Ok(pb::BlockExtention::default()));

    let transport = connect(addr).await;
    let found = transport.get_block_by_number(9_999_999).await.unwrap();

    assert!(found.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_receipt_id_decodes_to_none() {
    let (addr, handle) = spawn().await;
    handle.push_transaction_info(Ok(pb::TransactionInfo::default()));

    let transport = connect(addr).await;
    let info = transport.get_transaction_info(TxId::from([5u8; 32])).await.unwrap();
    assert!(info.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_receipt_reports_failure() {
    let (addr, handle) = spawn().await;
    handle.push_transaction_info(Ok(pb::TransactionInfo {
        id: vec![9u8; 32],
        result: 0,
        receipt: Some(pb::ResourceReceipt { result: 2, ..Default::default() }),
        ..Default::default()
    }));

    let transport = connect(addr).await;
    let info = transport.get_transaction_info(TxId::from([9u8; 32])).await.unwrap().unwrap();
    assert!(!info.is_success());
}

#[tokio::test(flavor = "multi_thread")]
async fn decodes_receipt_list_for_block() {
    let (addr, handle) = spawn().await;
    handle.push_transaction_info_by_block(Ok(pb::TransactionInfoList {
        transaction_info: vec![
            pb::TransactionInfo { id: vec![1u8; 32], ..Default::default() },
            pb::TransactionInfo { id: vec![2u8; 32], ..Default::default() },
        ],
    }));

    let transport = connect(addr).await;
    let list = transport.get_transaction_info_by_block_num(100).await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn decodes_count_constant_call_and_estimate() {
    let (addr, handle) = spawn().await;
    handle.push_transaction_count(Ok(pb::NumberMessage { num: 5 }));
    handle.push_constant(Ok(pb::TransactionExtention {
        constant_result: vec![vec![0xde, 0xad, 0xbe, 0xef]],
        result: Some(pb::Return { result: true, ..Default::default() }),
        ..Default::default()
    }));
    handle.push_estimate(Ok(pb::EstimateEnergyMessage {
        result: Some(pb::Return { result: true, ..Default::default() }),
        energy_required: 31_000,
    }));

    let transport = connect(addr).await;

    assert_eq!(transport.get_transaction_count_by_block_num(100).await.unwrap(), 5);

    let call = transport.trigger_constant_contract(trigger()).await.unwrap();
    assert_eq!(call.output.as_ref(), &[0xde, 0xad, 0xbe, 0xef]);

    assert_eq!(transport.estimate_energy(trigger()).await.unwrap(), 31_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn injects_api_key_header() {
    let (addr, handle) = spawn().await;
    handle.push_now_block(Ok(block(1, 1)));

    let transport = SolidityGrpcTransport::builder()
        .maybe_api_key(Some("key-123"))
        .connect(format!("http://{addr}"))
        .await
        .unwrap();
    transport.get_now_block().await.unwrap();

    assert_eq!(handle.seen_api_keys(), vec![Some("key-123".to_owned())]);
}

#[tokio::test(flavor = "multi_thread")]
async fn retries_retryable_status_then_succeeds() {
    let (addr, handle) = spawn().await;
    handle.push_account(Err(Status::unavailable("try again")));
    handle.push_account(Ok(pb::Account {
        address: vec![0x41; 21],
        balance: 7,
        ..Default::default()
    }));

    let transport = SolidityGrpcTransport::builder()
        .with_retry(RetryConfig::default())
        .connect(format!("http://{addr}"))
        .await
        .unwrap();
    let account = transport.get_account(Address::from_evm_bytes([9u8; 20])).await.unwrap();

    assert_eq!(account.balance, Trx::from_sun(7).unwrap());
    assert_eq!(handle.seen_methods(), vec!["GetAccount", "GetAccount"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn does_not_retry_non_retryable_status() {
    let (addr, handle) = spawn().await;
    handle.push_account(Err(Status::invalid_argument("bad address")));

    let transport = connect(addr).await;
    let err = transport.get_account(Address::from_evm_bytes([9u8; 20])).await.unwrap_err();

    assert!(err.to_string().contains("bad address"), "unexpected error: {err}");
    assert_eq!(handle.seen_methods(), vec!["GetAccount"]);
}

// ── middleware ────────────────────────────────────────────────────────────────

/// Records what passed through it, and can refuse a call before it is made.
#[derive(Default)]
struct Recorder {
    seen: std::sync::Mutex<Vec<(String, u32)>>,
    refuse: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl tronz_provider::transport::grpc::GrpcMiddleware for Recorder {
    async fn before(
        &self,
        call: tronz_provider::transport::grpc::GrpcCall<'_>,
    ) -> Result<(), tronz_provider::TransportErrorKind> {
        if self.refuse.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(tronz_provider::TransportErrorKind::Malformed(format!(
                "{} not allowed",
                call.method
            )));
        }
        Ok(())
    }

    async fn after(
        &self,
        call: tronz_provider::transport::grpc::GrpcCall<'_>,
        outcome: tronz_provider::transport::grpc::GrpcOutcome<'_>,
    ) {
        self.seen.lock().unwrap().push((call.method.to_owned(), outcome.attempts));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn middleware_sees_calls_on_both_paths() {
    let (addr, handle) = spawn().await;
    handle.push_now_block(Ok(block(5, 50)));
    handle.push_account(Ok(pb::Account { balance: 3, ..Default::default() }));

    let recorder = std::sync::Arc::new(Recorder::default());
    let transport = SolidityGrpcTransport::builder()
        .with_middleware(recorder.clone())
        .connect(format!("http://{addr}"))
        .await
        .unwrap();

    // One goes through the hand-rolled `unary`, the other through the generated
    // client — middleware has to catch both.
    transport.get_now_block().await.unwrap();
    transport.get_account(Address::from_evm_bytes([9u8; 20])).await.unwrap();

    let seen = recorder.seen.lock().unwrap().clone();
    assert_eq!(seen, vec![("get_now_block".to_owned(), 1), ("get_account".to_owned(), 1)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn middleware_can_refuse_a_call_before_it_is_made() {
    let (addr, handle) = spawn().await;

    let recorder = std::sync::Arc::new(Recorder::default());
    recorder.refuse.store(true, std::sync::atomic::Ordering::Relaxed);

    let transport = SolidityGrpcTransport::builder()
        .with_middleware(recorder.clone())
        .connect(format!("http://{addr}"))
        .await
        .unwrap();

    let err = transport.get_account(Address::from_evm_bytes([9u8; 20])).await.unwrap_err();

    assert!(err.to_string().contains("get_account not allowed"));
    assert!(handle.seen_methods().is_empty());

    // The one that refused is not owed an `after`, having never entered.
    assert!(recorder.seen.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_still_unwinds_the_middleware_that_ran() {
    let (addr, handle) = spawn().await;

    // The outer one lets the call through, the inner one refuses it.
    let outer = std::sync::Arc::new(Recorder::default());
    let inner = std::sync::Arc::new(Recorder::default());
    inner.refuse.store(true, std::sync::atomic::Ordering::Relaxed);

    let transport = SolidityGrpcTransport::builder()
        .with_middleware(outer.clone())
        .with_middleware(inner.clone())
        .connect(format!("http://{addr}"))
        .await
        .unwrap();

    transport.get_account(Address::from_evm_bytes([9u8; 20])).await.unwrap_err();

    assert!(handle.seen_methods().is_empty());
    // The one that ran sees the outcome; the one that refused does not.
    assert_eq!(*outer.seen.lock().unwrap(), vec![("get_account".to_owned(), 0)]);
    assert!(inner.seen.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn middleware_reports_one_call_and_counts_its_attempts() {
    let (addr, handle) = spawn().await;
    handle.push_account(Err(Status::unavailable("try again")));
    handle.push_account(Ok(pb::Account { balance: 7, ..Default::default() }));

    let recorder = std::sync::Arc::new(Recorder::default());
    let transport = SolidityGrpcTransport::builder()
        .with_middleware(recorder.clone())
        .connect(format!("http://{addr}"))
        .await
        .unwrap();

    transport.get_account(Address::from_evm_bytes([9u8; 20])).await.unwrap();

    // Two attempts reached the node; middleware saw one logical call that took two.
    assert_eq!(handle.seen_methods(), vec!["GetAccount", "GetAccount"]);
    assert_eq!(*recorder.seen.lock().unwrap(), vec![("get_account".to_owned(), 2)]);
}
