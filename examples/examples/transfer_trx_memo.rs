//! Transfer TRX with an on-chain memo (`raw.data`).
//!
//! TRON transactions carry an optional `data` field (up to 500 bytes) in the
//! raw transaction body. It is commonly used as a human-readable reference,
//! e.g. an exchange deposit identifier. The field does not affect execution.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_TO          — recipient (defaults to a well-known Nile address)
//!   TRON_AMOUNT_SUN  — amount in sun (default: 1 sun)
//!   TRON_MEMO        — memo string (default: "tronz example transfer")
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples --example transfer_trx_memo
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner, Trx};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let amount_sun: i64 = std::env::var("TRON_AMOUNT_SUN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let memo = std::env::var("TRON_MEMO").unwrap_or_else(|_| "tronz example transfer".to_owned());

    let signer = LocalSigner::from_hex(&key_hex)?;
    let from = signer.address();

    // Default to a well-known Nile address; TRON does not allow self-transfers.
    let to: tronz::Address = std::env::var("TRON_TO")
        .ok()
        .map(|s| s.parse().expect("valid TRON_TO address"))
        .unwrap_or_else(|| "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap());

    let amount = Trx::from_sun(amount_sun)?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    println!("=== TRX transfer with memo ===");
    println!("  from   : {from}");
    println!("  to     : {to}");
    println!("  amount : {amount}");
    println!("  memo   : {:?}", memo);

    // ── Attach memo and send ──────────────────────────────────────────────────
    //
    // `.memo()` accepts any `Into<Vec<u8>>` — bytes are stored verbatim.
    // String memos should be UTF-8, but the protocol does not enforce this.

    let pending = provider
        .send_trx()
        .to(to)
        .amount(amount)
        .memo(memo.as_bytes())
        .send()
        .await?;

    let tx_id = pending.tx_id();
    println!("\n  tx_id  : 0x{}", hex::encode(tx_id));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);
    println!("  net fee: {} sun", info.net_fee.as_sun());

    // ── Fetch receipt and confirm ─────────────────────────────────────────────
    //
    // The memo is stored in `Transaction.raw_data.data` inside the protobuf.
    // Use a block explorer or the full `get_transaction` gRPC call to inspect it.
    println!(
        "\n  explorer : https://nile.tronscan.org/#/transaction/{}",
        hex::encode(tx_id)
    );

    Ok(())
}
