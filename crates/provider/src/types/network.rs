//! Network, node, chain query result types, and governance (proposal) types.

use std::collections::HashMap;

use tronz_primitives::Address;

/// Summary of network node info returned by [`crate::provider::TronProvider::get_node_info`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct NodeInfo {
    /// Latest full-node block string (e.g. `"Num:12345,ID:..."`).
    pub block: String,
    /// Latest solidity block string.
    pub solidity_block: String,
    /// Number of currently connected peers.
    pub peer_num: i32,
}

/// A gossip-network peer address.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct NodeAddress {
    /// Peer hostname or IP.
    pub host: String,
    /// Peer port.
    pub port: i32,
}

/// Selected fields from the chain's dynamic properties (head block info).
///
/// Returned by [`crate::provider::TronProvider::get_dynamic_properties`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ChainProperties {
    /// Head block id (hex string as returned by the node).
    pub head_block_id: String,
    /// Head block number.
    pub head_block_num: i64,
    /// Head block timestamp (unix ms).
    pub head_block_time_stamp: i64,
}

/// Bandwidth and energy net usage for an account.
///
/// Returned by [`crate::provider::TronProvider::get_account_net`].
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct AccountNet {
    /// Free bandwidth used.
    pub free_net_used: i64,
    /// Free bandwidth limit.
    pub free_net_limit: i64,
    /// Staked bandwidth used.
    pub net_used: i64,
    /// Staked bandwidth limit.
    pub net_limit: i64,
    /// Total network bandwidth weight (chain-wide).
    pub total_net_weight: i64,
    /// Energy used.
    pub energy_used: i64,
    /// Energy limit.
    pub energy_limit: i64,
    /// Total energy weight (chain-wide).
    pub total_energy_weight: i64,
}

/// State of an on-chain governance proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalState {
    /// Voting period is open.
    Pending,
    /// Proposal did not reach approval threshold.
    Disapproved,
    /// Proposal reached approval threshold and was applied.
    Approved,
    /// Proposal was canceled by the proposer.
    Canceled,
    /// Unknown state (future-proofing).
    Unknown(i32),
}

impl From<i32> for ProposalState {
    fn from(v: i32) -> Self {
        match v {
            0 => Self::Pending,
            1 => Self::Disapproved,
            2 => Self::Approved,
            3 => Self::Canceled,
            other => Self::Unknown(other),
        }
    }
}

/// An on-chain governance proposal.
///
/// Returned by governance query methods on [`GovernanceApi`](crate::ext::GovernanceApi).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ProposalInfo {
    /// Unique proposal ID.
    pub proposal_id: i64,
    /// Address of the account that submitted the proposal.
    pub proposer_address: Option<Address>,
    /// Proposed changes: chain parameter ID → new value.
    pub parameters: HashMap<i64, i64>,
    /// When the voting period closes (unix ms).
    pub expiration_time: i64,
    /// When the proposal was submitted (unix ms).
    pub create_time: i64,
    /// Addresses that have approved this proposal.
    pub approvals: Vec<Address>,
    /// Current state.
    pub state: ProposalState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_state_from_known_values() {
        assert_eq!(ProposalState::from(0), ProposalState::Pending);
        assert_eq!(ProposalState::from(1), ProposalState::Disapproved);
        assert_eq!(ProposalState::from(2), ProposalState::Approved);
        assert_eq!(ProposalState::from(3), ProposalState::Canceled);
    }

    #[test]
    fn proposal_state_unknown_preserved() {
        assert_eq!(ProposalState::from(99), ProposalState::Unknown(99));
        assert_eq!(ProposalState::from(-1), ProposalState::Unknown(-1));
    }
}

/// Multi-sig sign-weight query result.
///
/// Returned by [`crate::provider::TronProvider::get_transaction_sign_weight`].
#[derive(Clone, Debug)]
pub struct SignWeight {
    /// Addresses that have already signed.
    pub approved_list: Vec<Address>,
    /// Combined weight of all current signatures.
    pub current_weight: i64,
    /// Weight required to reach the threshold.
    pub required_weight: i64,
    /// Human-readable result message from the node.
    pub result: String,
}
