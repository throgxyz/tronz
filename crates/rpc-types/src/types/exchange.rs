//! Domain types for the TRON built-in DEX (Bancor TRC10 exchange).

use tronz_primitives::Address;

/// State of an on-chain exchange pair.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ExchangeInfo {
    /// Unique exchange ID assigned by the network.
    pub exchange_id: i64,
    /// Address of the account that created the exchange.
    pub creator_address: Address,
    /// Creation timestamp (unix ms).
    pub create_time: i64,
    /// Token ID of the first token (`"_"` for TRX, numeric string for TRC10).
    pub first_token_id: String,
    /// Balance of the first token in the pool.
    pub first_token_balance: i64,
    /// Token ID of the second token.
    pub second_token_id: String,
    /// Balance of the second token in the pool.
    pub second_token_balance: i64,
}
