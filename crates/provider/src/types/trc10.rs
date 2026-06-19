//! TRC10 token metadata types.

use tronz_primitives::Address;

/// Metadata for a TRC10 (native TRON) token.
///
/// Returned by [`Trc10Api::get_asset_info`](crate::ext::Trc10Api::get_asset_info).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct AssetInfo {
    /// Numeric token ID (e.g. `"1000001"`).
    pub id: String,
    /// Full token name (e.g. `"BitTorrent"`).
    pub name: String,
    /// Token symbol / abbreviation (e.g. `"BTT"`).
    pub abbr: String,
    /// Decimal precision (0–6).
    pub decimals: i32,
    /// Address of the token issuer.
    pub owner: Address,
    /// Total supply in the smallest unit.
    pub total_supply: i64,
    /// Token description URL.
    pub url: String,
}
