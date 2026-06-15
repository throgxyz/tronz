//! Connect to a custom TRON node (plain HTTP/2, no TLS).
//!
//! Useful for:
//! - Local full nodes or private test networks
//! - Self-hosted TRON nodes where you control the endpoint
//!
//! The node must accept gRPC on port 50051 (the TRON default). No TLS is
//! required — use the `grpc` feature (not `grpc-tls`) when connecting to
//! plain HTTP/2 endpoints.
//!
//! Optional env:
//!   TRON_NODE_URL — gRPC URL of the local node (default: http://127.0.0.1:50051)
//!   TRON_API_KEY  — TronGrid API key (optional; useful if fronting through TronGrid)
//!
//! ```bash
//! cargo run -p examples --example connect_custom
//! ```

use tronz::{ProviderBuilder, TronProvider};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let node_url =
        std::env::var("TRON_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:50051".to_owned());
    let api_key = std::env::var("TRON_API_KEY").ok();

    println!("=== Connect to custom node ===");
    println!("  url : {node_url}");

    // ── Connect ──────────────────────────────────────────────────────────────
    //
    // Pass any URL string to `on_grpc` — it works for both custom nodes and
    // the well-known TronGrid constants (TRONGRID_MAINNET, TRONGRID_NILE).
    // The API key is injected as the `tron-pro-api-key` gRPC metadata header
    // on every request when set.
    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(node_url.as_str())
        .await?;

    // ── Smoke test ────────────────────────────────────────────────────────────

    let block = provider.get_now_block().await?;
    println!("  connected   : OK");
    println!("  block       : #{}", block.number);
    println!("  timestamp   : {} ms", block.timestamp);
    println!("  hash        : 0x{}", hex::encode(block.hash));

    Ok(())
}
