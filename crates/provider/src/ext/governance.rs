//! On-chain governance (proposal) API extension.
//!
//! Import [`GovernanceApi`] to add proposal methods to any [`TronProvider`].

use std::collections::HashMap;

use tronz_primitives::Address;

use crate::{
    builders::resolve_owner,
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    transport::TronTransport as _,
    types::{
        ContractType, ProposalApproveContract, ProposalCreateContract, ProposalDeleteContract,
        ProposalInfo, TransactionRequest,
    },
};

/// On-chain governance methods, available on any [`TronProvider`].
///
/// Only super representatives (SRs) and SR partners can submit or vote on
/// proposals. A proposal is applied if at least 15 of the 27 active SRs
/// approve it before the voting period ends (3 days by default).
///
/// # Example
///
/// ```no_run
/// use tronz_provider::ext::GovernanceApi;
/// # async fn run(provider: impl tronz_provider::TronProvider) -> tronz_provider::Result<()> {
/// // List all proposals
/// let proposals = provider.list_proposals().await?;
///
/// // Submit a proposal to change chain parameter 3 (max CPU time)
/// let pending = provider
///     .submit_proposal()
///     .parameter(3, 50)
///     .send()
///     .await?;
/// # Ok(()) }
/// ```
pub trait GovernanceApi: TronProvider + Sized {
    /// List all on-chain governance proposals.
    fn list_proposals(&self)
    -> impl std::future::Future<Output = Result<Vec<ProposalInfo>>> + Send;

    /// Fetch a paginated list of governance proposals.
    fn get_paginated_proposal_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> impl std::future::Future<Output = Result<Vec<ProposalInfo>>> + Send;

    /// Fetch a single proposal by its numeric ID.
    fn get_proposal_by_id(
        &self,
        proposal_id: i64,
    ) -> impl std::future::Future<Output = Result<ProposalInfo>> + Send;

    /// Start building a submit-proposal transaction.
    fn submit_proposal(&self) -> SubmitProposalBuilder<'_, Self>;

    /// Start building an approve/disapprove-proposal transaction.
    fn approve_proposal(&self) -> ApproveProposalBuilder<'_, Self>;

    /// Start building a cancel-proposal transaction.
    fn cancel_proposal(&self) -> CancelProposalBuilder<'_, Self>;
}

impl<P: TronProvider> GovernanceApi for P {
    async fn list_proposals(&self) -> Result<Vec<ProposalInfo>> {
        self.transport()
            .list_proposals()
            .await
            .map_err(|e| Error::Transport(e.into()))
    }

    async fn get_paginated_proposal_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ProposalInfo>> {
        self.transport()
            .get_paginated_proposal_list(offset, limit)
            .await
            .map_err(|e| Error::Transport(e.into()))
    }

    async fn get_proposal_by_id(&self, proposal_id: i64) -> Result<ProposalInfo> {
        self.transport()
            .get_proposal_by_id(proposal_id)
            .await
            .map_err(|e| Error::Transport(e.into()))
    }

    fn submit_proposal(&self) -> SubmitProposalBuilder<'_, Self> {
        SubmitProposalBuilder::new(self)
    }

    fn approve_proposal(&self) -> ApproveProposalBuilder<'_, Self> {
        ApproveProposalBuilder::new(self)
    }

    fn cancel_proposal(&self) -> CancelProposalBuilder<'_, Self> {
        CancelProposalBuilder::new(self)
    }
}

// ── SubmitProposalBuilder ─────────────────────────────────────────────────────

/// Builds a governance proposal transaction.
///
/// Created by [`GovernanceApi::submit_proposal`].
pub struct SubmitProposalBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    parameters: HashMap<i64, i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> SubmitProposalBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            parameters: HashMap::new(),
            memo: None,
        }
    }

    /// Override the proposer address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Add a single chain-parameter change to this proposal.
    ///
    /// `param_id` is the numeric ID of the chain parameter (see TRON docs).
    pub fn parameter(mut self, param_id: i64, value: i64) -> Self {
        self.parameters.insert(param_id, value);
        self
    }

    /// Set all parameters at once.
    pub fn parameters(mut self, params: impl IntoIterator<Item = (i64, i64)>) -> Self {
        self.parameters = params.into_iter().collect();
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
        if self.parameters.is_empty() {
            return Err(Error::missing_field("parameters"));
        }

        let req = TransactionRequest {
            contract: Some(ContractType::ProposalCreate(ProposalCreateContract {
                owner_address: owner,
                parameters: self.parameters,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── ApproveProposalBuilder ────────────────────────────────────────────────────

/// Builds an approve/disapprove proposal transaction.
///
/// Created by [`GovernanceApi::approve_proposal`].
pub struct ApproveProposalBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    proposal_id: Option<i64>,
    is_add_approval: bool,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> ApproveProposalBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            proposal_id: None,
            is_add_approval: true,
            memo: None,
        }
    }

    /// Override the voter address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the proposal ID to vote on (required).
    pub fn proposal_id(mut self, id: i64) -> Self {
        self.proposal_id = Some(id);
        self
    }

    /// Set whether this is an approval (`true`) or revocation (`false`).
    ///
    /// Defaults to `true` (add approval).
    pub fn approve(mut self, approve: bool) -> Self {
        self.is_add_approval = approve;
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
        let proposal_id = self
            .proposal_id
            .ok_or(Error::missing_field("proposal_id"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ProposalApprove(ProposalApproveContract {
                owner_address: owner,
                proposal_id,
                is_add_approval: self.is_add_approval,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}

// ── CancelProposalBuilder ─────────────────────────────────────────────────────

/// Builds a cancel-proposal transaction.
///
/// Created by [`GovernanceApi::cancel_proposal`].
pub struct CancelProposalBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    proposal_id: Option<i64>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> CancelProposalBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self {
            provider,
            owner: None,
            proposal_id: None,
            memo: None,
        }
    }

    /// Override the proposer address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the proposal ID to cancel (required).
    pub fn proposal_id(mut self, id: i64) -> Self {
        self.proposal_id = Some(id);
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
        let proposal_id = self
            .proposal_id
            .ok_or(Error::missing_field("proposal_id"))?;

        let req = TransactionRequest {
            contract: Some(ContractType::ProposalDelete(ProposalDeleteContract {
                owner_address: owner,
                proposal_id,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
