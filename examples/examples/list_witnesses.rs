//! Fetch and display the super representative (SR) and SR candidate list.
//!
//! TRON has 27 active SRs (in the producing set) and up to 100 SR candidates.
//! SRs are elected by vote; the top 27 by vote count produce blocks.
//!
//! No private key required (read-only).
//!
//! Optional env:
//!   TRON_API_KEY — TronGrid API key
//!
//! ```bash
//! cargo run -p examples --example list_witnesses
//! ```

use tronz::{ProviderBuilder, TRONGRID_NILE, TronProvider};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let api_key = std::env::var("TRON_API_KEY").ok();

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let witnesses = provider.list_witnesses().await?;

    // Sort descending by vote count.
    let mut witnesses = witnesses;
    witnesses.sort_by_key(|w| std::cmp::Reverse(w.vote_count));

    let active_count = witnesses.iter().filter(|w| w.is_active).count();
    println!("=== Super Representatives ({active_count} active) ===");
    println!();
    println!(
        "  {:<3}  {:<35}  {:>12}  {:>8}  {:>8}  URL",
        "#", "Address", "Votes", "Produced", "Missed"
    );
    println!("  {}", "-".repeat(100));

    for (i, w) in witnesses.iter().enumerate() {
        let active_marker = if w.is_active { "*" } else { " " };
        println!(
            "  {active_marker}{:>2}  {:<35}  {:>12}  {:>8}  {:>8}  {}",
            i + 1,
            w.address.to_string(),
            w.vote_count,
            w.total_produced,
            w.total_missed,
            if w.url.len() > 40 {
                &w.url[..40]
            } else {
                &w.url
            },
        );

        // Only show top 30.
        if i >= 29 {
            let remaining = witnesses.len() - 30;
            if remaining > 0 {
                println!("  ... and {remaining} more candidates");
            }
            break;
        }
    }

    println!();
    println!("  * = active SR (top 27, currently producing blocks)");

    Ok(())
}
