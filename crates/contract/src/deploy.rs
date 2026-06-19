//! [`DeployBuilder`] — a lazy smart-contract deployment transaction.
//!
//! Mirrors the role of alloy's `ContractDeployer`: wraps a
//! [`CreateSmartContract`] request with a fluent builder API.
//!
//! ## Two deployment paths
//!
//! | Method | Returns | Use when |
//! |--------|---------|----------|
//! | [`send`](DeployBuilder::send) | `PendingTransaction` | you want the tx id or a custom poll loop |
//! | [`deploy`](DeployBuilder::deploy) | `Address` | you just need the deployed address |
//!
//! # Example
//!
//! ```no_run
//! use tronz_contract::ContractExt as _;
//!
//! # async fn run(
//! #     provider: impl tronz_provider::TronProvider,
//! #     bytecode: tronz_primitives::Bytes,
//! #     abi: &[u8],
//! # ) -> tronz_contract::Result<()> {
//! // One-shot: broadcast + wait + return address
//! let address = provider
//!     .deploy(bytecode.clone())
//!     .abi(abi)
//!     .name("MyToken")
//!     .deploy()
//!     .await?;
//!
//! // Or split into broadcast + poll separately:
//! let pending = provider
//!     .deploy(bytecode)
//!     .abi(abi)
//!     .send()
//!     .await?;
//! let info = pending.get_receipt().await?;
//! let contract_address = info.contract_address;
//! # Ok(()) }
//! ```

use tronz_primitives::{Address, Bytes, Trx};
use tronz_provider::{
    Error as ProviderError, PendingTransaction, TronProvider,
    types::{ContractType, CreateSmartContract, TransactionRequest},
};

use crate::error::{ContractError, Result};

/// A builder for deploying a TRON smart contract.
///
/// Created by [`ContractExt::deploy`](crate::instance::ContractExt::deploy).
pub struct DeployBuilder<P> {
    provider: P,
    bytecode: Bytes,
    abi: Vec<u8>,
    call_value: Trx,
    fee_limit: Option<Trx>,
    consume_user_resource_percent: i64,
    origin_energy_limit: i64,
    name: String,
}

impl<P: TronProvider> DeployBuilder<P> {
    pub(crate) fn new(provider: P, bytecode: impl Into<Bytes>) -> Self {
        Self {
            provider,
            bytecode: bytecode.into(),
            abi: Vec::new(),
            call_value: Trx::ZERO,
            fee_limit: None,
            consume_user_resource_percent: 100,
            origin_energy_limit: 10_000_000,
            name: String::new(),
        }
    }

    /// Attach the contract's JSON ABI (used by TRON nodes to validate the
    /// deployment and store ABI metadata on-chain).
    #[inline]
    pub fn abi(mut self, abi: impl Into<Vec<u8>>) -> Self {
        self.abi = abi.into();
        self
    }

    /// Send TRX to the contract constructor (for payable constructors).
    #[inline]
    pub fn value(mut self, trx: Trx) -> Self {
        self.call_value = trx;
        self
    }

    /// Override the energy fee limit for this deployment.
    ///
    /// If not set, the provider's default `fee_limit` (from [`FeeLimitFiller`])
    /// is used. Large contracts may require a higher limit.
    ///
    /// [`FeeLimitFiller`]: tronz_provider::fillers::FeeLimitFiller
    #[inline]
    pub fn fee_limit(mut self, limit: Trx) -> Self {
        self.fee_limit = Some(limit);
        self
    }

    /// Set a human-readable name stored on-chain with the contract.
    #[inline]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Percentage of energy paid by the *caller* (0–100; default: `100`).
    ///
    /// The remainder is charged to the contract origin. A value of `0` means
    /// the contract absorbs all energy costs.
    #[inline]
    pub fn consume_user_resource_percent(mut self, pct: i64) -> Self {
        self.consume_user_resource_percent = pct;
        self
    }

    /// Maximum energy the origin account pays per call (default: `10_000_000`).
    #[inline]
    pub fn origin_energy_limit(mut self, limit: i64) -> Self {
        self.origin_energy_limit = limit;
        self
    }

    /// Build, sign, broadcast, wait for confirmation, and return the deployed
    /// contract address.
    ///
    /// Returns [`ContractError::ContractNotDeployed`] if the transaction was
    /// confirmed but `contract_address` was absent from the receipt (e.g. the
    /// deployment transaction reverted or the node rejected it).
    ///
    /// For a lower-level path that gives you the [`PendingTransaction`] handle,
    /// use [`send`](Self::send) instead.
    pub async fn deploy(self) -> Result<Address> {
        let pending = self.send().await?;
        // `?` uses `From<PendingTransactionError> for ContractError`:
        // Transport errors → ContractError::Provider,
        // timeout → ContractError::ConfirmationTimeout.
        let info = pending.get_receipt().await?;
        info.contract_address
            .ok_or(ContractError::ContractNotDeployed)
    }

    /// Build, sign, and broadcast the deployment transaction.
    ///
    /// The returned [`PendingTransaction`] can be awaited with
    /// [`get_receipt`](tronz_provider::PendingTransaction::get_receipt).
    /// The deployed contract address is in [`TransactionInfo::contract_address`].
    ///
    /// [`TransactionInfo::contract_address`]: tronz_provider::types::TransactionInfo::contract_address
    pub async fn send(self) -> Result<PendingTransaction<P>> {
        let owner = self
            .provider
            .signer_address()
            .ok_or_else(ProviderError::no_signer)
            .map_err(ContractError::Provider)?;

        let mut req = TransactionRequest::default().with_contract(
            ContractType::CreateSmartContract(CreateSmartContract {
                owner_address: owner,
                bytecode: self.bytecode,
                abi: self.abi,
                call_value: self.call_value,
                consume_user_resource_percent: self.consume_user_resource_percent,
                origin_energy_limit: self.origin_energy_limit,
                name: self.name,
            }),
        );

        if let Some(limit) = self.fee_limit {
            req = req.with_fee_limit(limit);
        }

        self.provider
            .send_transaction(req)
            .await
            .map_err(ContractError::Provider)
    }
}
