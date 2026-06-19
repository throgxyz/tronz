//! Order-book DEX (Market Orders) API extension.
//!
//! Import [`MarketApi`] to add market-order methods to any [`TronProvider`].

use tronz_primitives::Address;

use crate::{
    builders::resolve_owner,
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    transport::TronTransport as _,
    types::{
        ContractType, MarketCancelOrderContract, MarketOrderInfo, MarketOrderPair, MarketPrice,
        MarketSellAssetContract, TransactionRequest,
    },
};

/// Order-book DEX methods, available on any [`TronProvider`].
///
/// # Example
///
/// ```no_run
/// use tronz_provider::ext::MarketApi;
/// # async fn run(provider: impl tronz_provider::TronProvider) -> tronz_provider::Result<()> {
/// // Fetch all active trading pairs
/// let pairs = provider.get_market_pair_list().await?;
///
/// // Place a limit sell order: sell 1_000_000 TRX for at least 500_000 of token "1000001"
/// let pending = provider
///     .market_sell()
///     .sell_token_id("_")
///     .sell_token_quantity(1_000_000)
///     .buy_token_id("1000001")
///     .buy_token_quantity(500_000)
///     .send()
///     .await?;
/// # Ok(()) }
/// ```
pub trait MarketApi: TronProvider + Sized {
    /// Fetch a market order by its 32-byte order ID.
    ///
    /// Returns `None` if no order with that ID exists.
    fn get_market_order_by_id(
        &self,
        order_id: &[u8],
    ) -> impl std::future::Future<Output = Result<Option<MarketOrderInfo>>> + Send;

    /// Fetch all market orders placed by `address`.
    fn get_market_order_by_account(
        &self,
        address: Address,
    ) -> impl std::future::Future<Output = Result<Vec<MarketOrderInfo>>> + Send;

    /// Fetch the price levels in the order book for a trading pair.
    fn get_market_price_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<MarketPrice>>> + Send;

    /// Fetch all open orders for a trading pair.
    fn get_market_order_list_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<MarketOrderInfo>>> + Send;

    /// Fetch all active trading pairs on the order-book DEX.
    fn get_market_pair_list(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<MarketOrderPair>>> + Send;

    /// Start building a limit sell order.
    fn market_sell(&self) -> MarketSellBuilder<'_, Self>;

    /// Start building a market-order cancellation.
    fn market_cancel(&self) -> MarketCancelBuilder<'_, Self>;
}

impl<P: TronProvider> MarketApi for P {
    async fn get_market_order_by_id(
        &self,
        order_id: &[u8],
    ) -> Result<Option<MarketOrderInfo>> {
        self.transport()
            .get_market_order_by_id(order_id)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_market_order_by_account(
        &self,
        address: Address,
    ) -> Result<Vec<MarketOrderInfo>> {
        self.transport()
            .get_market_order_by_account(address)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_market_price_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> Result<Vec<MarketPrice>> {
        self.transport()
            .get_market_price_by_pair(sell_token_id, buy_token_id)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_market_order_list_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> Result<Vec<MarketOrderInfo>> {
        self.transport()
            .get_market_order_list_by_pair(sell_token_id, buy_token_id)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_market_pair_list(&self) -> Result<Vec<MarketOrderPair>> {
        self.transport()
            .get_market_pair_list()
            .await
            .map_err(|e| Error::from(e.into()))
    }

    fn market_sell(&self) -> MarketSellBuilder<'_, Self> {
        MarketSellBuilder::new(self)
    }

    fn market_cancel(&self) -> MarketCancelBuilder<'_, Self> {
        MarketCancelBuilder::new(self)
    }
}

// ── MarketSellBuilder ─────────────────────────────────────────────────────────

/// Builds a limit sell order on the order-book DEX.
///
/// Created by [`MarketApi::market_sell`].
pub struct MarketSellBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    sell_token_id: Option<String>,
    sell_token_quantity: Option<i64>,
    buy_token_id: Option<String>,
    buy_token_quantity: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> MarketSellBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            sell_token_id: None,
            sell_token_quantity: None,
            buy_token_id: None,
            buy_token_quantity: None,
            memo: None,
        }
    }

    /// Override the seller address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the token ID being sold (`"_"` for TRX, numeric string for TRC10).
    pub fn sell_token_id(mut self, id: impl Into<String>) -> Self {
        self.sell_token_id = Some(id.into());
        self
    }

    /// Set the quantity of the sell token to offer.
    pub fn sell_token_quantity(mut self, qty: i64) -> Self {
        self.sell_token_quantity = Some(qty);
        self
    }

    /// Set the token ID to receive.
    pub fn buy_token_id(mut self, id: impl Into<String>) -> Self {
        self.buy_token_id = Some(id.into());
        self
    }

    /// Set the minimum quantity of the buy token to accept (sets the limit price).
    pub fn buy_token_quantity(mut self, qty: i64) -> Self {
        self.buy_token_quantity = Some(qty);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the sell order.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let sell_token_id =
            self.sell_token_id.ok_or(Error::missing_field("sell_token_id"))?;
        let sell_token_quantity =
            self.sell_token_quantity.ok_or(Error::missing_field("sell_token_quantity"))?;
        let buy_token_id =
            self.buy_token_id.ok_or(Error::missing_field("buy_token_id"))?;
        let buy_token_quantity =
            self.buy_token_quantity.ok_or(Error::missing_field("buy_token_quantity"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::MarketSellAsset(MarketSellAssetContract {
                owner_address: owner,
                sell_token_id,
                sell_token_quantity,
                buy_token_id,
                buy_token_quantity,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── MarketCancelBuilder ───────────────────────────────────────────────────────

/// Builds a market-order cancellation transaction.
///
/// Created by [`MarketApi::market_cancel`].
pub struct MarketCancelBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    order_id: Option<Vec<u8>>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> MarketCancelBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            order_id: None,
            memo: None,
        }
    }

    /// Override the canceller address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the 32-byte order ID to cancel.
    pub fn order_id(mut self, id: impl Into<Vec<u8>>) -> Self {
        self.order_id = Some(id.into());
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the cancellation.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let order_id = self.order_id.ok_or(Error::missing_field("order_id"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::MarketCancelOrder(MarketCancelOrderContract {
                owner_address: owner,
                order_id,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
