//! Integration tests against the TRON Nile testnet.
//!
//! All tests are `#[ignore]` by default — they require live network access.
//! Run them with:
//!
//! ```text
//! cargo test -p tronz-provider --test integration -- --ignored
//! ```
//!
//! Write tests additionally need a funded Nile account supplied via env:
//!
//! ```text
//! TRON_TEST_KEY=<64-char-hex> cargo test -p tronz-provider --test integration -- --ignored
//! ```
//!
//! # What these tests validate
//!
//! 1. **Connectivity** — the gRPC endpoint responds and codec round-trips work.
//! 2. **Not-found edge cases** — the codec correctly returns `Ok(None)` (not `Err(...)`) when the
//!    node returns default/empty proto messages. This is critical: `PendingTransaction` polls on
//!    `Ok(None)` to detect unconfirmed transactions; any `Err` variant aborts the poll early.
//! 3. **Never-activated accounts** — node returns `Account { address: [] }` for addresses that have
//!    never received funds; we must fill in the queried address and set `is_activated = false`, NOT
//!    return an error.
//! 4. **Full send flow** — fill → sign → broadcast → poll for receipt.

use tronz_primitives::{Address, TxId};
use tronz_provider::{
    ProviderBuilder, TronProvider,
    ext::Trc10Api as _,
    transport::{TronTransport as _, grpc::TRONGRID_NILE},
};
use tronz_signer::TronSigner as _;

// ── Well-known Nile fixtures ──────────────────────────────────────────────────

/// A Nile account that has been active and holds TRX.
/// TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t is mainnet USDT but also exists on
/// Nile with TRX balance — verified to parse and return data from the Nile node.
const NILE_ACTIVE_ADDR: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

/// USDT (TRC20) contract on the Nile testnet.
const NILE_USDT_CONTRACT: &str = "TXLAQ63Xg1NAzckPwKHvzw7CSEmLMEqcdj";

/// A TRC10 token ID that exists on Nile (the canonical "WIN" token).
const NILE_TRC10_TOKEN_ID: &str = "1000001";

/// A well-formed but nonexistent TRC10 token ID.
const NILE_BOGUS_TOKEN_ID: &str = "9999999999";

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a plain read-only provider connected to Nile.
async fn read_provider() -> impl TronProvider {
    ProviderBuilder::new()
        .on_grpc(TRONGRID_NILE)
        .await
        .expect("failed to connect to Nile testnet")
}

/// Read `TRON_TEST_KEY` from the environment.
///
/// Returns `None` (and skips the test) if the variable is absent.
fn test_signer() -> Option<tronz_signer::LocalSigner> {
    let hex = std::env::var("TRON_TEST_KEY").ok()?;
    Some(tronz_signer::LocalSigner::from_hex(&hex).expect("TRON_TEST_KEY is not a valid hex key"))
}

// ── Connectivity ──────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires network"]
async fn test_get_now_block() {
    let provider = read_provider().await;
    let block = provider
        .get_now_block()
        .await
        .expect("get_now_block failed");

    assert!(
        block.number > 0,
        "block number should be positive, got {}",
        block.number
    );
    assert_ne!(
        block.hash,
        tronz_primitives::B256::ZERO,
        "block hash should be non-zero"
    );
    assert!(block.timestamp > 0, "block timestamp should be positive");

    eprintln!(
        "Nile head: block #{} (ts={})",
        block.number, block.timestamp
    );
}

// ── Account ───────────────────────────────────────────────────────────────────

/// A never-activated account must come back successfully (not as an error).
///
/// The TRON node returns `Account { address: [] }` for unknown addresses.
/// Our codec fills in the queried address and sets `is_activated = false`.
/// Returning an error here would break any wallet that checks balances of
/// fresh addresses before they receive their first TRX.
#[tokio::test]
#[ignore = "requires network"]
async fn test_get_account_never_activated_is_not_an_error() {
    let provider = read_provider().await;

    // Use a deterministic fresh address derived from the lowest valid key.
    // This address has no funds on Nile and should never have been activated.
    let signer = tronz_signer::LocalSigner::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000002",
    )
    .unwrap();
    let fresh_addr = signer.address();

    let account = provider
        .get_account(fresh_addr)
        .await
        .expect("get_account should succeed even for a never-activated address");

    assert_eq!(
        account.address, fresh_addr,
        "returned address should match the queried address"
    );
    assert!(
        !account.is_activated,
        "fresh address must not be marked as activated"
    );
    assert_eq!(
        account.balance,
        tronz_primitives::Trx::ZERO,
        "fresh address should have zero TRX balance"
    );
}

