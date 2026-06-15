//! TRON native contract types and their parameter structs.

use tronz_primitives::{Address, Bytes, ResourceCode, Trx};

/// All TRON native contract types. Discriminants mirror the protobuf
/// `Transaction.Contract.ContractType` enum.
///
/// Only the `v0` variants carry fully-defined parameter structs today; the
/// remaining variants are reserved for later milestones.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ContractType {
    // --- v0 ---
    /// Transfer TRX.
    Transfer(TransferContract),
    /// Call/trigger a smart contract.
    TriggerSmartContract(TriggerSmartContract),
    /// Stake TRX for a resource (Stake 2.0).
    FreezeBalanceV2(FreezeBalanceV2Contract),
    /// Unstake TRX (Stake 2.0).
    UnfreezeBalanceV2(UnfreezeBalanceV2Contract),
    /// Delegate a resource to another account.
    DelegateResource(DelegateResourceContract),
    /// Reclaim a delegated resource.
    UnDelegateResource(UnDelegateResourceContract),
    /// Withdraw TRX from expired unfreeze windows.
    WithdrawExpireUnfreeze(WithdrawExpireUnfreezeContract),
    /// Cancel all in-progress unfreeze operations.
    CancelAllUnfreezeV2(CancelAllUnfreezeV2Contract),
    /// Claim accumulated block/vote rewards.
    WithdrawBalance(WithdrawBalanceContract),
    /// Update account permissions (multisig).
    AccountPermissionUpdate(AccountPermissionUpdateContract),
    /// Deploy a new smart contract.
    CreateSmartContract(CreateSmartContract),
    /// Issue (create) a new TRC10 native token.
    AssetIssue(AssetIssueContract),
    /// Transfer a TRC10 token.
    TransferAsset(TransferAssetContract),
    /// Activate a new account by sending TRX to it.
    CreateAccount(CreateAccountContract),
    /// Vote for super representatives.
    VoteWitness(VoteWitnessContract),
    /// Update account name.
    UpdateAccount(UpdateAccountContract),
}

impl ContractType {
    /// Whether this contract type requires a `fee_limit` to be set
    /// (smart-contract operations) versus native contracts that ignore it.
    pub fn needs_fee_limit(&self) -> bool {
        matches!(
            self,
            ContractType::TriggerSmartContract(_) | ContractType::CreateSmartContract(_)
        )
    }

    /// The owner (sender) address of this contract operation.
    pub fn owner_address(&self) -> Address {
        match self {
            ContractType::Transfer(c) => c.owner_address,
            ContractType::TriggerSmartContract(c) => c.owner_address,
            ContractType::FreezeBalanceV2(c) => c.owner_address,
            ContractType::UnfreezeBalanceV2(c) => c.owner_address,
            ContractType::DelegateResource(c) => c.owner_address,
            ContractType::UnDelegateResource(c) => c.owner_address,
            ContractType::WithdrawExpireUnfreeze(c) => c.owner_address,
            ContractType::CancelAllUnfreezeV2(c) => c.owner_address,
            ContractType::WithdrawBalance(c) => c.owner_address,
            ContractType::AccountPermissionUpdate(c) => c.owner_address,
            ContractType::CreateSmartContract(c) => c.owner_address,
            ContractType::AssetIssue(c) => c.owner_address,
            ContractType::TransferAsset(c) => c.owner_address,
            ContractType::CreateAccount(c) => c.owner_address,
            ContractType::VoteWitness(c) => c.owner_address,
            ContractType::UpdateAccount(c) => c.owner_address,
        }
    }
}

/// Transfer TRX from one account to another.
#[derive(Clone, Debug)]
pub struct TransferContract {
    /// Sender address.
    pub owner_address: Address,
    /// Recipient address.
    pub to_address: Address,
    /// Amount to transfer.
    pub amount: Trx,
}

/// Call or trigger a smart contract.
#[derive(Clone, Debug)]
pub struct TriggerSmartContract {
    /// Caller address.
    pub owner_address: Address,
    /// Target contract address.
    pub contract_address: Address,
    /// TRX sent along with the call.
    pub call_value: Trx,
    /// ABI-encoded selector + arguments.
    pub data: Bytes,
    /// TRC10 token value sent with the call.
    pub call_token_value: Trx,
    /// TRC10 token id sent with the call.
    pub token_id: i64,
}

