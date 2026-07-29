//! Domain types for the TRON order-book DEX (Market Orders).

use tronz_primitives::{Address, B256};

/// State of a market order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketOrderState {
    /// Order is open and awaiting a match.
    Active,
    /// Order has been fully filled.
    Inactive,
    /// Order was cancelled by its owner.
    Canceled,
}

/// A single market order.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MarketOrderInfo {
    /// Unique order ID (32-byte hash).
    pub order_id: B256,
    /// Address of the account that placed the order.
    pub owner_address: Address,
    /// Creation timestamp (unix ms).
    pub create_time: i64,
    /// Token ID being sold (`"_"` for TRX, numeric string for TRC10).
    pub sell_token_id: String,
    /// Quantity of the sell token at order creation.
    pub sell_token_quantity: i64,
    /// Token ID being bought.
    pub buy_token_id: String,
    /// Minimum quantity of the buy token to receive (limit price).
    pub buy_token_quantity: i64,
    /// Remaining sell quantity not yet matched.
    pub sell_token_quantity_remain: i64,
    /// Sell quantity returned when the order expires with insufficient balance.
    pub sell_token_quantity_return: i64,
    /// Current order state.
    pub state: MarketOrderState,
}

/// A trading pair on the order-book DEX.
#[derive(Clone, Debug)]
pub struct MarketOrderPair {
    /// Token ID being sold.
    pub sell_token_id: String,
    /// Token ID being bought.
    pub buy_token_id: String,
}

/// A single price level in the order book.
#[derive(Clone, Copy, Debug)]
pub struct MarketPrice {
    /// Sell token quantity at this price level.
    pub sell_token_quantity: i64,
    /// Buy token quantity at this price level.
    pub buy_token_quantity: i64,
}
