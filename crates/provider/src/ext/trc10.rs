//! TRC10 (native TRON token) API extension.
//!
//! Import [`Trc10Api`] to add TRC10 methods to any [`TronProvider`].

use tronz_primitives::Address;

use crate::{
    builders::resolve_owner,
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    transport::TronTransport as _,
    types::{
        AssetInfo, AssetIssueContract, ContractType, FrozenSupply, ParticipateAssetIssueContract,
        TransactionRequest, TransferAssetContract, UnfreezeAssetContract, UpdateAssetContract,
    },
};

/// TRC10 native token methods, available on any [`TronProvider`].
///
/// # Example
///
/// ```no_run
/// use tronz_provider::ext::Trc10Api;
/// # async fn run(provider: impl tronz_provider::TronProvider, recipient: tronz_primitives::Address) -> tronz_provider::Result<()> {
/// // Query token metadata
/// let info = provider.get_asset_info("1000001").await?.expect("token exists");
/// println!("{} ({}), decimals={}", info.name, info.abbr, info.decimals);
///
/// // Check a TRC10 balance (reads from get_account)
/// let balance = provider.trc10_balance(recipient, "1000001").await?;
///
/// // Transfer tokens
/// let pending = provider
///     .transfer_trc10()
///     .to(recipient)
///     .token_id("1000001")
///     .amount(1_000_000)
///     .send()
///     .await?;
/// # Ok(()) }
/// ```
pub trait Trc10Api: TronProvider + Sized {
    /// Fetch metadata for a TRC10 token by its numeric ID (e.g. `"1000001"`).
    ///
    /// Returns `None` if no token with that ID exists.
    fn get_asset_info(
        &self,
        token_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<AssetInfo>>> + Send;

    /// Return the raw TRC10 balance of `address` for `token_id`.
    ///
    /// Internally calls [`get_account`](TronProvider::get_account) and
    /// extracts the balance from `trc10_balances`. Returns `0` if the account
    /// holds none of the token.
    fn trc10_balance(
        &self,
        address: Address,
        token_id: &str,
    ) -> impl std::future::Future<Output = Result<i64>> + Send;

    /// Fetch all TRC10 tokens issued by `address`.
    fn get_asset_issue_by_account(
        &self,
        address: Address,
    ) -> impl std::future::Future<Output = Result<Vec<AssetInfo>>> + Send;

    /// Fetch a paginated list of all TRC10 tokens on-chain.
    ///
    /// `offset` is the token index to start from (0-based); `limit` is the
    /// maximum number of tokens to return.
    fn get_asset_issue_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> impl std::future::Future<Output = Result<Vec<AssetInfo>>> + Send;

    /// Fetch a TRC10 token by name.
    ///
    /// Returns `None` if no token with that name exists.
    ///
    /// Token names are not unique after the `ALLOW_SAME_TOKEN_NAME` proposal.
    /// Use [`get_asset_issue_list_by_name`](Trc10Api::get_asset_issue_list_by_name) when
    /// multiple tokens may share the same name.
    fn get_asset_issue_by_name(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Option<AssetInfo>>> + Send;

    /// Fetch all TRC10 tokens with a given name.
    fn get_asset_issue_list_by_name(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Vec<AssetInfo>>> + Send;

    /// Start building a TRC10 token transfer.
    fn transfer_trc10(&self) -> TransferTrc10Builder<'_, Self>;

    /// Start building a TRC10 token issuance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use tronz_provider::ext::Trc10Api;
    /// # async fn run(provider: impl tronz_provider::TronProvider) -> tronz_provider::Result<()> {
    /// let pending = provider
    ///     .issue_trc10()
    ///     .name("MyToken")
    ///     .abbr("MTK")
    ///     .description("A test token")
    ///     .url("https://example.com")
    ///     .total_supply(1_000_000_000)
    ///     .precision(6)
    ///     .send()
    ///     .await?;
    /// # Ok(()) }
    /// ```
    fn issue_trc10(&self) -> IssueTrc10Builder<'_, Self>;

    /// Start building a TRC10 ICO participation (buy tokens with TRX).
    fn participate_trc10(&self) -> ParticipateTrc10Builder<'_, Self>;

    /// Start building a frozen-supply release for a TRC10 token.
    fn unfreeze_trc10(&self) -> UnfreezeTrc10Builder<'_, Self>;

    /// Start building a TRC10 token metadata update.
    fn update_trc10(&self) -> UpdateTrc10Builder<'_, Self>;
}

impl<P: TronProvider> Trc10Api for P {
    async fn get_asset_info(&self, token_id: &str) -> Result<Option<AssetInfo>> {
        self.transport()
            .get_asset_issue_by_id(token_id)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_asset_issue_by_account(&self, address: Address) -> Result<Vec<AssetInfo>> {
        self.transport()
            .get_asset_issue_by_account(address)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_asset_issue_list(&self, offset: i64, limit: i64) -> Result<Vec<AssetInfo>> {
        self.transport()
            .get_paginated_asset_issue_list(offset, limit)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn trc10_balance(&self, address: Address, token_id: &str) -> Result<i64> {
        let account = self.get_account(address).await?;
        Ok(account.trc10_balances.get(token_id).copied().unwrap_or(0))
    }

    async fn get_asset_issue_by_name(&self, name: &str) -> Result<Option<AssetInfo>> {
        self.transport()
            .get_asset_issue_by_name(name)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_asset_issue_list_by_name(&self, name: &str) -> Result<Vec<AssetInfo>> {
        self.transport()
            .get_asset_issue_list_by_name(name)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    fn transfer_trc10(&self) -> TransferTrc10Builder<'_, Self> {
        TransferTrc10Builder::new(self)
    }

    fn issue_trc10(&self) -> IssueTrc10Builder<'_, Self> {
        IssueTrc10Builder::new(self)
    }

    fn participate_trc10(&self) -> ParticipateTrc10Builder<'_, Self> {
        ParticipateTrc10Builder::new(self)
    }

    fn unfreeze_trc10(&self) -> UnfreezeTrc10Builder<'_, Self> {
        UnfreezeTrc10Builder::new(self)
    }

    fn update_trc10(&self) -> UpdateTrc10Builder<'_, Self> {
        UpdateTrc10Builder::new(self)
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

/// Builds a TRC10 token transfer.
///
/// Created by [`Trc10Api::transfer_trc10`].
pub struct TransferTrc10Builder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    to: Option<Address>,
    token_id: Option<String>,
    amount: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> TransferTrc10Builder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            to: None,
            token_id: None,
            amount: None,
            memo: None,
        }
    }

    /// Override the sender (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the recipient address.
    pub fn to(mut self, to: Address) -> Self {
        self.to = Some(to);
        self
    }

    /// Set the numeric token ID (e.g. `"1000001"`).
    pub fn token_id(mut self, id: impl Into<String>) -> Self {
        self.token_id = Some(id.into());
        self
    }

    /// Set the amount in the token's smallest unit.
    pub fn amount(mut self, amount: i64) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the transfer.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let to = self.to.ok_or(Error::missing_field("to"))?;
        let token_id = self.token_id.ok_or(Error::missing_field("token_id"))?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::TransferAsset(TransferAssetContract {
                owner_address: owner,
                to_address: to,
                token_id,
                amount,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── IssueTrc10Builder ─────────────────────────────────────────────────────────

/// Builds a TRC10 token issuance transaction.
///
/// Created by [`Trc10Api::issue_trc10`].
pub struct IssueTrc10Builder<'a, P> {
    provider: &'a P,
    owner: Option<tronz_primitives::Address>,
    name: Option<String>,
    abbr: Option<String>,
    description: String,
    url: Option<String>,
    total_supply: Option<i64>,
    precision: i32,
    trx_num: i32,
    num: i32,
    /// ICO start offset from now in milliseconds (default: 5 minutes).
    start_offset_ms: i64,
    /// ICO duration in milliseconds after start (default: 30 days).
    duration_ms: i64,
    free_asset_net_limit: i64,
    public_free_asset_net_limit: i64,
    frozen_supply: Vec<FrozenSupply>,
}

impl<'a, P: TronProvider> IssueTrc10Builder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            name: None,
            abbr: None,
            description: String::new(),
            url: None,
            total_supply: None,
            precision: 0,
            trx_num: 1,
            num: 1,
            start_offset_ms: 5 * 60 * 1_000, // 5 minutes from now
            duration_ms: 30 * 24 * 60 * 60 * 1_000, // 30 days
            free_asset_net_limit: 0,
            public_free_asset_net_limit: 0,
            frozen_supply: vec![],
        }
    }

    /// Override the issuer address (defaults to the provider's signer address).
    pub fn from(mut self, from: tronz_primitives::Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the full token name (required).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the token abbreviation / symbol (required).
    pub fn abbr(mut self, abbr: impl Into<String>) -> Self {
        self.abbr = Some(abbr.into());
        self
    }

    /// Set a human-readable description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the project URL (required).
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Set the total supply in the token's smallest unit (required).
    pub fn total_supply(mut self, supply: i64) -> Self {
        self.total_supply = Some(supply);
        self
    }

    /// Set the decimal precision (0–6, default: 0).
    pub fn precision(mut self, precision: i32) -> Self {
        self.precision = precision;
        self
    }

    /// Set the ICO exchange rate as `trx_num` TRX units per `num` tokens.
    ///
    /// Defaults to `1:1` (1 TRX = 1 token). Pass `trx_num=1, num=1000` for
    /// 1 TRX = 1000 tokens.
    pub fn exchange_rate(mut self, trx_num: i32, num: i32) -> Self {
        self.trx_num = trx_num;
        self.num = num;
        self
    }

    /// Set how far in the future the ICO starts (default: 5 minutes).
    pub fn start_offset_ms(mut self, ms: i64) -> Self {
        self.start_offset_ms = ms;
        self
    }

    /// Set the ICO duration in milliseconds after the start (default: 30 days).
    pub fn duration_ms(mut self, ms: i64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Free bandwidth limit per account for token transfers.
    pub fn free_asset_net_limit(mut self, limit: i64) -> Self {
        self.free_asset_net_limit = limit;
        self
    }

    /// Total free bandwidth limit across all token transfers.
    pub fn public_free_asset_net_limit(mut self, limit: i64) -> Self {
        self.public_free_asset_net_limit = limit;
        self
    }

    /// Lock a portion of the supply for `days` days.
    pub fn freeze(mut self, amount: i64, days: i64) -> Self {
        self.frozen_supply.push(FrozenSupply {
            frozen_amount: amount,
            frozen_days: days,
        });
        self
    }

    /// Build, sign, and broadcast the token issuance.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let name = self.name.ok_or(Error::missing_field("name"))?;
        let abbr = self.abbr.ok_or(Error::missing_field("abbr"))?;
        let url = self.url.ok_or(Error::missing_field("url"))?;
        let total_supply = self
            .total_supply
            .ok_or(Error::missing_field("total_supply"))?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let start_time = now_ms + self.start_offset_ms;
        let end_time = start_time + self.duration_ms;

        let req = TransactionRequest {
            contract: Some(ContractType::AssetIssue(AssetIssueContract {
                owner_address: owner,
                name,
                abbr,
                description: self.description,
                url,
                total_supply,
                precision: self.precision,
                trx_num: self.trx_num,
                num: self.num,
                start_time,
                end_time,
                free_asset_net_limit: self.free_asset_net_limit,
                public_free_asset_net_limit: self.public_free_asset_net_limit,
                frozen_supply: self.frozen_supply,
            })),
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── ParticipateTrc10Builder ───────────────────────────────────────────────────

/// Builds a TRC10 ICO participation transaction.
///
/// Created by [`Trc10Api::participate_trc10`].
pub struct ParticipateTrc10Builder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    to: Option<Address>,
    token_id: Option<String>,
    amount: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> ParticipateTrc10Builder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            to: None,
            token_id: None,
            amount: None,
            memo: None,
        }
    }

    /// Override the buyer address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the issuer / ICO address.
    pub fn to(mut self, to: Address) -> Self {
        self.to = Some(to);
        self
    }

    /// Set the numeric token ID (e.g. `"1000001"`).
    pub fn token_id(mut self, id: impl Into<String>) -> Self {
        self.token_id = Some(id.into());
        self
    }

    /// Set the amount of TRX in sun to spend.
    pub fn amount_sun(mut self, sun: i64) -> Self {
        self.amount = Some(sun);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the participation.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let to = self.to.ok_or(Error::missing_field("to"))?;
        let token_id = self.token_id.ok_or(Error::missing_field("token_id"))?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ParticipateAssetIssue(
                ParticipateAssetIssueContract {
                    owner_address: owner,
                    to_address: to,
                    token_id,
                    amount,
                },
            )),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── UnfreezeTrc10Builder ──────────────────────────────────────────────────────

/// Builds an unfreeze-asset transaction (releases frozen TRC10 supply).
///
/// Created by [`Trc10Api::unfreeze_trc10`].
pub struct UnfreezeTrc10Builder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> UnfreezeTrc10Builder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            memo: None,
        }
    }

