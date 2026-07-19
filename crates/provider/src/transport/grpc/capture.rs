//! Manual, live-node fixture capture for the offline replay tests.
//!
//! The replay tests in [`super::codec`] and [`super::light_block`] decode real
//! protobuf bytes stored under `fixtures/`. This module regenerates those
//! `.bin` files by talking to a live node. It is `#[ignore]`d and env-gated, so
//! it never runs in normal CI — run it by hand, then commit the fixtures:
//!
//! ```bash
//! TRONZ_CAPTURE_URL=http://<host>:50051 \
//!   TRONZ_TX_SUCCESS=<txid_hex> \
//!   TRONZ_TX_REVERTED=<txid_hex> \
//!   cargo test -p tronz-provider --all-features \
//!     -- --ignored transport::grpc::capture --nocapture
//! git add crates/provider/src/transport/grpc/fixtures/*.bin
//! ```
//!
//! Env vars:
//! - `TRONZ_CAPTURE_URL` (required): gRPC endpoint. Plain `http://host:50051` works without TLS; `https://…`
//!   requires the `grpc-tls` feature.
//! - `TRONZ_CAPTURE_API_KEY` (optional): injected as the `tron-pro-api-key` header (TronGrid).
//! - `TRONZ_ACCT_ACTIVATED` (optional, base58): defaults to the mainnet USDT contract.
//! - `TRONZ_CONST_HOLDER` (optional, base58): `balanceOf` argument for the constant-call fixture;
//!   defaults to the USDT contract.
//! - `TRONZ_TX_SUCCESS` / `TRONZ_TX_REVERTED` (optional, hex txid): captured only when set (a
//!   reverted txid in particular must be supplied manually).

use std::{fs, path::PathBuf};

use prost::Message as _;
use tonic::{
    Request,
    metadata::MetadataValue,
    transport::{Channel, Endpoint},
};
use tronz_primitives::Address;

use crate::proto::{self, wallet_client::WalletClient};

/// Mainnet USDT (TRC20) contract address.
const USDT_MAINNET: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/transport/grpc/fixtures")
}

fn write_fixture(name: &str, bytes: &[u8]) {
    let dir = fixtures_dir();
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    eprintln!("wrote {} ({} bytes)", path.display(), bytes.len());
}

/// Minimal hex decoder (avoids pulling a `hex` dependency for a manual tool).
fn hex_decode(s: &str) -> Vec<u8> {
    let s = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("invalid hex txid"))
        .collect()
}

async fn connect(url: &str) -> WalletClient<Channel> {
    let endpoint = Endpoint::from_shared(url.to_owned()).unwrap();

    #[cfg(feature = "grpc-tls")]
    let endpoint = if url.starts_with("https") {
        endpoint.tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots()).unwrap()
    } else {
        endpoint
    };

    WalletClient::new(endpoint.connect().await.unwrap())
}

/// Wrap a message in a request, injecting the optional API-key header.
fn req<T>(msg: T, api_key: &Option<String>) -> Request<T> {
    let mut request = Request::new(msg);
    if let Some(key) = api_key
        && let Ok(value) = key.parse::<MetadataValue<_>>()
    {
        request.metadata_mut().insert("tron-pro-api-key", value);
    }
    request
}

#[tokio::test]
#[ignore = "captures fixtures from a live node; run manually"]
async fn capture_fixtures() {
    let Ok(url) = std::env::var("TRONZ_CAPTURE_URL") else {
        eprintln!(
            "TRONZ_CAPTURE_URL unset; skipping capture. \
             Set it to a gRPC endpoint (e.g. http://<host>:50051) to regenerate fixtures."
        );
        return;
    };
    let api_key = std::env::var("TRONZ_CAPTURE_API_KEY").ok();
    let mut client = connect(&url).await;

    // Latest block (full BlockExtention; the replay decodes it via the light view).
    let block = client
        .get_now_block2(req(proto::EmptyMessage::default(), &api_key))
        .await
        .unwrap()
        .into_inner();
    write_fixture("now_block.bin", &block.encode_to_vec());

    // Activated account (USDT contract by default).
    let activated =
        std::env::var("TRONZ_ACCT_ACTIVATED").unwrap_or_else(|_| USDT_MAINNET.to_string());
    let addr = Address::from_base58(&activated).unwrap();
    let account = client
        .get_account(req(
            proto::Account { address: addr.as_bytes().to_vec(), ..Default::default() },
            &api_key,
        ))
        .await
        .unwrap()
        .into_inner();
    write_fixture("account_activated.bin", &account.encode_to_vec());

    // Never-activated account: a fixed, almost-certainly-unused address.
    let fresh = Address::from_evm_bytes([
        0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa,
        0xbb, 0xcc, 0xdd, 0xee, 0xff,
    ]);
    let empty = client
        .get_account(req(
            proto::Account { address: fresh.as_bytes().to_vec(), ..Default::default() },
            &api_key,
        ))
        .await
        .unwrap()
        .into_inner();
    write_fixture("account_never_activated.bin", &empty.encode_to_vec());

    // Constant call: balanceOf(holder) on USDT.
    let holder = std::env::var("TRONZ_CONST_HOLDER").unwrap_or_else(|_| USDT_MAINNET.to_string());
    let holder_addr = Address::from_base58(&holder).unwrap();
    let mut data = vec![0x70, 0xa0, 0x82, 0x31]; // balanceOf(address) selector
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(holder_addr.as_evm_bytes());
    let trigger = proto::TriggerSmartContract {
        owner_address: Address::ZERO.as_bytes().to_vec(),
        contract_address: Address::from_base58(USDT_MAINNET).unwrap().as_bytes().to_vec(),
        data: data.into(),
        ..Default::default()
    };
    let ext = client.trigger_constant_contract(req(trigger, &api_key)).await.unwrap().into_inner();
    write_fixture("constant_call_balanceof.bin", &ext.encode_to_vec());

    // Transaction info (env-gated: supply real txids to capture).
    for (var, file) in
        [("TRONZ_TX_SUCCESS", "tx_info_success.bin"), ("TRONZ_TX_REVERTED", "tx_info_reverted.bin")]
    {
        match std::env::var(var) {
            Ok(txid) => {
                let info = client
                    .get_transaction_info_by_id(req(
                        proto::BytesMessage { value: hex_decode(&txid) },
                        &api_key,
                    ))
                    .await
                    .unwrap()
                    .into_inner();
                write_fixture(file, &info.encode_to_vec());
            }
            Err(_) => eprintln!("{var} unset; skipping {file}"),
        }
    }
}
