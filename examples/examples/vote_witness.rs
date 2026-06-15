//! Cast votes for super representative (SR) candidates on the Nile testnet.
//!
//! 1 TRON Power (TP) = 1 TRX frozen via Stake 2.0. Votes cost no TRX but
//! require TRON Power. Submitting a new vote transaction replaces all existing
//! votes — you cannot partially update votes.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key with staked TRX (for TRON Power)
//!   TRON_TO          — SR candidate address to vote for
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_VOTES       — number of votes to cast (default: 1)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_TO=<sr-addr> cargo run -p examples --example vote_witness
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let sr_str = std::env::var("TRON_TO").expect("TRON_TO env var required (SR candidate address)");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let votes: i64 = std::env::var("TRON_VOTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();
    let sr: tronz::Address = sr_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Check TRON Power ──────────────────────────────────────────────────────

    let res = provider.get_account_resource(me).await?;
    println!("=== Voter {} ===", me);
    println!("  TRON Power used  : {} sun", res.tron_power_used.as_sun());
    println!("  TRON Power limit : {} sun", res.tron_power_limit.as_sun());

    let tron_power = res.tron_power_limit.as_sun() - res.tron_power_used.as_sun();
    println!(
        "  available TP     : {tron_power} sun ({} TP)",
        tron_power / 1_000_000
    );

    if tron_power < votes * 1_000_000 {
        anyhow::bail!(
            "not enough TRON Power: have {tron_power} sun, need {} sun. Stake TRX first.",
            votes * 1_000_000
        );
    }

    // ── Current votes ─────────────────────────────────────────────────────────

    let account = provider.get_account(me).await?;
    if !account.votes.is_empty() {
        println!("\n=== Current votes ===");
        for v in &account.votes {
            println!("  {} : {} votes", v.vote_address, v.vote_count);
        }
        println!("  (submitting new votes will replace all of the above)");
    }

    // ── Cast votes ────────────────────────────────────────────────────────────
    //
    // Call `.vote(sr, count)` multiple times to split votes across SRs.
    // All votes in a single call replace previously cast votes.

    println!("\n=== Casting {votes} vote(s) for {sr} ===");
    let pending = provider.vote_witness().vote(sr, votes).send().await?;

    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── Verify ───────────────────────────────────────────────────────────────

    let after = provider.get_account(me).await?;
    println!("\n=== Votes after ===");
    for v in &after.votes {
        println!("  {} : {} votes", v.vote_address, v.vote_count);
    }

    Ok(())
}
