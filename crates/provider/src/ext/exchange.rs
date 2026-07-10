//! DEX (built-in Bancor TRC10 exchange) API extension.
//!
//! Import [`ExchangeApi`] to add exchange methods to any [`TronProvider`].

use tronz_primitives::Address;

use crate::{
    builders::resolve_owner,
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    transport::TronTransport as _,
    types::{
        ContractType, ExchangeCreateContract, ExchangeInfo, ExchangeInjectContract,
        ExchangeTransactionContract, ExchangeWithdrawContract, TransactionRequest,
    },
};

/// Built-in Bancor DEX methods (TRC10 exchange pairs), available on any [`TronProvider`].
///
/// # Example
///
/// ```no_run
/// use tronz_provider::ext::ExchangeApi;
/// # async fn run(provider: impl tronz_provider::TronProvider) -> tronz_provider::Result<()> {
/// // List all exchanges
/// let exchanges = provider.list_exchanges().await?;
/// for ex in &exchanges {
///     println!("exchange {} : {} / {}", ex.exchange_id, ex.first_token_id, ex.second_token_id);
/// }
///
/// // Execute a swap: sell 1_000_000 units of token "1000001", expect at least 1 TRX
/// let pending = provider
///     .exchange_trade()
///     .exchange_id(1)
///     .token_id("1000001")
///     .quant(1_000_000)
///     .expected(1_000_000)
///     .send()
///     .await?;
/// # Ok(()) }
/// ```
pub trait ExchangeApi: TronProvider + Sized {
    /// List all exchange pairs on-chain.
    fn list_exchanges(&self)
    -> impl std::future::Future<Output = Result<Vec<ExchangeInfo>>> + Send;

    /// Fetch a paginated list of exchange pairs.
    fn get_paginated_exchange_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> impl std::future::Future<Output = Result<Vec<ExchangeInfo>>> + Send;

    /// Fetch a single exchange pair by its ID.
    ///
    /// Returns `None` if no exchange with that ID exists.
    fn get_exchange_by_id(
        &self,
        exchange_id: i64,
    ) -> impl std::future::Future<Output = Result<Option<ExchangeInfo>>> + Send;

    /// Start building a transaction that creates a new TRC10 exchange pair.
    fn exchange_create(&self) -> ExchangeCreateBuilder<'_, Self>;

    /// Start building a liquidity injection transaction.
    fn exchange_inject(&self) -> ExchangeInjectBuilder<'_, Self>;

    /// Start building a liquidity withdrawal transaction.
    fn exchange_withdraw(&self) -> ExchangeWithdrawBuilder<'_, Self>;

    /// Start building a swap (trade) transaction.
    fn exchange_trade(&self) -> ExchangeTradeBuilder<'_, Self>;
}

impl<P: TronProvider> ExchangeApi for P {
    async fn list_exchanges(&self) -> Result<Vec<ExchangeInfo>> {
        self.transport().list_exchanges().await.map_err(|e| Error::from(e.into()))
    }

    async fn get_paginated_exchange_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ExchangeInfo>> {
        self.transport()
            .get_paginated_exchange_list(offset, limit)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_exchange_by_id(&self, exchange_id: i64) -> Result<Option<ExchangeInfo>> {
        self.transport().get_exchange_by_id(exchange_id).await.map_err(|e| Error::from(e.into()))
    }

    fn exchange_create(&self) -> ExchangeCreateBuilder<'_, Self> {
        ExchangeCreateBuilder::new(self)
    }

    fn exchange_inject(&self) -> ExchangeInjectBuilder<'_, Self> {
        ExchangeInjectBuilder::new(self)
    }

    fn exchange_withdraw(&self) -> ExchangeWithdrawBuilder<'_, Self> {
        ExchangeWithdrawBuilder::new(self)
    }

    fn exchange_trade(&self) -> ExchangeTradeBuilder<'_, Self> {
        ExchangeTradeBuilder::new(self)
    }
}

