//! gRPC integration tests for [`SolidityGrpcTransport`].

use tonic::Status;
use tronz_primitives::{Address, Bytes, Trx, TxId};
use tronz_provider::{
    SolidityTransport,
    transport::grpc::{RetryConfig, SolidityGrpcTransport},
    types::TriggerSmartContract,
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

    let by_num = transport.get_block_by_number(7).await.unwrap();
    assert_eq!(by_num.number, 7);
    assert_eq!(handle.seen_methods(), vec!["GetNowBlock2", "GetBlockByNum2"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn decodes_transaction_by_id() {
    let (addr, handle) = spawn().await;
    handle.push_transaction(Ok(pb::Transaction {
        raw_data: Some(pb::transaction::Raw {
            expiration: 2_000,
            timestamp: 1_000,
            ..Default::default()
        }),
        ..Default::default()
    }));

    let transport = connect(addr).await;
    let transaction = transport.get_transaction_by_id(TxId::from([3u8; 32])).await.unwrap();

    assert_eq!(transaction.raw.expiration, 2_000);
    assert_eq!(transaction.raw.timestamp, 1_000);
    assert!(transaction.signatures.is_empty());
    assert_eq!(handle.seen_methods(), vec!["GetTransactionById"]);
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