/// Stake TRX for energy or bandwidth (Stake 2.0).
#[derive(Clone, Debug)]
pub struct FreezeBalanceV2Contract {
    /// Account staking the TRX.
    pub owner_address: Address,
    /// Amount of TRX to stake.
    pub frozen_balance: Trx,
    /// Resource to obtain.
    pub resource: ResourceCode,
}

/// Unstake TRX (Stake 2.0).
#[derive(Clone, Debug)]
pub struct UnfreezeBalanceV2Contract {
    /// Account unstaking the TRX.
    pub owner_address: Address,
    /// Amount of TRX to unstake.
    pub unfreeze_balance: Trx,
    /// Resource being released.
    pub resource: ResourceCode,
}

/// Delegate staked energy or bandwidth to another account.
#[derive(Clone, Debug)]
pub struct DelegateResourceContract {
    /// Delegator address.
    pub owner_address: Address,
    /// Resource being delegated.
    pub resource: ResourceCode,
    /// Amount of staked TRX whose resource is delegated.
    pub balance: Trx,
    /// Recipient of the delegation.
    pub receiver_address: Address,
    /// Optional lock period in seconds (`None` = no lock).
    pub lock_period: Option<i64>,
}

/// Reclaim delegated resources.
#[derive(Clone, Debug)]
pub struct UnDelegateResourceContract {
    /// Delegator address.
    pub owner_address: Address,
    /// Resource being reclaimed.
    pub resource: ResourceCode,
    /// Amount of staked TRX whose resource is reclaimed.
    pub balance: Trx,
    /// Account the delegation was made to.
    pub receiver_address: Address,
}

/// Withdraw TRX from expired unfreeze windows.
#[derive(Clone, Debug)]
pub struct WithdrawExpireUnfreezeContract {
    /// Account withdrawing.
    pub owner_address: Address,
}

/// Cancel all in-progress unfreeze operations.
#[derive(Clone, Debug)]
pub struct CancelAllUnfreezeV2Contract {
    /// Account cancelling.
    pub owner_address: Address,
}

/// Claim accumulated block/vote rewards.
#[derive(Clone, Debug)]
pub struct WithdrawBalanceContract {
    /// Account claiming rewards.
    pub owner_address: Address,
}

/// Update account permissions (multisig configuration).
#[derive(Clone, Debug)]
pub struct AccountPermissionUpdateContract {
    /// Account being updated.
    pub owner_address: Address,
    /// New owner permission.
    pub owner: Option<Permission>,
    /// New witness permission (for super representatives).
    pub witness: Option<Permission>,
    /// New active permissions.
    pub actives: Vec<Permission>,
}

/// Deploy a new smart contract.
#[derive(Clone, Debug)]
pub struct CreateSmartContract {
    /// Deployer address.
    pub owner_address: Address,
    /// Contract bytecode.
    pub bytecode: Bytes,
    /// JSON-encoded ABI.
    pub abi: Vec<u8>,
    /// TRX sent on deployment.
    pub call_value: Trx,
    /// Percentage of energy the caller (vs origin) pays.
    pub consume_user_resource_percent: i64,
    /// Per-call energy cap charged to the contract origin.
    pub origin_energy_limit: i64,
    /// Contract name.
    pub name: String,
}

/// Issue (create) a new TRC10 native token.
///
/// After submission the token receives a numeric ID assigned by the network.
/// Query it via
/// [`Trc10Api::get_asset_issue_by_account`](crate::ext::Trc10Api::get_asset_issue_by_account).
#[derive(Clone, Debug)]
pub struct AssetIssueContract {
    /// Issuer address.
    pub owner_address: Address,
    /// Full token name (e.g. `"MyToken"`).
    pub name: String,
    /// Token abbreviation / symbol (e.g. `"MTK"`).
    pub abbr: String,
    /// Human-readable description.
    pub description: String,
    /// Project URL.
    pub url: String,
    /// Total supply in the token's smallest unit.
    pub total_supply: i64,
    /// Decimal precision (0–6).
    pub precision: i32,
    /// Exchange rate denominator: how many TRX units correspond to `num` tokens.
    ///
    /// Together `trx_num / num` defines the ICO exchange rate.
    /// Set both to `1` for a 1 TRX = 1 token rate.
    pub trx_num: i32,
    /// Exchange rate numerator: number of tokens per `trx_num` TRX units.
    pub num: i32,
    /// ICO start time in Unix milliseconds (must be in the future).
    pub start_time: i64,
    /// ICO end time in Unix milliseconds (must be after `start_time`).
    pub end_time: i64,
    /// Free bandwidth each account can use for token transfers (per-account limit).
    pub free_asset_net_limit: i64,
    /// Total free bandwidth available across all token transfers.
    pub public_free_asset_net_limit: i64,
    /// Portions of the supply that are locked for a number of days.
    pub frozen_supply: Vec<FrozenSupply>,
}

