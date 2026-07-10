//! SR vote builder.

use tronz_primitives::Address;

use super::resolve_owner;
use crate::{
    error::Result,
    provider::{PendingTransaction, TronProvider},
    types::{ContractType, SrVote, TransactionRequest, VoteWitnessContract},
};

/// Builds a super-representative vote transaction.
///
/// Votes are weighted by TRON Power (1 TP = 1 TRX frozen via Stake 2.0).
/// Submitting an empty vote list clears all existing votes.
///
/// # Example
///
/// ```no_run
/// use tronz_provider::TronProvider as _;
/// # async fn run(provider: impl tronz_provider::TronProvider, sr: tronz_primitives::Address) -> tronz_provider::Result<()> {
/// let pending = provider
///     .vote_witness()
///     .vote(sr, 100)
///     .send()
///     .await?;
/// # Ok(()) }
/// ```
///
/// Created by [`TronProvider::vote_witness`].
pub struct VoteBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    votes: Vec<SrVote>,
    memo: Option<Vec<u8>>,
}

impl<'a, P: TronProvider> VoteBuilder<'a, P> {
    pub(crate) fn new(provider: &'a P) -> Self {
        Self { provider, owner: None, votes: Vec::new(), memo: None }
    }

    /// Override the voter address (defaults to the provider's signer).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Add a single SR vote entry.
    ///
    /// Call multiple times to vote for several SRs in one transaction.
    pub fn vote(mut self, sr_address: Address, count: i64) -> Self {
        self.votes.push(SrVote { vote_address: sr_address, vote_count: count });
        self
    }

    /// Append multiple SR votes at once.
    pub fn votes(mut self, votes: impl IntoIterator<Item = (Address, i64)>) -> Self {
        self.votes.extend(
            votes.into_iter().map(|(addr, count)| SrVote { vote_address: addr, vote_count: count }),
        );
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

        let req = TransactionRequest {
            contract: Some(ContractType::VoteWitness(VoteWitnessContract {
                owner_address: owner,
                votes: self.votes,
            })),
            memo: self.memo,
            ..Default::default()
        };
        self.provider.send_transaction(req).await
    }
}
