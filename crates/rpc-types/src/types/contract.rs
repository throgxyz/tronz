//! TRON native contract types and their parameter structs.

use std::{collections::HashMap, fmt};

use tronz_abi::TronAbi;
use tronz_primitives::{Address, B256, Bytes, ResourceCode, Trx};

/// All TRON native contract types. Discriminants mirror the protobuf
/// `Transaction.Contract.ContractType` enum.
///
/// Only the `v0` variants carry fully-defined parameter structs today; the
/// remaining variants are reserved for later milestones.
///
/// Exhaustive, unlike the types a node returns: a provider routes every variant
/// to the endpoint that builds it, and that table has to stop compiling when a
/// variant is added without a route. Adding one is therefore a breaking change,
/// which on TRON is a rare and visible event anyway.
#[derive(Clone, Debug)]
pub enum ContractType {
    // --- v0 ---
    /// Transfer TRX.
    Transfer(TransferContract),
    /// Call/trigger a smart contract.
    TriggerSmartContract(TriggerSmartContract),
    /// Stake TRX for a resource (Stake 1.0, legacy).
    FreezeBalanceV1(FreezeBalanceV1Contract),
    /// Unstake TRX (Stake 1.0, legacy).
    UnfreezeBalanceV1(UnfreezeBalanceV1Contract),
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
    /// Participate in a TRC10 token ICO.
    ParticipateAssetIssue(ParticipateAssetIssueContract),
    /// Release frozen TRC10 token supply after the lock period.
    UnfreezeAsset(UnfreezeAssetContract),
    /// Update TRC10 token metadata.
    UpdateAsset(UpdateAssetContract),
    /// Activate a new account by sending TRX to it.
    CreateAccount(CreateAccountContract),
    /// Vote for super representatives.
    VoteWitness(VoteWitnessContract),
    /// Update account name.
    UpdateAccount(UpdateAccountContract),
    /// Submit a chain-parameter governance proposal.
    ProposalCreate(ProposalCreateContract),
    /// Approve or disapprove a governance proposal.
    ProposalApprove(ProposalApproveContract),
    /// Cancel a governance proposal.
    ProposalDelete(ProposalDeleteContract),
    /// Apply to become a super representative candidate.
    CreateWitness(CreateWitnessContract),
    /// Update a super representative's public URL.
    UpdateWitness(UpdateWitnessContract),
    /// Update a super representative's brokerage ratio.
    UpdateBrokerage(UpdateBrokerageContract),
    /// Set a short alphanumeric on-chain account ID.
    SetAccountId(SetAccountIdContract),
    /// Clear a deployed smart contract's ABI.
    ClearContractAbi(ClearContractAbiContract),
    /// Update the caller-energy-percentage setting on a smart contract.
    UpdateSetting(UpdateSettingContract),
    /// Update the per-call origin energy limit on a smart contract.
    UpdateEnergyLimit(UpdateEnergyLimitContract),
    // --- DEX (built-in Bancor exchange, TRC10 pairs) ---
    /// Create a new TRC10 exchange pair.
    ExchangeCreate(ExchangeCreateContract),
    /// Inject liquidity into an exchange pair.
    ExchangeInject(ExchangeInjectContract),
    /// Withdraw liquidity from an exchange pair.
    ExchangeWithdraw(ExchangeWithdrawContract),
    /// Execute a trade on an exchange pair.
    ExchangeTransaction(ExchangeTransactionContract),
    // --- Market (order-book DEX) ---
    /// Place a limit sell order on the order-book DEX.
    MarketSellAsset(MarketSellAssetContract),
    /// Cancel an open market order.
    MarketCancelOrder(MarketCancelOrderContract),
}

impl ContractType {
    /// Whether this contract type requires a `fee_limit` to be set
    /// (smart-contract operations) versus native contracts that ignore it.
    pub fn needs_fee_limit(&self) -> bool {
        matches!(self, ContractType::TriggerSmartContract(_) | ContractType::CreateSmartContract(_))
    }