/// A portion of a TRC10 token supply locked for a fixed period.
#[derive(Clone, Debug)]
pub struct FrozenSupply {
    /// Amount locked (in the token's smallest unit).
    pub frozen_amount: i64,
    /// Lock duration in days.
    pub frozen_days: i64,
}

/// Transfer a TRC10 (native) token.
#[derive(Clone, Debug)]
pub struct TransferAssetContract {
    /// Sender address.
    pub owner_address: Address,
    /// Recipient address.
    pub to_address: Address,
    /// Numeric token ID as a string (e.g. `"1000001"`).
    pub token_id: String,
    /// Amount in the token's smallest unit.
    pub amount: i64,
}

/// Activate a new account by sending TRX to it.
///
/// On TRON, accounts that have never received funds do not exist on-chain.
/// Sending this contract creates the account and transfers a small amount of
/// TRX in one atomic operation.
#[derive(Clone, Debug)]
pub struct CreateAccountContract {
    /// Payer / creator address.
    pub owner_address: Address,
    /// Address of the account to activate.
    pub account_address: Address,
}

/// Vote for super representatives.
///
/// Votes are weighted by TRON Power (1 TP = 1 frozen TRX).
/// Submitting an empty `votes` list clears all existing votes.
#[derive(Clone, Debug)]
pub struct VoteWitnessContract {
    /// Voter address.
    pub owner_address: Address,
    /// SR addresses and vote counts.
    pub votes: Vec<SrVote>,
}

/// A single SR vote entry inside [`VoteWitnessContract`].
#[derive(Clone, Debug)]
pub struct SrVote {
    /// Super representative candidate address.
    pub vote_address: Address,
    /// Number of votes to cast.
    pub vote_count: i64,
}

/// Update an account's on-chain name.
///
/// Account names are not unique on TRON and can be changed freely.
#[derive(Clone, Debug)]
pub struct UpdateAccountContract {
    /// Account being renamed.
    pub owner_address: Address,
    /// New name (UTF-8).
    pub name: String,
}

/// A single account permission entry (multisig).
#[derive(Clone, Debug)]
pub struct Permission {
    /// Permission id (`0` = owner, `2+` = active).
    pub id: i32,
    /// Human-readable permission name.
    pub permission_name: String,
    /// Signature-weight threshold required to authorize an operation.
    pub threshold: i64,
    /// Keys and their weights.
    pub keys: Vec<PermissionKey>,
}

/// A key + weight pair within a [`Permission`].
#[derive(Clone, Debug)]
pub struct PermissionKey {
    /// Authorized address.
    pub address: Address,
    /// Voting weight of this key.
    pub weight: i64,
}

/// Result of a constant (read-only) smart-contract call.
#[derive(Clone, Debug, Default)]
pub struct ConstantCallResult {
    /// Raw ABI-encoded return data.
    pub output: Vec<u8>,
    /// Energy the call would have consumed.
    pub energy_used: i64,
    /// Revert message, if the call reverted.
    pub revert_reason: Option<String>,
}

/// Metadata about a deployed smart contract.
#[derive(Clone, Debug, Default)]
pub struct SmartContractInfo {
    /// Contract address.
    pub address: Option<Address>,
    /// Deployer address.
    pub origin_address: Option<Address>,
    /// JSON-encoded ABI bytes.
    pub abi: Vec<u8>,
    /// Creation bytecode (as supplied to `deploy_contract`).
    pub bytecode: Bytes,
    /// Deployed (runtime) bytecode — only populated by
    /// [`get_contract_info`](crate::provider::TronProvider::get_contract_info).
    pub runtime_bytecode: Option<Bytes>,
    /// Contract name.
    pub name: String,
    /// Percentage of energy the caller pays.
    pub consume_user_resource_percent: i64,
    /// Per-call energy cap charged to the origin.
    pub origin_energy_limit: i64,
}
