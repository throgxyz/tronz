//! List on-chain governance proposals and fetch one by ID.
//!
//! On TRON, SR-level governance proposals change chain parameters (e.g. block
//! rewards, energy prices). Only SRs and SR partners may submit or vote on
//! them; anyone may query them.
//!
//! No private key required (read-only).
//!
//! Optional env:
//!   TRON_PROPOSAL_ID — numeric proposal ID to fetch individually (default: 1)
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! cargo run -p examples-queries --example governance_list
//! TRON_PROPOSAL_ID=3 cargo run -p examples-queries --example governance_list
//! ```

use tronz::{ProviderBuilder, TRONGRID_NILE, providers::ext::GovernanceApi as _};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let proposal_id: i64 = std::env::var("TRON_PROPOSAL_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let api_key = std::env::var("TRON_API_KEY").ok();

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── List all proposals ────────────────────────────────────────────────────

    let proposals = provider.list_proposals().await?;

    println!("=== Governance Proposals ({} total) ===", proposals.len());
    for p in &proposals {
        let proposer = p
            .proposer_address
            .map(|a| a.to_string())
            .unwrap_or_else(|| "<unknown>".into());
        println!(
            "  #{:<4}  state={:?}  params={}  proposer={}",
            p.proposal_id,
            p.state,
            p.parameters.len(),
            proposer
        );
    }

    // ── Paginated fetch (first 5) ─────────────────────────────────────────────

    println!("\n=== First 5 proposals (paginated) ===");
    let page = provider.get_paginated_proposal_list(0, 5).await?;
    for p in &page {
        println!(
            "  #{:<4}  expires={}  approvals={}",
            p.proposal_id,
            p.expiration_time,
            p.approvals.len()
        );
    }

    // ── Fetch individual proposal ─────────────────────────────────────────────

    println!("\n=== Proposal #{proposal_id} ===");
    match provider.get_proposal_by_id(proposal_id).await {
        Ok(p) => {
            println!("  state        : {:?}", p.state);
            println!("  proposer     : {:?}", p.proposer_address);
            println!("  create_time  : {}", p.create_time);
            println!("  expiry       : {}", p.expiration_time);
            println!("  approvals    : {}", p.approvals.len());
            println!("  parameters   :");
            let mut params: Vec<_> = p.parameters.iter().collect();
            params.sort_by_key(|(k, _)| *k);
            for (id, val) in params {
                println!("    param[{id}] = {val}");
            }
        }
        Err(e) => println!("  not found: {e}"),
    }

    Ok(())
}
