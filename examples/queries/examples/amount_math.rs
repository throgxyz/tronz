//! TRX ↔ sun conversions and amount arithmetic.
//!
//! TRON denominates all on-chain values in *sun* (`i64`), where:
//!
//! ```text
//! 1 TRX = 1_000_000 sun
//! ```
//!
//! [`Trx`] wraps `i64` sun and provides:
//!
//! - `from_sun` / `from_trx` constructors (both reject negatives)
//! - `as_sun` / `as_trx` accessors
//! - Saturating `+` / `-` operators
//! - Checked `checked_add` / `checked_sub`
//!
//! TRC20 token amounts use [`U256`] (256-bit unsigned) — identical to ERC-20.
//!
//! No network access required.
//!
//! ```bash
//! cargo run -p examples-queries --example amount_math
//! ```

use tronz::{Trx, U256, primitives::SUN_PER_TRX};

fn main() -> anyhow::Result<()> {
    // ── TRX ↔ sun conversions ─────────────────────────────────────────────────

    println!("=== TRX ↔ sun ===");
    println!("  1 TRX   = {} sun  (SUN_PER_TRX)", SUN_PER_TRX);

    let one_trx = Trx::from_trx(1.0)?;
    println!("  1 TRX   → {:?}", one_trx);
    println!("  as sun  : {}", one_trx.as_sun());
    println!("  as trx  : {} TRX", one_trx.as_trx());

    let half = Trx::from_trx(0.5)?;
    println!("\n  0.5 TRX → {:?}", half);
    println!("  as sun  : {}", half.as_sun());

    let from_sun = Trx::from_sun(2_500_000)?;
    println!("\n  2_500_000 sun → {} TRX", from_sun.as_trx());

    // ── Arithmetic ────────────────────────────────────────────────────────────

    println!("\n=== Arithmetic ===");

    let a = Trx::from_trx(10.0)?;
    let b = Trx::from_trx(3.5)?;

    println!("  a = {a}");
    println!("  b = {b}");
    println!("  a + b = {}", a + b);
    println!("  a - b = {}", a - b);

    // Saturating: subtraction never goes below zero (wraps to zero at i64 min,
    // but in practice amounts never reach that boundary).
    let small = Trx::from_sun(1)?;
    let large = Trx::from_sun(i64::MAX)?;
    println!(
        "\n  saturating add  : small + MAX = {}",
        (small + large).as_sun()
    );
    println!("  saturating sub  : 1 - MAX = {}", (small - large).as_sun());

    // ── Checked arithmetic ────────────────────────────────────────────────────

    println!("\n=== Checked arithmetic ===");

    let x = Trx::from_trx(5.0)?;
    let y = Trx::from_trx(2.0)?;

    match x.checked_sub(y) {
        Some(diff) => println!("  5 - 2 = {diff}  (OK)"),
        None => println!("  5 - 2 overflowed  (unexpected)"),
    }

    let overflow = Trx::from_sun(i64::MAX)?.checked_add(Trx::from_sun(1)?);
    println!("  MAX + 1 = {:?}  (None = overflow detected)", overflow);

    // ── Ordering ──────────────────────────────────────────────────────────────

    println!("\n=== Ordering ===");

    let amounts = [
        Trx::from_trx(100.0)?,
        Trx::from_trx(1.5)?,
        Trx::from_trx(50.0)?,
        Trx::from_sun(0)?,
    ];

    let min = amounts.iter().min().copied().unwrap();
    let max = amounts.iter().max().copied().unwrap();
    println!("  min = {min}");
    println!("  max = {max}");

    // ── U256 for TRC20 tokens ─────────────────────────────────────────────────
    //
    // TRC20 balances and transfer amounts use U256, matching the ERC-20 ABI.
    // A 6-decimal token (e.g. USDT) represents 1 unit as 1_000_000 in U256.

    println!("\n=== U256 for TRC20 amounts ===");

    let usdt_decimals: u32 = 6;
    let one_usdt = U256::from(10u64).pow(U256::from(usdt_decimals));
    println!("  1 USDT  = {} (raw U256 with 6 decimals)", one_usdt);

    let hundred_usdt = U256::from(100u64) * one_usdt;
    println!("  100 USDT = {} (raw U256)", hundred_usdt);

    // Human-readable formatting helper (divide by 10^decimals).
    let whole = hundred_usdt / one_usdt;
    let frac = hundred_usdt % one_usdt;
    println!("  display  : {whole}.{frac:0>6} USDT");

    // 18-decimal token (like WETH bridged to TRON).
    let weth_decimals: u32 = 18;
    let one_weth = U256::from(10u64).pow(U256::from(weth_decimals));
    println!("\n  1 WETH  = {} (raw U256 with 18 decimals)", one_weth);

    Ok(())
}