    /// Override the issuer address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the unfreeze.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;

        let req = TransactionRequest {
            contract: Some(ContractType::UnfreezeAsset(UnfreezeAssetContract {
                owner_address: owner,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── UpdateTrc10Builder ────────────────────────────────────────────────────────

/// Builds a TRC10 token metadata update transaction.
///
/// Created by [`Trc10Api::update_trc10`].
pub struct UpdateTrc10Builder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    description: String,
    url: Option<String>,
    new_limit: i64,
    new_public_limit: i64,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> UpdateTrc10Builder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            description: String::new(),
            url: None,
            new_limit: 0,
            new_public_limit: 0,
            memo: None,
        }
    }

    /// Override the issuer address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the new token description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the new project URL (required).
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Set the new per-account free-transfer bandwidth limit.
    pub fn new_limit(mut self, limit: i64) -> Self {
        self.new_limit = limit;
        self
    }

    /// Set the new total free-transfer bandwidth limit.
    pub fn new_public_limit(mut self, limit: i64) -> Self {
        self.new_public_limit = limit;
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast the metadata update.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let url = self.url.ok_or(Error::missing_field("url"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UpdateAsset(UpdateAssetContract {
                owner_address: owner,
                description: self.description,
                url,
                new_limit: self.new_limit,
                new_public_limit: self.new_public_limit,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
