//! Update owner / active multi-sig permissions and key weights.
//!
//! TRON accounts support multi-signature: you can require M-of-N signatures
//! from different keys to authorize transactions. This is useful for:
//!
//! - Corporate accounts requiring multiple signers
//! - Cold-warm key separation (owner controls hot keys)
//! - Recovery key setups
//!
//! Permission structure:
//! - **owner** (id=0): can do everything, including updating permissions
//! - **active** (id=2+): can execute transactions within allowed operations
//! - **witness** (id=1): block production only (for SRs)
//!
//! WARNING: This example shows how to add an additional key to the active
//! permission. Misconfiguring permissions can lock you out of your account.
//! Test on Nile testnet only.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key (owner permission required)
//!   TRON_TO          — second key address to add to active permissions
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_TO=<second-key-addr> \
//!   cargo run -p examples --example account_permissions
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner,
    providers::types::{Permission, PermissionKey},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let second_key_str =
        std::env::var("TRON_TO").expect("TRON_TO env var required (second key address)");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();
    let second_key: tronz::Address = second_key_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Current permissions ───────────────────────────────────────────────────

    let account = provider.get_account(me).await?;
    println!("=== Account {} ===", me);
    println!("\n  Owner permission:");
    if let Some(ref owner_perm) = account.permissions.owner {
        println!("    threshold : {}", owner_perm.threshold);
        for k in &owner_perm.keys {
            println!("    key : {}  weight={}", k.address, k.weight);
        }
    } else {
        println!("    (default — single key = {me})");
    }

    println!("\n  Active permissions:");
    if account.permissions.actives.is_empty() {
        println!("    (default)");
    }
    for perm in &account.permissions.actives {
        println!(
            "    id={}  name={:?}  threshold={}",
            perm.id, perm.permission_name, perm.threshold
        );
        for k in &perm.keys {
            println!("      key : {}  weight={}", k.address, k.weight);
        }
    }

    // ── Build new permissions ─────────────────────────────────────────────────
    //
    // Add a 1-of-2 active permission: either the primary key OR the second key
    // can authorize transactions unilaterally (threshold=1, each key weight=1).

    println!("\n=== Adding second key to active permissions ===");
    println!("  second key : {second_key}");
    println!("  policy     : 1-of-2 (either key can sign independently)");

    // Owner permission: keep as-is (just the primary key, weight 1, threshold 1).
    let owner_permission = Permission {
        id: 0,
        permission_name: "owner".to_owned(),
        threshold: 1,
        keys: vec![PermissionKey {
            address: me,
            weight: 1,
        }],
    };

    // Active permission: both keys, threshold 1 (1-of-2).
    let active_permission = Permission {
        id: 2,
        permission_name: "active".to_owned(),
        threshold: 1,
        keys: vec![
            PermissionKey {
                address: me,
                weight: 1,
            },
            PermissionKey {
                address: second_key,
                weight: 1,
            },
        ],
    };

    println!("\n  broadcasting permission update…");
    let pending = provider
        .update_permissions()
        .owner_permission(owner_permission)
        .actives(vec![active_permission])
        .send()
        .await?;

    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── Verify ───────────────────────────────────────────────────────────────

    let after = provider.get_account(me).await?;
    println!("\n=== Updated permissions ===");
    if let Some(ref owner_perm) = after.permissions.owner {
        println!("  owner (threshold={})", owner_perm.threshold);
        for k in &owner_perm.keys {
            println!("    {} w={}", k.address, k.weight);
        }
    }
    for perm in &after.permissions.actives {
        println!("  active id={}  threshold={}", perm.id, perm.threshold);
        for k in &perm.keys {
            println!("    {} w={}", k.address, k.weight);
        }
    }

    Ok(())
}