    /// The owner (sender) address of this contract operation.
    pub fn owner_address(&self) -> Address {
        match self {
            ContractType::Transfer(c) => c.owner_address,
            ContractType::TriggerSmartContract(c) => c.owner_address,
            ContractType::FreezeBalanceV1(c) => c.owner_address,
            ContractType::UnfreezeBalanceV1(c) => c.owner_address,
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
            ContractType::ParticipateAssetIssue(c) => c.owner_address,
            ContractType::UnfreezeAsset(c) => c.owner_address,
            ContractType::UpdateAsset(c) => c.owner_address,
            ContractType::CreateAccount(c) => c.owner_address,
            ContractType::VoteWitness(c) => c.owner_address,
            ContractType::UpdateAccount(c) => c.owner_address,
            ContractType::ProposalCreate(c) => c.owner_address,
            ContractType::ProposalApprove(c) => c.owner_address,
            ContractType::ProposalDelete(c) => c.owner_address,
            ContractType::CreateWitness(c) => c.owner_address,
            ContractType::UpdateWitness(c) => c.owner_address,
            ContractType::UpdateBrokerage(c) => c.owner_address,
            ContractType::SetAccountId(c) => c.owner_address,
            ContractType::ClearContractAbi(c) => c.owner_address,
            ContractType::UpdateSetting(c) => c.owner_address,
            ContractType::UpdateEnergyLimit(c) => c.owner_address,
            ContractType::ExchangeCreate(c) => c.owner_address,
            ContractType::ExchangeInject(c) => c.owner_address,
            ContractType::ExchangeWithdraw(c) => c.owner_address,
            ContractType::ExchangeTransaction(c) => c.owner_address,
            ContractType::MarketSellAsset(c) => c.owner_address,
            ContractType::MarketCancelOrder(c) => c.owner_address,
        }
    }
}

/// A native contract kind without its parameters.
///
/// Unknown protobuf values are preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContractKind {
    /// Activate a new account by sending TRX to it.
    CreateAccount,
    /// Transfer TRX.
    Transfer,
    /// Transfer a TRC10 token.
    TransferAsset,
    /// Vote for a TRC10 token (unused on-chain).
    VoteAsset,
    /// Vote for super representatives.
    VoteWitness,
    /// Apply to become a super representative candidate.
    CreateWitness,
    /// Issue (create) a new TRC10 native token.
    AssetIssue,
    /// Update a super representative's public URL.
    UpdateWitness,
    /// Participate in a TRC10 token ICO.
    ParticipateAssetIssue,
    /// Update account name.
    UpdateAccount,
    /// Stake TRX for a resource (Stake 1.0, legacy).
    FreezeBalanceV1,
    /// Unstake TRX (Stake 1.0, legacy).
    UnfreezeBalanceV1,
    /// Claim accumulated block/vote rewards.
    WithdrawBalance,
    /// Release frozen TRC10 token supply after the lock period.
    UnfreezeAsset,
    /// Update TRC10 token metadata.
    UpdateAsset,
    /// Submit a chain-parameter governance proposal.
    ProposalCreate,
    /// Approve or disapprove a governance proposal.
    ProposalApprove,
    /// Cancel a governance proposal.
    ProposalDelete,
    /// Set a short alphanumeric on-chain account ID.
    SetAccountId,
    /// Reserved by java-tron, with no message defined for it.
    Custom,
    /// Deploy a new smart contract.
    CreateSmartContract,
    /// Call/trigger a smart contract.
    TriggerSmartContract,
    /// Reserved by java-tron for a query, not a transaction.
    GetContract,
    /// Update the caller-energy-percentage setting on a smart contract.
    UpdateSetting,
    /// Create a new TRC10 exchange pair.
    ExchangeCreate,
    /// Inject liquidity into an exchange pair.
    ExchangeInject,
    /// Withdraw liquidity from an exchange pair.
    ExchangeWithdraw,
    /// Execute a trade on an exchange pair.
    ExchangeTransaction,
    /// Update the per-call origin energy limit on a smart contract.
    UpdateEnergyLimit,
    /// Update account permissions (multisig).
    AccountPermissionUpdate,
    /// Clear a deployed smart contract's ABI.
    ClearContractAbi,
    /// Update a super representative's brokerage ratio.
    UpdateBrokerage,
    /// Shielded (private) transfer.
    ShieldedTransfer,
    /// Place a limit sell order on the order-book DEX.
    MarketSellAsset,
    /// Cancel an open market order.
    MarketCancelOrder,
    /// Stake TRX for a resource (Stake 2.0).
    FreezeBalanceV2,
    /// Unstake TRX (Stake 2.0).
    UnfreezeBalanceV2,
    /// Withdraw TRX from expired unfreeze windows.
    WithdrawExpireUnfreeze,
    /// Delegate a resource to another account.
    DelegateResource,
    /// Reclaim a delegated resource.
    UnDelegateResource,
    /// Cancel all in-progress unfreeze operations.
    CancelAllUnfreezeV2,
    /// A contract type this build does not know, keeping the protobuf id.
    Unknown(i32),
}