// ── ExchangeCreateBuilder ─────────────────────────────────────────────────────

/// Builds a transaction that creates a new TRC10 exchange pair.
///
/// Created by [`ExchangeApi::exchange_create`].
pub struct ExchangeCreateBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    first_token_id: Option<String>,
    first_token_balance: Option<i64>,
    second_token_id: Option<String>,
    second_token_balance: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> ExchangeCreateBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            first_token_id: None,
            first_token_balance: None,
            second_token_id: None,
            second_token_balance: None,
            memo: None,
        }
    }

    /// Override the creator address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the first token ID (`"_"` for TRX, numeric string for TRC10).
    pub fn first_token_id(mut self, id: impl Into<String>) -> Self {
        self.first_token_id = Some(id.into());
        self
    }

    /// Set the initial deposit of the first token.
    pub fn first_token_balance(mut self, balance: i64) -> Self {
        self.first_token_balance = Some(balance);
        self
    }

    /// Set the second token ID.
    pub fn second_token_id(mut self, id: impl Into<String>) -> Self {
        self.second_token_id = Some(id.into());
        self
    }

    /// Set the initial deposit of the second token.
    pub fn second_token_balance(mut self, balance: i64) -> Self {
        self.second_token_balance = Some(balance);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the exchange-create transaction.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let first_token_id = self.first_token_id.ok_or(Error::missing_field("first_token_id"))?;
        let first_token_balance =
            self.first_token_balance.ok_or(Error::missing_field("first_token_balance"))?;
        let second_token_id =
            self.second_token_id.ok_or(Error::missing_field("second_token_id"))?;
        let second_token_balance =
            self.second_token_balance.ok_or(Error::missing_field("second_token_balance"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ExchangeCreate(ExchangeCreateContract {
                owner_address: owner,
                first_token_id,
                first_token_balance,
                second_token_id,
                second_token_balance,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── ExchangeInjectBuilder ─────────────────────────────────────────────────────

/// Builds a liquidity injection transaction.
///
/// Created by [`ExchangeApi::exchange_inject`].
pub struct ExchangeInjectBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    exchange_id: Option<i64>,
    token_id: Option<String>,
    quant: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> ExchangeInjectBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self { provider, owner: None, exchange_id: None, token_id: None, quant: None, memo: None }
    }

    /// Override the sender address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the exchange ID.
    pub fn exchange_id(mut self, id: i64) -> Self {
        self.exchange_id = Some(id);
        self
    }

    /// Set the token ID being injected.
    pub fn token_id(mut self, id: impl Into<String>) -> Self {
        self.token_id = Some(id.into());
        self
    }

    /// Set the amount to inject.
    pub fn quant(mut self, quant: i64) -> Self {
        self.quant = Some(quant);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the inject transaction.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let exchange_id = self.exchange_id.ok_or(Error::missing_field("exchange_id"))?;
        let token_id = self.token_id.ok_or(Error::missing_field("token_id"))?;
        let quant = self.quant.ok_or(Error::missing_field("quant"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ExchangeInject(ExchangeInjectContract {
                owner_address: owner,
                exchange_id,
                token_id,
                quant,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── ExchangeWithdrawBuilder ───────────────────────────────────────────────────

/// Builds a liquidity withdrawal transaction.
///
/// Created by [`ExchangeApi::exchange_withdraw`].
pub struct ExchangeWithdrawBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    exchange_id: Option<i64>,
    token_id: Option<String>,
    quant: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> ExchangeWithdrawBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self { provider, owner: None, exchange_id: None, token_id: None, quant: None, memo: None }
    }

    /// Override the sender address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the exchange ID.
    pub fn exchange_id(mut self, id: i64) -> Self {
        self.exchange_id = Some(id);
        self
    }

    /// Set the token ID being withdrawn.
    pub fn token_id(mut self, id: impl Into<String>) -> Self {
        self.token_id = Some(id.into());
        self
    }

    /// Set the amount to withdraw.
    pub fn quant(mut self, quant: i64) -> Self {
        self.quant = Some(quant);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the withdraw transaction.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let exchange_id = self.exchange_id.ok_or(Error::missing_field("exchange_id"))?;
        let token_id = self.token_id.ok_or(Error::missing_field("token_id"))?;
        let quant = self.quant.ok_or(Error::missing_field("quant"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ExchangeWithdraw(ExchangeWithdrawContract {
                owner_address: owner,
                exchange_id,
                token_id,
                quant,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── ExchangeTradeBuilder ──────────────────────────────────────────────────────

/// Builds a swap (trade) transaction on an exchange pair.
///
/// Created by [`ExchangeApi::exchange_trade`].
pub struct ExchangeTradeBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    exchange_id: Option<i64>,
    token_id: Option<String>,
    quant: Option<i64>,
    expected: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> ExchangeTradeBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            exchange_id: None,
            token_id: None,
            quant: None,
            expected: None,
            memo: None,
        }
    }

    /// Override the trader address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the exchange ID.
    pub fn exchange_id(mut self, id: i64) -> Self {
        self.exchange_id = Some(id);
        self
    }

    /// Set the token ID being sold.
    pub fn token_id(mut self, id: impl Into<String>) -> Self {
        self.token_id = Some(id.into());
        self
    }

    /// Set the amount of the sell token to trade.
    pub fn quant(mut self, quant: i64) -> Self {
        self.quant = Some(quant);
        self
    }

    /// Set the minimum amount of the other token to receive (slippage protection).
    pub fn expected(mut self, expected: i64) -> Self {
        self.expected = Some(expected);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the trade transaction.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let exchange_id = self.exchange_id.ok_or(Error::missing_field("exchange_id"))?;
        let token_id = self.token_id.ok_or(Error::missing_field("token_id"))?;
        let quant = self.quant.ok_or(Error::missing_field("quant"))?;
        let expected = self.expected.ok_or(Error::missing_field("expected"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ExchangeTransaction(ExchangeTransactionContract {
                owner_address: owner,
                exchange_id,
                token_id,
                quant,
                expected,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

#[cfg(test)]
mod tests {
    use tronz_primitives::Address;

    use super::*;
    use crate::{provider::RootProvider, transport::mock::MockTransport, types::ExchangeInfo};

    fn mock_provider() -> RootProvider<MockTransport> {
        RootProvider::new(MockTransport::new())
    }

    fn exchange(id: i64) -> ExchangeInfo {
        ExchangeInfo {
            exchange_id: id,
            creator_address: Address::from_evm_bytes([0u8; 20]),
            create_time: 0,
            first_token_id: "_".into(),
            first_token_balance: 1_000_000,
            second_token_id: "1000001".into(),
            second_token_balance: 500_000,
        }
    }

    #[tokio::test]
    async fn list_exchanges_returns_pushed_list() {
        let provider = mock_provider();
        provider
            .transport()
            .push_ok::<Vec<ExchangeInfo>>("list_exchanges", vec![exchange(1), exchange(2)]);
        let result = provider.list_exchanges().await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].exchange_id, 1);
        assert_eq!(result[1].exchange_id, 2);
    }

    #[tokio::test]
    async fn get_exchange_by_id_found() {
        let provider = mock_provider();
        provider
            .transport()
            .push_ok::<Option<ExchangeInfo>>("get_exchange_by_id", Some(exchange(7)));
        let result = provider.get_exchange_by_id(7).await.unwrap();
        assert_eq!(result.unwrap().exchange_id, 7);
    }

    #[tokio::test]
    async fn get_exchange_by_id_not_found() {
        let provider = mock_provider();
        provider.transport().push_ok::<Option<ExchangeInfo>>("get_exchange_by_id", None);
        let result = provider.get_exchange_by_id(99).await.unwrap();
        assert!(result.is_none());
    }
}