#[tokio::test]
#[ignore = "requires network"]
async fn test_get_account_activated() {
    let provider = read_provider().await;
    let addr = NILE_ACTIVE_ADDR
        .parse::<Address>()
        .expect("invalid NILE_ACTIVE_ADDR constant");

    let account = provider
        .get_account(addr)
        .await
        .expect("get_account failed");
    assert_eq!(account.address, addr);
    assert!(
        account.is_activated,
        "known active account should be activated"
    );

    eprintln!(
        "Account {} balance: {} TRX",
        addr,
        account.balance.as_sun() as f64 / 1_000_000.0
    );
}

// ── Transaction not found ─────────────────────────────────────────────────────

/// TRON nodes return an empty `TransactionInfo { id: [] }` for unknown/unconfirmed
/// tx IDs. Our codec MUST translate that into `Ok(None)`.
///
/// If it returns `Err(TransportErrorKind::Malformed)` instead, the `PendingTransaction`
/// polling loop will abort immediately on the first poll (which happens before
/// the tx is even indexed), making all `.send().await?.get_receipt().await`
/// calls time out immediately.
#[tokio::test]
#[ignore = "requires network"]
async fn test_get_transaction_info_returns_not_found_for_unknown_txid() {
    let provider = read_provider().await;
    // An all-zero txid cannot correspond to any real transaction.
    let fake_id = TxId::from([0u8; 32]);

    let result = provider.get_transaction_info(fake_id).await;
    match result {
        Ok(None) => {
            // Correct — node hasn't indexed this tx, polling will continue.
        }
        Err(other) => panic!(
            "Expected Ok(None) for unknown txid, got an error: {other:?}\n\
             This will break PendingTransaction polling!"
        ),
        Ok(Some(info)) => panic!("Expected Ok(None) for all-zero txid, got: {info:?}"),
    }
}

// ── TRC10 ─────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires network"]
async fn test_get_asset_info_existing_token() {
    let provider = read_provider().await;
    let info = provider
        .get_asset_info(NILE_TRC10_TOKEN_ID)
        .await
        .expect("get_asset_info failed for a known token")
        .expect("known token should be found");

    assert_eq!(
        info.id, NILE_TRC10_TOKEN_ID,
        "returned token id should match query"
    );
    assert!(!info.name.is_empty(), "token name should not be empty");

    eprintln!(
        "TRC10 {}: name={}, abbr={}, decimals={}",
        info.id, info.name, info.abbr, info.decimals
    );
}

/// Querying a non-existent TRC10 token should return `Ok(None)`, not a panic
/// or a `Malformed` error.
#[tokio::test]
#[ignore = "requires network"]
async fn test_get_asset_info_nonexistent_token_returns_not_found() {
    let provider = read_provider().await;
    let result = provider
        .get_asset_info(NILE_BOGUS_TOKEN_ID)
        .await
        .expect("transport should not error for bogus token");

    assert!(
        result.is_none(),
        "expected None for bogus token, got: {result:?}"
    );
}

/// `trc10_balance` for an account that holds none of the token must return `0`,
/// not an error — it reads the `trc10_balances` map and defaults to zero.
#[tokio::test]
#[ignore = "requires network"]
async fn test_trc10_balance_zero_for_holder_without_token() {
    let provider = read_provider().await;

    // Fresh address — holds no TRC10 tokens.
    let signer = tronz_signer::LocalSigner::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000002",
    )
    .unwrap();
    let addr = signer.address();

    let balance = provider
        .trc10_balance(addr, NILE_TRC10_TOKEN_ID)
        .await
        .expect("trc10_balance should return Ok(0) for an account with no TRC10 tokens");

    assert_eq!(balance, 0, "expected zero balance, got {balance}");
}

// ── Write tests (require funded account) ──────────────────────────────────────