impl From<i32> for ContractKind {
    fn from(v: i32) -> Self {
        match v {
            0 => Self::CreateAccount,
            1 => Self::Transfer,
            2 => Self::TransferAsset,
            3 => Self::VoteAsset,
            4 => Self::VoteWitness,
            5 => Self::CreateWitness,
            6 => Self::AssetIssue,
            8 => Self::UpdateWitness,
            9 => Self::ParticipateAssetIssue,
            10 => Self::UpdateAccount,
            11 => Self::FreezeBalanceV1,
            12 => Self::UnfreezeBalanceV1,
            13 => Self::WithdrawBalance,
            14 => Self::UnfreezeAsset,
            15 => Self::UpdateAsset,
            16 => Self::ProposalCreate,
            17 => Self::ProposalApprove,
            18 => Self::ProposalDelete,
            19 => Self::SetAccountId,
            20 => Self::Custom,
            30 => Self::CreateSmartContract,
            31 => Self::TriggerSmartContract,
            32 => Self::GetContract,
            33 => Self::UpdateSetting,
            41 => Self::ExchangeCreate,
            42 => Self::ExchangeInject,
            43 => Self::ExchangeWithdraw,
            44 => Self::ExchangeTransaction,
            45 => Self::UpdateEnergyLimit,
            46 => Self::AccountPermissionUpdate,
            48 => Self::ClearContractAbi,
            49 => Self::UpdateBrokerage,
            51 => Self::ShieldedTransfer,
            52 => Self::MarketSellAsset,
            53 => Self::MarketCancelOrder,
            54 => Self::FreezeBalanceV2,
            55 => Self::UnfreezeBalanceV2,
            56 => Self::WithdrawExpireUnfreeze,
            57 => Self::DelegateResource,
            58 => Self::UnDelegateResource,
            59 => Self::CancelAllUnfreezeV2,
            other => Self::Unknown(other),
        }
    }
}

impl ContractKind {
    /// Returns the protobuf id.
    pub const fn id(&self) -> i32 {
        match self {
            Self::CreateAccount => 0,
            Self::Transfer => 1,
            Self::TransferAsset => 2,
            Self::VoteAsset => 3,
            Self::VoteWitness => 4,
            Self::CreateWitness => 5,
            Self::AssetIssue => 6,
            Self::UpdateWitness => 8,
            Self::ParticipateAssetIssue => 9,
            Self::UpdateAccount => 10,
            Self::FreezeBalanceV1 => 11,
            Self::UnfreezeBalanceV1 => 12,
            Self::WithdrawBalance => 13,
            Self::UnfreezeAsset => 14,
            Self::UpdateAsset => 15,
            Self::ProposalCreate => 16,
            Self::ProposalApprove => 17,
            Self::ProposalDelete => 18,
            Self::SetAccountId => 19,
            Self::Custom => 20,
            Self::CreateSmartContract => 30,
            Self::TriggerSmartContract => 31,
            Self::GetContract => 32,
            Self::UpdateSetting => 33,
            Self::ExchangeCreate => 41,
            Self::ExchangeInject => 42,
            Self::ExchangeWithdraw => 43,
            Self::ExchangeTransaction => 44,
            Self::UpdateEnergyLimit => 45,
            Self::AccountPermissionUpdate => 46,
            Self::ClearContractAbi => 48,
            Self::UpdateBrokerage => 49,
            Self::ShieldedTransfer => 51,
            Self::MarketSellAsset => 52,
            Self::MarketCancelOrder => 53,
            Self::FreezeBalanceV2 => 54,
            Self::UnfreezeBalanceV2 => 55,
            Self::WithdrawExpireUnfreeze => 56,
            Self::DelegateResource => 57,
            Self::UnDelegateResource => 58,
            Self::CancelAllUnfreezeV2 => 59,
            Self::Unknown(id) => *id,
        }
    }
}

impl fmt::Display for ContractKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "Unknown({id})"),
            known => write!(f, "{known:?}"),
        }
    }
}

/// A 256-bit set of native contract operations.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct OperationSet([u8; 32]);

impl OperationSet {
    /// An empty operation set.
    pub const fn empty() -> Self {
        Self([0; 32])
    }

