//! Super-representative (witness) API extension.
//!
//! Import [`WitnessApi`] to add SR management methods to any [`TronProvider`].

use tronz_primitives::Address;

use crate::{
    builders::resolve_owner,
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    transport::TronTransport as _,
    types::{
        ContractType, CreateWitnessContract, TransactionRequest, UpdateBrokerageContract,
        UpdateWitnessContract, WitnessInfo,
    },
};

/// Super-representative (witness) methods, available on any [`TronProvider`].
///
/// # Example
///
/// ```no_run
/// use tronz_provider::ext::WitnessApi;
/// # async fn run(provider: impl tronz_provider::TronProvider, sr: tronz_primitives::Address) -> tronz_provider::Result<()> {
/// // List all SRs sorted by vote count
/// let witnesses = provider.list_witnesses().await?;
///
/// // Query an SR's brokerage ratio
/// let brokerage = provider.get_brokerage(sr).await?;
///
/// // Apply to become an SR (costs 9,999 TRX deposit)
/// let pending = provider
///     .become_witness()
///     .url("https://my-sr.example.com")
///     .send()
///     .await?;
/// # Ok(()) }
/// ```
pub trait WitnessApi: TronProvider + Sized {
    /// List all super representatives and candidates.
    ///
    /// Returns the same list as [`TronProvider::list_witnesses`] — this
    /// method is re-exposed here for convenience when only `WitnessApi` is
    /// in scope.
    fn list_witnesses(&self) -> impl std::future::Future<Output = Result<Vec<WitnessInfo>>> + Send;

    /// Fetch the brokerage ratio (0–100) for a super representative.
    fn get_brokerage(
        &self,
        address: Address,
    ) -> impl std::future::Future<Output = Result<u64>> + Send;

    /// Fetch the unclaimed reward (in sun) for an address.
    fn get_reward_info(
        &self,
        address: Address,
    ) -> impl std::future::Future<Output = Result<u64>> + Send;

    /// Start building a become-SR-candidate transaction.
    fn become_witness(&self) -> BecomeWitnessBuilder<'_, Self>;

    /// Start building an update-SR-URL transaction.
    fn update_witness(&self) -> UpdateWitnessBuilder<'_, Self>;

    /// Start building a change-brokerage-ratio transaction.
    fn update_brokerage(&self) -> UpdateBrokerageBuilder<'_, Self>;
}

impl<P: TronProvider> WitnessApi for P {
    async fn list_witnesses(&self) -> Result<Vec<WitnessInfo>> {
        TronProvider::list_witnesses(self).await
    }

    async fn get_brokerage(&self, address: Address) -> Result<u64> {
        self.transport()
            .get_brokerage(address)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    async fn get_reward_info(&self, address: Address) -> Result<u64> {
        self.transport()
            .get_reward_info(address)
            .await
            .map_err(|e| Error::from(e.into()))
    }

    fn become_witness(&self) -> BecomeWitnessBuilder<'_, Self> {
        BecomeWitnessBuilder::new(self)
    }

    fn update_witness(&self) -> UpdateWitnessBuilder<'_, Self> {
        UpdateWitnessBuilder::new(self)
    }

    fn update_brokerage(&self) -> UpdateBrokerageBuilder<'_, Self> {
        UpdateBrokerageBuilder::new(self)
    }
}

// ── BecomeWitnessBuilder ──────────────────────────────────────────────────────

/// Builds a become-SR-candidate transaction.
///
/// The applicant must have at least 9,999 TRX to cover the SR deposit.
///
/// Created by [`WitnessApi::become_witness`].
pub struct BecomeWitnessBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    url: Option<String>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> BecomeWitnessBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            url: None,
            memo: None,
        }
    }

    /// Override the applicant address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the public SR information URL (required).
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let url = self.url.ok_or(Error::missing_field("url"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::CreateWitness(CreateWitnessContract {
                owner_address: owner,
                url,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── UpdateWitnessBuilder ──────────────────────────────────────────────────────

/// Builds an update-SR-URL transaction.
///
/// Created by [`WitnessApi::update_witness`].
pub struct UpdateWitnessBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    update_url: Option<String>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> UpdateWitnessBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            update_url: None,
            memo: None,
        }
    }

    /// Override the SR address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the new public SR URL (required).
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.update_url = Some(url.into());
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let update_url = self.update_url.ok_or(Error::missing_field("url"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UpdateWitness(UpdateWitnessContract {
                owner_address: owner,
                update_url,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── UpdateBrokerageBuilder ────────────────────────────────────────────────────

/// Builds a change-brokerage-ratio transaction.
///
/// Created by [`WitnessApi::update_brokerage`].
pub struct UpdateBrokerageBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    brokerage: Option<i32>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> UpdateBrokerageBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            brokerage: None,
            memo: None,
        }
    }

    /// Override the SR address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the new brokerage ratio (0–100, required).
    ///
    /// This is the percentage of block rewards the SR keeps. The remainder
    /// (100 − brokerage) is distributed proportionally to voters.
    pub fn brokerage(mut self, percent: i32) -> Self {
        self.brokerage = Some(percent);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Build, sign, and broadcast.
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let brokerage = self.brokerage.ok_or(Error::missing_field("brokerage"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::UpdateBrokerage(UpdateBrokerageContract {
                owner_address: owner,
                brokerage,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