/// Transfer a tiny amount of TRX and wait for the receipt.
///
/// Requires `TRON_TEST_KEY` to be set to a funded Nile private key.
#[tokio::test]
#[ignore = "requires funded account"]
async fn test_trx_transfer_and_receipt() {
    use tronz_primitives::Trx;

    let signer = match test_signer() {
        Some(s) => s,
        None => {
            eprintln!("Skipping: TRON_TEST_KEY not set");
            return;
        }
    };
    let from = signer.address();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .on_grpc(TRONGRID_NILE)
        .await
        .expect("connect failed");

    // Send 1 sun to ourselves — minimal cost, verifiable on-chain.
    let pending = provider
        .send_trx()
        .to(from)
        .amount(Trx::from_sun_unchecked(1))
        .send()
        .await
        .expect("transfer failed");

    eprintln!("Broadcast tx: {}", pending.tx_id());

    let info = pending.get_receipt().await.expect("get_receipt failed");
    eprintln!(
        "Confirmed: block #{}, energy_used={}",
        info.block_number, info.energy_usage
    );

    assert_eq!(
        info.status,
        tronz_provider::types::TxStatus::Success,
        "TRX self-transfer should succeed"
    );
}

/// Full TRC20 USDT read — `balanceOf` via `trigger_constant_contract`.
///
/// This validates that the contract call + ABI decode path works end to end.
#[tokio::test]
#[ignore = "requires network"]
async fn test_usdt_balance_of_constant_call() {
    use tronz_primitives::Trx;
    use tronz_provider::types::TriggerSmartContract;

    let provider = read_provider().await;

    let contract = NILE_USDT_CONTRACT
        .parse::<Address>()
        .expect("bad USDT address");

    // `balanceOf(address)` selector = 0x70a08231; arg = NILE_ACTIVE_ADDR (padded to 32 bytes).
    let owner = NILE_ACTIVE_ADDR
        .parse::<Address>()
        .expect("bad active addr");
    let mut data = vec![0x70u8, 0xa0, 0x82, 0x31];
    // ABI-encode the address: 12 zero bytes + 20 address bytes.
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(owner.as_evm_bytes());

    let params = TriggerSmartContract {
        owner_address: owner,
        contract_address: contract,
        call_value: Trx::ZERO,
        data: data.into(),
        call_token_value: Trx::ZERO,
        token_id: 0,
    };

    let result = provider
        .transport()
        .trigger_constant_contract(params)
        .await
        .expect("trigger_constant_contract failed");

    // We just check that the output is 32 bytes (a uint256) — not the actual value.
    assert_eq!(
        result.output.len(),
        32,
        "balanceOf should return a 32-byte uint256"
    );

    let balance_u256 = tronz_primitives::U256::from_be_slice(&result.output);
    eprintln!("USDT balanceOf({owner}): {balance_u256} (6 decimals)");
}

/// Estimate energy for a USDT `transfer` call.
///
/// Validates `estimate_energy` returns a positive number for a real contract.
#[tokio::test]
#[ignore = "requires network"]
async fn test_estimate_energy_usdt_transfer() {
    use tronz_primitives::Trx;
    use tronz_provider::types::TriggerSmartContract;

    let provider = read_provider().await;

    let contract = NILE_USDT_CONTRACT
        .parse::<Address>()
        .expect("bad USDT address");
    let caller = NILE_ACTIVE_ADDR
        .parse::<Address>()
        .expect("bad active addr");

    // `transfer(address,uint256)` selector = 0xa9059cbb
    // recipient = NILE_ACTIVE_ADDR, amount = 1
    let mut data = vec![0xa9u8, 0x05, 0x9c, 0xbb];
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(caller.as_evm_bytes()); // recipient
    data.extend_from_slice(&[0u8; 31]);
    data.push(1u8); // amount = 1

    let params = TriggerSmartContract {
        owner_address: caller,
        contract_address: contract,
        call_value: Trx::ZERO,
        data: data.into(),
        call_token_value: Trx::ZERO,
        token_id: 0,
    };

    let energy = provider
        .estimate_energy(params)
        .await
        .expect("estimate_energy failed");

    assert!(
        energy > 0,
        "energy estimate should be positive, got {energy}"
    );
    eprintln!("Estimated energy for USDT transfer: {energy}");
}