    /// Decodes a bitmap, accepting omitted trailing zero bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OperationSetError> {
        if bytes.len() > 32 && bytes[32..].iter().any(|byte| *byte != 0) {
            return Err(OperationSetError::NonZeroExcessBits);
        }
        let mut bitmap = [0u8; 32];
        let copied = bytes.len().min(bitmap.len());
        bitmap[..copied].copy_from_slice(&bytes[..copied]);
        Ok(Self(bitmap))
    }

    /// The canonical 32-byte representation sent to a node.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Whether no operation is granted.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }

    /// Whether this set grants `kind`.
    pub fn contains(&self, kind: ContractKind) -> bool {
        let Ok(bit) = usize::try_from(kind.id()) else {
            return false;
        };
        self.0.get(bit / 8).is_some_and(|byte| byte & (1 << (bit % 8)) != 0)
    }

    /// Add `kind`, returning whether it was newly inserted.
    pub fn insert(&mut self, kind: ContractKind) -> Result<bool, OperationSetError> {
        let id = kind.id();
        let bit = usize::try_from(id)
            .ok()
            .filter(|bit| *bit < 256)
            .ok_or(OperationSetError::OutOfRange(id))?;
        let mask = 1 << (bit % 8);
        let byte = &mut self.0[bit / 8];
        let inserted = *byte & mask == 0;
        *byte |= mask;
        Ok(inserted)
    }

    /// Remove `kind`, returning whether it was present.
    pub fn remove(&mut self, kind: ContractKind) -> bool {
        let Ok(bit) = usize::try_from(kind.id()) else {
            return false;
        };
        let Some(byte) = self.0.get_mut(bit / 8) else {
            return false;
        };
        let mask = 1 << (bit % 8);
        let removed = *byte & mask != 0;
        *byte &= !mask;
        removed
    }

    /// Iterate over every granted operation in numeric id order.
    pub fn iter(&self) -> OperationSetIter<'_> {
        OperationSetIter { set: self, next: 0 }
    }

    /// Builds a set from operation kinds.
    pub fn try_from_iter(
        kinds: impl IntoIterator<Item = ContractKind>,
    ) -> Result<Self, OperationSetError> {
        let mut set = Self::empty();
        for kind in kinds {
            set.insert(kind)?;
        }
        Ok(set)
    }
}

impl<const N: usize> TryFrom<[ContractKind; N]> for OperationSet {
    type Error = OperationSetError;

    fn try_from(kinds: [ContractKind; N]) -> Result<Self, Self::Error> {
        Self::try_from_iter(kinds)
    }
}

impl TryFrom<&[ContractKind]> for OperationSet {
    type Error = OperationSetError;

    fn try_from(kinds: &[ContractKind]) -> Result<Self, Self::Error> {
        Self::try_from_iter(kinds.iter().copied())
    }
}

impl<'a> IntoIterator for &'a OperationSet {
    type Item = ContractKind;
    type IntoIter = OperationSetIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over the operations granted by an [`OperationSet`].
pub struct OperationSetIter<'a> {
    set: &'a OperationSet,
    next: usize,
}

impl Iterator for OperationSetIter<'_> {
    type Item = ContractKind;

    fn next(&mut self) -> Option<Self::Item> {
        while self.next < 256 {
            let id = self.next;
            self.next += 1;
            if self.set.0[id / 8] & (1 << (id % 8)) != 0 {
                return Some(ContractKind::from(id as i32));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(256 - self.next))
    }
}

impl fmt::Debug for OperationSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

/// An invalid operation bitmap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OperationSetError {
    /// The contract id falls outside the bitmap.
    #[error("contract operation id {0} is outside the permission bitmap")]
    OutOfRange(i32),
    /// The bitmap has non-zero excess bits.
    #[error("permission bitmap has non-zero bits beyond its 32-byte boundary")]
    NonZeroExcessBits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnerField {
    First,
    Second,
}

impl ContractKind {
    pub(crate) const fn owner_field(&self) -> Option<OwnerField> {
        match self {
            Self::TransferAsset | Self::UpdateAccount | Self::SetAccountId => {
                Some(OwnerField::Second)
            }
            Self::ShieldedTransfer | Self::Custom | Self::GetContract | Self::Unknown(_) => None,
            _ => Some(OwnerField::First),
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

/// Stake TRX for energy or bandwidth (Stake 1.0, legacy).
///
/// `frozen_duration` must be `3` on mainnet (the only accepted value).
/// Set `receiver_address` to delegate the obtained resource to another account
/// in a single step (inline delegation).
#[derive(Clone, Debug)]
pub struct FreezeBalanceV1Contract {
    /// Account staking the TRX.
    pub owner_address: Address,
    /// Amount of TRX to stake.
    pub frozen_balance: Trx,
    /// Lock duration in days. Must be `3` on mainnet.
    pub frozen_duration: i64,
    /// Resource to obtain.
    pub resource: ResourceCode,
    /// Optional: delegate the resource to this account (inline delegation).
    pub receiver_address: Option<Address>,
}

/// Unstake TRX (Stake 1.0, legacy).
///
/// Unlike Stake 2.0, this unfreezes **all** staked TRX for the given resource
/// and releases the funds immediately (no unbonding delay).
#[derive(Clone, Debug)]
pub struct UnfreezeBalanceV1Contract {
    /// Account unstaking.
    pub owner_address: Address,
    /// Resource being released.
    pub resource: ResourceCode,
    /// If the stake was delegated, the delegatee address.
    pub receiver_address: Option<Address>,
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
    /// Native TRON ABI metadata to store with the contract.
    pub abi: TronAbi,
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
/// `Trc10Api::get_asset_issue_by_account`.
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

/// Participate in a TRC10 token ICO by buying tokens with TRX.
///
/// The buyer (`owner_address`) sends `amount` sun to `to_address` (the issuer)
/// and receives the proportional amount of the token in return.
#[derive(Clone, Debug)]
pub struct ParticipateAssetIssueContract {
    /// Buyer address.
    pub owner_address: Address,
    /// Issuer / ICO address (the token creator).
    pub to_address: Address,
    /// Numeric token ID as a string (e.g. `"1000001"`).
    pub token_id: String,
    /// Amount of TRX in sun to spend.
    pub amount: i64,
}

/// Release TRC10 tokens that were locked as frozen supply during issuance.
///
/// After the lock period expires the issuer can call this to unfreeze them.
#[derive(Clone, Debug)]
pub struct UnfreezeAssetContract {
    /// Issuer address.
    pub owner_address: Address,
}

/// Update the metadata (description, URL, bandwidth limits) for a TRC10 token.
///
/// Only the original issuer can call this.
#[derive(Clone, Debug)]
pub struct UpdateAssetContract {
    /// Issuer address.
    pub owner_address: Address,
    /// New description (UTF-8).
    pub description: String,
    /// New project URL.
    pub url: String,
    /// New per-account free-transfer bandwidth limit.
    pub new_limit: i64,
    /// New total free-transfer bandwidth limit.
    pub new_public_limit: i64,
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
    /// Contract operations granted by an active permission.
    pub operations: OperationSet,
}

impl Permission {
    /// The weight `address` carries in this permission, or `None` if it is not
    /// an authorized key.
    pub fn weight_of(&self, address: &Address) -> Option<i64> {
        self.keys.iter().find(|key| key.address == *address).map(|key| key.weight)
    }

    /// The combined weight of the distinct `keys` that this permission
    /// authorizes.
    ///
    /// Addresses the permission does not authorize contribute nothing, and a
    /// repeated address counts once — the same as on-chain, where a second
    /// signature from one key adds no weight.
    pub fn weight_of_all<'a, I>(&self, keys: I) -> i64
    where
        I: IntoIterator<Item = &'a Address>,
    {
        let mut counted = Vec::new();
        let mut total = 0;
        for address in keys {
            if counted.contains(&address) {
                continue;
            }
            if let Some(weight) = self.weight_of(address) {
                counted.push(address);
                total += weight;
            }
        }
        total
    }

    /// Whether `keys` together reach this permission's threshold.
    pub fn is_satisfied_by<'a, I>(&self, keys: I) -> bool
    where
        I: IntoIterator<Item = &'a Address>,
    {
        self.weight_of_all(keys) >= self.threshold
    }
}

/// A key + weight pair within a [`Permission`].
#[derive(Clone, Debug)]
pub struct PermissionKey {
    /// Authorized address.
    pub address: Address,
    /// Voting weight of this key.
    pub weight: i64,
}

/// Submit a chain-parameter governance proposal.
///
/// Only SRs and SR partners can call this. A proposal takes effect if at least
/// 15 SRs (out of 27) approve it before the voting period ends.
#[derive(Clone, Debug)]
pub struct ProposalCreateContract {
    /// Proposer address (must be an SR or SR partner).
    pub owner_address: Address,
    /// Map of chain parameter ID → proposed new value.
    pub parameters: HashMap<i64, i64>,
}

/// Approve or disapprove a governance proposal.
#[derive(Clone, Debug)]
pub struct ProposalApproveContract {
    /// Voter address (must be an SR or SR partner).
    pub owner_address: Address,
    /// ID of the proposal to vote on.
    pub proposal_id: i64,
    /// `true` = add approval, `false` = revoke approval.
    pub is_add_approval: bool,
}

/// Cancel a governance proposal.
///
/// Only the original proposer can cancel, and only while it is still pending.
#[derive(Clone, Debug)]
pub struct ProposalDeleteContract {
    /// Proposer address.
    pub owner_address: Address,
    /// ID of the proposal to cancel.
    pub proposal_id: i64,
}

/// Apply to become a super representative (SR) candidate.
///
/// The applicant must post a 9,999 TRX deposit. The URL is a link to the SR's
/// public information page.
#[derive(Clone, Debug)]
pub struct CreateWitnessContract {
    /// Applicant address.
    pub owner_address: Address,
    /// Public URL for the SR's information page.
    pub url: String,
}

/// Update a super representative's public URL.
#[derive(Clone, Debug)]
pub struct UpdateWitnessContract {
    /// SR address.
    pub owner_address: Address,
    /// New public URL.
    pub update_url: String,
}

/// Update a super representative's brokerage ratio.
///
/// `brokerage` is a percentage (0–100): portion of block rewards the SR keeps.
/// The remainder is distributed to voters.
#[derive(Clone, Debug)]
pub struct UpdateBrokerageContract {
    /// SR address.
    pub owner_address: Address,
    /// New brokerage ratio (0–100).
    pub brokerage: i32,
}

/// Set a short alphanumeric account ID (on-chain alias).
///
/// The `account_id` must be unique across the network and can only be set once.
#[derive(Clone, Debug)]
pub struct SetAccountIdContract {
    /// Account being named.
    pub owner_address: Address,
    /// The account ID to assign (UTF-8, up to 32 bytes).
    pub account_id: String,
}

/// Clear the ABI of a deployed smart contract.
///
/// Only the contract owner can call this.
#[derive(Clone, Debug)]
pub struct ClearContractAbiContract {
    /// Contract owner address.
    pub owner_address: Address,
    /// Address of the contract whose ABI is being cleared.
    pub contract_address: Address,
}

/// Update the percentage of energy that callers pay (vs the contract origin).
///
/// Only the contract owner can call this.
#[derive(Clone, Debug)]
pub struct UpdateSettingContract {
    /// Contract owner address.
    pub owner_address: Address,
    /// Address of the contract being updated.
    pub contract_address: Address,
    /// New percentage (0–100) of energy charged to callers.
    pub consume_user_resource_percent: i64,
}

/// Update the per-call energy cap charged to the contract origin.
///
/// Only the contract owner can call this.
#[derive(Clone, Debug)]
pub struct UpdateEnergyLimitContract {
    /// Contract owner address.
    pub owner_address: Address,
    /// Address of the contract being updated.
    pub contract_address: Address,
    /// New per-call energy limit for the origin.
    pub origin_energy_limit: i64,
}

// ── DEX (built-in Bancor exchange) ───────────────────────────────────────────

/// Create a new TRC10 exchange pair with an initial liquidity deposit.
///
/// Token IDs use `"_"` for TRX and a numeric string (e.g. `"1000001"`) for TRC10.
#[derive(Clone, Debug)]
pub struct ExchangeCreateContract {
    /// Creator address.
    pub owner_address: Address,
    /// Token ID of the first token.
    pub first_token_id: String,
    /// Initial balance of the first token.
    pub first_token_balance: i64,
    /// Token ID of the second token.
    pub second_token_id: String,
    /// Initial balance of the second token.
    pub second_token_balance: i64,
}

/// Inject additional liquidity into an existing exchange pair.
#[derive(Clone, Debug)]
pub struct ExchangeInjectContract {
    /// Injector address (must be the exchange creator).
    pub owner_address: Address,
    /// Exchange ID.
    pub exchange_id: i64,
    /// Token ID of the token being injected.
    pub token_id: String,
    /// Amount to inject.
    pub quant: i64,
}

/// Withdraw liquidity from an existing exchange pair.
#[derive(Clone, Debug)]
pub struct ExchangeWithdrawContract {
    /// Withdrawer address (must be the exchange creator).
    pub owner_address: Address,
    /// Exchange ID.
    pub exchange_id: i64,
    /// Token ID of the token being withdrawn.
    pub token_id: String,
    /// Amount to withdraw.
    pub quant: i64,
}

/// Execute a trade (swap) on an existing exchange pair.
#[derive(Clone, Debug)]
pub struct ExchangeTransactionContract {
    /// Trader address.
    pub owner_address: Address,
    /// Exchange ID.
    pub exchange_id: i64,
    /// Token ID of the token being sold.
    pub token_id: String,
    /// Amount of the sell token to trade.
    pub quant: i64,
    /// Minimum amount of the other token expected in return (slippage protection).
    pub expected: i64,
}

// ── Market Orders (order-book DEX) ───────────────────────────────────────────

/// Place a limit sell order on the order-book DEX.
///
/// Token IDs use `"_"` for TRX and a numeric string (e.g. `"1000001"`) for TRC10.
/// `buy_token_quantity` is the *minimum* amount of the buy token to accept — the
/// effective limit price.
#[derive(Clone, Debug)]
pub struct MarketSellAssetContract {
    /// Seller address.
    pub owner_address: Address,
    /// Token ID of the token being sold.
    pub sell_token_id: String,
    /// Quantity of the sell token to offer.
    pub sell_token_quantity: i64,
    /// Token ID of the token to receive.
    pub buy_token_id: String,
    /// Minimum quantity of the buy token to accept (sets the limit price).
    pub buy_token_quantity: i64,
}

/// Cancel an open market order.
#[derive(Clone, Debug)]
pub struct MarketCancelOrderContract {
    /// Order owner address.
    pub owner_address: Address,
    /// The 32-byte order ID to cancel.
    pub order_id: B256,
}

/// Result of a constant (read-only) smart-contract call.
#[derive(Clone, Debug, Default)]
pub struct ConstantCallResult {
    /// Raw ABI-encoded return data.
    pub output: Bytes,
    /// Energy the call would have consumed.
    pub energy_used: i64,
    /// Revert message, if the call reverted.
    pub revert_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // USDT contract address (mainnet), used as a stable test address.
    const ADDR: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

    #[test]
    fn every_contract_type_the_protocol_defines_maps_to_a_kind() {
        use crate::proto::transaction::contract::ContractType as Proto;

        for id in 0..=64 {
            if Proto::try_from(id).is_err() {
                continue;
            }

            assert_ne!(ContractKind::from(id), ContractKind::Unknown(id), "protobuf id {id}");
        }
    }

    fn addr() -> Address {
        ADDR.parse().unwrap()
    }

    #[test]
    fn owner_address_new_variants() {
        let a = addr();

        // Governance
        let c = ContractType::ProposalCreate(ProposalCreateContract {
            owner_address: a,
            parameters: Default::default(),
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::ProposalApprove(ProposalApproveContract {
            owner_address: a,
            proposal_id: 1,
            is_add_approval: true,
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::ProposalDelete(ProposalDeleteContract {
            owner_address: a,
            proposal_id: 1,
        });
        assert_eq!(c.owner_address(), a);

        // Witness
        let c = ContractType::CreateWitness(CreateWitnessContract {
            owner_address: a,
            url: "https://example.com".into(),
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::UpdateWitness(UpdateWitnessContract {
            owner_address: a,
            update_url: "https://example.com".into(),
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::UpdateBrokerage(UpdateBrokerageContract {
            owner_address: a,
            brokerage: 20,
        });
        assert_eq!(c.owner_address(), a);

        // Account / contract management
        let c = ContractType::SetAccountId(SetAccountIdContract {
            owner_address: a,
            account_id: "myacct".into(),
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::ClearContractAbi(ClearContractAbiContract {
            owner_address: a,
            contract_address: a,
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::UpdateSetting(UpdateSettingContract {
            owner_address: a,
            contract_address: a,
            consume_user_resource_percent: 100,
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::UpdateEnergyLimit(UpdateEnergyLimitContract {
            owner_address: a,
            contract_address: a,
            origin_energy_limit: 100_000,
        });
        assert_eq!(c.owner_address(), a);

        // TRC10
        let c = ContractType::ParticipateAssetIssue(ParticipateAssetIssueContract {
            owner_address: a,
            to_address: a,
            token_id: "1000001".into(),
            amount: 1_000_000,
        });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::UnfreezeAsset(UnfreezeAssetContract { owner_address: a });
        assert_eq!(c.owner_address(), a);

        let c = ContractType::UpdateAsset(UpdateAssetContract {
            owner_address: a,
            description: "desc".into(),
            url: "https://example.com".into(),
            new_limit: 0,
            new_public_limit: 0,
        });
        assert_eq!(c.owner_address(), a);
    }

    #[test]
    fn needs_fee_limit_only_for_smart_contracts() {
        let a = addr();

        // Native contracts do NOT need fee_limit.
        assert!(
            !ContractType::ProposalCreate(ProposalCreateContract {
                owner_address: a,
                parameters: Default::default(),
            })
            .needs_fee_limit()
        );
        assert!(
            !ContractType::CreateWitness(CreateWitnessContract {
                owner_address: a,
                url: String::new(),
            })
            .needs_fee_limit()
        );
        assert!(
            !ContractType::UpdateBrokerage(UpdateBrokerageContract {
                owner_address: a,
                brokerage: 20,
            })
            .needs_fee_limit()
        );
        assert!(
            !ContractType::ClearContractAbi(ClearContractAbiContract {
                owner_address: a,
                contract_address: a,
            })
            .needs_fee_limit()
        );
        assert!(
            !ContractType::UpdateSetting(UpdateSettingContract {
                owner_address: a,
                contract_address: a,
                consume_user_resource_percent: 0,
            })
            .needs_fee_limit()
        );
        assert!(
            !ContractType::UpdateEnergyLimit(UpdateEnergyLimitContract {
                owner_address: a,
                contract_address: a,
                origin_energy_limit: 0,
            })
            .needs_fee_limit()
        );
    }

    fn key(byte: u8, weight: i64) -> PermissionKey {
        let mut bytes = [0u8; 20];
        bytes[19] = byte;
        PermissionKey { address: Address::from_evm_bytes(bytes), weight }
    }

    fn two_of_three() -> Permission {
        Permission {
            id: 2,
            permission_name: "active".to_string(),
            threshold: 3,
            keys: vec![key(1, 2), key(2, 2), key(3, 2)],
            operations: OperationSet::try_from([ContractKind::Transfer]).unwrap(),
        }
    }

    #[test]
    fn permission_weighs_known_keys_only() {
        let permission = two_of_three();
        let stranger = key(9, 0).address;

        assert_eq!(permission.weight_of(&key(1, 0).address), Some(2));
        assert_eq!(permission.weight_of(&stranger), None);
        assert_eq!(permission.weight_of_all(&[key(1, 0).address, stranger]), 2);
    }

    #[test]
    fn permission_counts_a_repeated_key_once() {
        let permission = two_of_three();
        let address = key(1, 0).address;

        assert_eq!(permission.weight_of_all(&[address, address, address]), 2);
        assert!(!permission.is_satisfied_by(&[address, address]));
    }

    #[test]
    fn permission_is_satisfied_once_the_threshold_is_reached() {
        let permission = two_of_three();

        assert!(!permission.is_satisfied_by(&[key(1, 0).address]));
        assert!(permission.is_satisfied_by(&[key(1, 0).address, key(2, 0).address]));
    }

    #[test]
    fn every_contract_kind_survives_a_round_trip_through_its_id() {
        for id in 0..80 {
            let kind = ContractKind::from(id);
            assert_eq!(kind.id(), id, "{kind} does not report the id it was built from");
        }
        assert_eq!(ContractKind::from(4_000), ContractKind::Unknown(4_000));
    }

    #[test]
    fn operations_bitmaps_name_the_types_their_bits_grant() {
        let mut bitmap = [0u8; 32];
        bitmap[0] = 0b11; // CreateAccount (0) and Transfer (1)
        bitmap[3] = 0x80; // TriggerSmartContract (31)
        bitmap[7] = 0x20; // a type at bit 61, which no build knows yet

        let set = OperationSet::from_bytes(&bitmap).unwrap();
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            vec![
                ContractKind::CreateAccount,
                ContractKind::Transfer,
                ContractKind::TriggerSmartContract,
                ContractKind::Unknown(61),
            ]
        );
        assert_eq!(*set.as_bytes(), bitmap);
    }

    #[test]
    fn a_bitmap_cannot_hold_a_type_beyond_its_256_bits() {
        let err = OperationSet::try_from([
            ContractKind::Transfer,
            ContractKind::Unknown(256),
            ContractKind::Unknown(-1),
        ])
        .unwrap_err();
        assert_eq!(err, OperationSetError::OutOfRange(256));
    }

    #[test]
    fn operation_bitmaps_normalize_only_zero_extension() {
        let short = OperationSet::from_bytes(&[0b10]).unwrap();
        assert!(short.contains(ContractKind::Transfer));
        assert_eq!(short.as_bytes()[1..], [0; 31]);

        let mut long = vec![0u8; 34];
        long[0] = 0b10;
        assert_eq!(OperationSet::from_bytes(&long).unwrap(), short);
        long[33] = 1;
        assert_eq!(OperationSet::from_bytes(&long), Err(OperationSetError::NonZeroExcessBits));
    }
}

/// Metadata about a deployed smart contract.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct SmartContractInfo {
    /// Contract address.
    pub address: Option<Address>,
    /// Deployer address.
    pub origin_address: Option<Address>,
    /// Native TRON ABI metadata returned by the node.
    pub abi: TronAbi,
    /// Creation bytecode (as supplied to `deploy_contract`).
    pub bytecode: Bytes,
    /// Deployed (runtime) bytecode — only populated by
    /// `TronProvider::get_contract_info`.
    pub runtime_bytecode: Option<Bytes>,
    /// Contract name.
    pub name: String,
    /// Percentage of energy the caller pays.
    pub consume_user_resource_percent: i64,
    /// Per-call energy cap charged to the origin.
    pub origin_energy_limit: i64,
}
