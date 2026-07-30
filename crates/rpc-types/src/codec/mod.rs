//! Proto ↔ domain type conversions.
//!
//! Pure data mapping between the two representations this crate owns: no
//! transport, no gRPC. The transport crates call these to turn a node's messages
//! into the domain model and back.

mod abi;

use prost::Message as _;
use tronz_primitives::{Address, B256, Bytes, Log, Trx, TxId};

use crate::{
    error::ResponseError,
    proto,
    types::{
        AccountInfo, AccountNet, AccountPermissionUpdateContract, AccountPermissions,
        AccountResource, AssetInfo, AssetIssueContract, CallValue, ChainProperties,
        ClearContractAbiContract, ConstantCallResult, ContractResult, CreateAccountContract,
        CreateSmartContract, CreateWitnessContract, DelegatedResource, DelegatedResourceIndex,
        ExchangeCreateContract, ExchangeInfo, ExchangeInjectContract, ExchangeTransactionContract,
        ExchangeWithdrawContract, FreezeV2, InternalTransaction, MarketCancelOrderContract,
        MarketOrderInfo, MarketOrderPair, MarketOrderState, MarketPrice, MarketSellAssetContract,
        NodeAddress, NodeInfo, OperationSet, ParticipateAssetIssueContract, Permission,
        PermissionKey, ProposalApproveContract, ProposalCreateContract, ProposalDeleteContract,
        ProposalInfo, ProposalState, RawTransaction, ResourceReceipt, SetAccountIdContract,
        SignWeight, SignedTransaction, SmartContractInfo, TransactionInfo, TransferAssetContract,
        TransferContract, TriggerSmartContract, TxStatus, UnfreezeAssetContract, UnfreezeV2,
        UpdateAccountContract, UpdateAssetContract, UpdateBrokerageContract,
        UpdateEnergyLimitContract, UpdateSettingContract, UpdateWitnessContract, Vote,
        VoteWitnessContract, WitnessInfo,
    },
};

pub fn check_return(ret: Option<proto::Return>) -> Result<(), ResponseError> {
    if let Some(ret) = ret
        && !ret.result
    {
        return Err(ResponseError::NodeError(String::from_utf8_lossy(&ret.message).into_owned()));
    }
    Ok(())
}

/// Serialize a domain `Address` to the proto wire format (21-byte vec).
#[inline]
fn addr_bytes(a: Address) -> Vec<u8> {
    a.as_bytes().to_vec()
}

/// Serialize a Rust `String` to proto bytes (UTF-8, no NUL terminator).
#[inline]
fn str_bytes(s: String) -> Vec<u8> {
    s.into_bytes()
}

fn addr(bytes: Vec<u8>) -> Result<Address, ResponseError> {
    Address::from_slice(&bytes).map_err(|e| ResponseError::Malformed(format!("bad address: {e}")))
}

fn opt_addr(bytes: Vec<u8>) -> Option<Address> {
    if bytes.is_empty() { None } else { Address::from_slice(&bytes).ok() }
}

fn log_addr(bytes: Vec<u8>) -> Result<Address, ResponseError> {
    match bytes.as_slice().try_into() {
        Ok(evm) => Ok(Address::from_evm_bytes(evm)),
        Err(_) => addr(bytes),
    }
}

/// Convert a byte vec to a B256. Returns `B256::ZERO` when the slice is not
/// exactly 32 bytes, and emits a `warn!`.
///
/// This is acceptable for log topics (a wrong-length topic simply won't match
/// any filter). Block-summary conversion validates block hash lengths instead
/// of using this fallback.
fn b256(bytes: Vec<u8>) -> B256 {
    if bytes.len() == 32 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        B256::from(arr)
    } else {
        warn!(len = bytes.len(), "unexpected b256 byte length from node, substituting B256::ZERO");
        B256::ZERO
    }
}

/// Convert a proto `Account` into `AccountInfo`.
///
/// `queried` is the address that was requested — used as a fallback when the
/// node returns an empty address field (happens for non-existent accounts on
/// some TRON fullnode versions).
pub fn account_from_proto(
    a: proto::Account,
    queried: Address,
) -> Result<AccountInfo, ResponseError> {
    let is_activated = !a.address.is_empty();
    let address = if a.address.is_empty() { queried } else { addr(a.address.clone())? };

    let frozen_v2 = a
        .frozen_v2
        .into_iter()
        .filter_map(|f| {
            tronz_primitives::ResourceCode::from_i32(f.r#type)
                .map(|r| FreezeV2 { resource: r, amount: Trx::from_sun_unchecked(f.amount) })
        })
        .collect();

    let unfrozen_v2 = a
        .unfrozen_v2
        .into_iter()
        .filter_map(|u| {
            tronz_primitives::ResourceCode::from_i32(u.r#type).map(|r| UnfreezeV2 {
                resource: r,
                amount: Trx::from_sun_unchecked(u.unfreeze_amount),
                expire_time_ms: u.unfreeze_expire_time,
            })
        })
        .collect();

    let votes = a
        .votes
        .into_iter()
        .filter_map(|v| {
            addr(v.vote_address)
                .inspect_err(|e| warn!("skipping vote entry with bad address: {e}"))
                .ok()
                .map(|va| Vote { vote_address: va, vote_count: v.vote_count })
        })
        .collect();

    let permissions = AccountPermissions {
        owner: a.owner_permission.and_then(|p| {
            permission_from_proto(p)
                .inspect_err(|e| warn!("skipping malformed owner permission: {e}"))
                .ok()
        }),
        witness: a.witness_permission.and_then(|p| {
            permission_from_proto(p)
                .inspect_err(|e| warn!("skipping malformed witness permission: {e}"))
                .ok()
        }),
        actives: a
            .active_permission
            .into_iter()
            .filter_map(|p| {
                permission_from_proto(p)
                    .inspect_err(|e| warn!("skipping malformed active permission: {e}"))
                    .ok()
            })
            .collect(),
    };

    Ok(AccountInfo {
        address,
        balance: Trx::from_sun_unchecked(a.balance),
        name: String::from_utf8_lossy(&a.account_name).into_owned(),
        is_activated,
        frozen_v2,
        unfrozen_v2,
        votes,
        permissions,
        trc10_balances: a.asset_v2,
    })
}

fn permission_from_proto(p: proto::Permission) -> Result<Permission, ResponseError> {
    let keys = p
        .keys
        .into_iter()
        .filter_map(|k| {
            addr(k.address)
                .inspect_err(|e| warn!("skipping permission key with bad address: {e}"))
                .ok()
                .map(|a| PermissionKey { address: a, weight: k.weight })
        })
        .collect();
    Ok(Permission {
        id: p.id,
        permission_name: p.permission_name,
        threshold: p.threshold,
        keys,
        operations: OperationSet::from_bytes(&p.operations)
            .map_err(|e| ResponseError::Malformed(format!("bad permission operations: {e}")))?,
    })
}

pub fn account_resource_from_proto(r: proto::AccountResourceMessage) -> AccountResource {
    AccountResource {
        free_bandwidth_used: r.free_net_used,
        free_bandwidth_limit: r.free_net_limit,
        bandwidth_used: r.net_used,
        bandwidth_limit: r.net_limit,
        energy_used: r.energy_used,
        energy_limit: r.energy_limit,
        // tronPowerUsed / tronPowerLimit are in TRX units (1 vote = 1 TRX),
        // not sun — multiply by 1_000_000 to convert to the sun-based Trx type.
        tron_power_used: Trx::from_sun_unchecked(r.tron_power_used * 1_000_000),
        tron_power_limit: Trx::from_sun_unchecked(r.tron_power_limit * 1_000_000),
        ..Default::default()
    }
}

/// A transaction the caller asked for by id, absent if the chain has no such
/// transaction.
///
/// TRON answers an unknown id with an entirely empty message rather than an error.
/// Only that counts as absent: a message carrying signatures or results but no
/// `raw_data` is a broken answer, not a missing transaction.
pub fn signed_tx_lookup(
    tx: proto::Transaction,
    claimed_tx_id: &[u8],
) -> Result<Option<SignedTransaction>, ResponseError> {
    if tx == proto::Transaction::default() {
        return Ok(None);
    }

    SignedTransaction::from_node_encoded(tx.encode_to_vec(), claimed_tx_id).map(Some)
}

/// Returns `Ok(None)` when the node has not yet indexed the transaction
/// (empty `id` field).  Callers that need to wait for confirmation should
/// poll until they receive `Ok(Some(_))`.
pub fn transaction_info_from_proto(
    info: proto::TransactionInfo,
) -> Result<Option<TransactionInfo>, ResponseError> {
    if info.id.is_empty() {
        return Ok(None);
    }

    let tx_id = {
        let bytes: [u8; 32] =
            info.id.try_into().map_err(|_| ResponseError::Malformed("bad txid length".into()))?;
        TxId::from(bytes)
    };

    let receipt = info.receipt.unwrap_or_default();
    let contract_result = match receipt.result {
        0 => ContractResult::Default,
        1 => ContractResult::Success,
        2 => ContractResult::Revert,
        3 => ContractResult::BadJumpDestination,
        4 => ContractResult::OutOfMemory,
        5 => ContractResult::PrecompiledContract,
        6 => ContractResult::StackTooSmall,
        7 => ContractResult::StackTooLarge,
        8 => ContractResult::IllegalOperation,
        9 => ContractResult::StackOverflow,
        10 => ContractResult::OutOfEnergy,
        11 => ContractResult::OutOfTime,
        12 => ContractResult::JvmStackOverflow,
        13 => ContractResult::UnknownFailure,
        14 => ContractResult::TransferFailed,
        15 => ContractResult::InvalidCode,
        other => ContractResult::Other(other),
    };

    // Contract failures do not always set the top-level result code. A default
    // receipt keeps the top-level verdict for system contracts and transfers.
    let status = if info.result != 0 || contract_result.is_failure() {
        TxStatus::Failed
    } else {
        TxStatus::Success
    };

    let logs = info
        .log
        .into_iter()
        .map(|l| {
            // Preserve the node's response verbatim, even if it exceeds the
            // four-topic EVM limit; callers can check `Log::is_valid`.
            Ok(Log::new_unchecked(
                log_addr(l.address)?,
                l.topics.into_iter().map(b256).collect(),
                Bytes::from(l.data),
            ))
        })
        .collect::<Result<Vec<_>, ResponseError>>()?;

    let revert_reason = if info.res_message.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&info.res_message).into_owned())
    };

    let internal_transactions = info
        .internal_transactions
        .into_iter()
        .map(internal_transaction_from_proto)
        .collect::<Result<Vec<_>, ResponseError>>()?;

    Ok(Some(TransactionInfo {
        tx_id,
        block_number: info.block_number,
        block_timestamp: info.block_time_stamp,
        status,
        fee: Trx::from_sun_unchecked(info.fee),
        energy_usage_total: receipt.energy_usage_total,
        energy_fee: Trx::from_sun_unchecked(receipt.energy_fee),
        net_usage: receipt.net_usage,
        net_fee: Trx::from_sun_unchecked(receipt.net_fee),
        receipt: ResourceReceipt {
            energy_usage: receipt.energy_usage,
            energy_fee: Trx::from_sun_unchecked(receipt.energy_fee),
            origin_energy_usage: receipt.origin_energy_usage,
            energy_usage_total: receipt.energy_usage_total,
            net_usage: receipt.net_usage,
            net_fee: Trx::from_sun_unchecked(receipt.net_fee),
            energy_penalty_total: receipt.energy_penalty_total,
        },
        contract_result,
        contract_address: opt_addr(info.contract_address),
        logs,
        internal_transactions,
        revert_reason,
    }))
}

fn internal_transaction_from_proto(
    tx: proto::InternalTransaction,
) -> Result<InternalTransaction, ResponseError> {
    let hash: [u8; 32] = tx
        .hash
        .try_into()
        .map_err(|_| ResponseError::Malformed("bad internal transaction hash length".into()))?;

    Ok(InternalTransaction {
        hash: TxId::from(hash),
        caller_address: opt_addr(tx.caller_address),
        transfer_to_address: opt_addr(tx.transfer_to_address),
        call_values: tx
            .call_value_info
            .into_iter()
            .map(|v| CallValue { amount: v.call_value, token_id: v.token_id })
            .collect(),
        note: String::from_utf8_lossy(&tx.note).into_owned(),
        rejected: tx.rejected,
        extra: tx.extra,
    })
}

pub fn trigger_smart_contract_to_proto(p: TriggerSmartContract) -> proto::TriggerSmartContract {
    proto::TriggerSmartContract {
        owner_address: addr_bytes(p.owner_address),
        contract_address: addr_bytes(p.contract_address),
        call_value: p.call_value.as_sun(),
        data: p.data.into(),
        call_token_value: p.call_token_value.as_sun(),
        token_id: p.token_id,
    }
}

pub fn constant_result_from_extention(
    ext: proto::TransactionExtention,
) -> Result<ConstantCallResult, ResponseError> {
    let output: Bytes = ext.constant_result.into_iter().next().unwrap_or_default().into();

    let revert_reason = if let Some(ref r) = ext.result {
        if !r.result {
            let msg = String::from_utf8_lossy(&r.message).into_owned();
            if output.is_empty() {
                return Err(ResponseError::NodeError(msg));
            }
            Some(msg)
        } else {
            None
        }
    } else {
        None
    };

    Ok(ConstantCallResult { output, energy_used: ext.energy_used, revert_reason })
}

pub fn smart_contract_from_proto(c: proto::SmartContract) -> SmartContractInfo {
    SmartContractInfo {
        address: opt_addr(c.contract_address),
        origin_address: opt_addr(c.origin_address),
        abi: c.abi.map(abi::from_proto).unwrap_or_default(),
        bytecode: Bytes::from(c.bytecode),
        runtime_bytecode: None,
        name: c.name,
        consume_user_resource_percent: c.consume_user_resource_percent,
        origin_energy_limit: c.origin_energy_limit,
    }
}

pub fn smart_contract_info_from_wrapper(w: proto::SmartContractDataWrapper) -> SmartContractInfo {
    let mut info = w.smart_contract.map(smart_contract_from_proto).unwrap_or_default();
    if !w.runtimecode.is_empty() {
        info.runtime_bytecode = Some(Bytes::from(w.runtimecode));
    }
    info
}

pub fn witness_from_proto(w: proto::Witness) -> Option<WitnessInfo> {
    let address = opt_addr(w.address)?;
    Some(WitnessInfo {
        address,
        vote_count: w.vote_count,
        url: w.url,
        total_produced: w.total_produced,
        total_missed: w.total_missed,
        is_active: w.is_jobs,
    })
}

pub fn delegated_resource_from_proto(
    d: proto::DelegatedResource,
) -> Result<DelegatedResource, ResponseError> {
    Ok(DelegatedResource {
        from: addr(d.from)?,
        to: addr(d.to)?,
        bandwidth_amount: Trx::from_sun_unchecked(d.frozen_balance_for_bandwidth),
        energy_amount: Trx::from_sun_unchecked(d.frozen_balance_for_energy),
        bandwidth_expire_time_ms: d.expire_time_for_bandwidth,
        energy_expire_time_ms: d.expire_time_for_energy,
    })
}

pub fn transfer_to_proto(p: TransferContract) -> proto::TransferContract {
    proto::TransferContract {
        owner_address: addr_bytes(p.owner_address),
        to_address: addr_bytes(p.to_address),
        amount: p.amount.as_sun(),
    }
}

fn permission_to_proto(
    p: Permission,
    r#type: proto::permission::PermissionType,
) -> proto::Permission {
    use proto::permission::PermissionType;

    let operations = match r#type {
        PermissionType::Active => p.operations.as_bytes().to_vec(),
        PermissionType::Owner | PermissionType::Witness => Vec::new(),
    };

    proto::Permission {
        r#type: r#type as i32,
        id: p.id,
        permission_name: p.permission_name,
        threshold: p.threshold,
        parent_id: 0,
        operations,
        keys: p
            .keys
            .into_iter()
            .map(|k| proto::Key { address: addr_bytes(k.address), weight: k.weight })
            .collect(),
    }
}

pub fn account_permission_update_to_proto(
    p: AccountPermissionUpdateContract,
) -> proto::AccountPermissionUpdateContract {
    use proto::permission::PermissionType;

    let owner = p.owner.map(|perm| permission_to_proto(perm, PermissionType::Owner));
    let witness = p.witness.map(|perm| permission_to_proto(perm, PermissionType::Witness));
    let actives = p
        .actives
        .into_iter()
        .map(|perm| permission_to_proto(perm, PermissionType::Active))
        .collect();

    proto::AccountPermissionUpdateContract {
        owner_address: addr_bytes(p.owner_address),
        owner,
        witness,
        actives,
    }
}

pub fn create_smart_contract_to_proto(p: CreateSmartContract) -> proto::CreateSmartContract {
    proto::CreateSmartContract {
        owner_address: addr_bytes(p.owner_address),
        new_contract: Some(proto::SmartContract {
            origin_address: addr_bytes(p.owner_address),
            contract_address: vec![],
            abi: Some(abi::to_proto(p.abi)),
            bytecode: p.bytecode.into(),
            call_value: p.call_value.as_sun(),
            consume_user_resource_percent: p.consume_user_resource_percent,
            name: p.name,
            origin_energy_limit: p.origin_energy_limit,
            code_hash: vec![],
            trx_hash: vec![],
            version: 0,
        }),
        call_token_value: 0,
        token_id: 0,
    }
}

pub fn asset_issue_to_proto(p: AssetIssueContract) -> proto::AssetIssueContract {
    proto::AssetIssueContract {
        owner_address: addr_bytes(p.owner_address),
        name: str_bytes(p.name),
        abbr: str_bytes(p.abbr),
        description: str_bytes(p.description),
        url: str_bytes(p.url),
        total_supply: p.total_supply,
        precision: p.precision,
        trx_num: p.trx_num,
        num: p.num,
        start_time: p.start_time,
        end_time: p.end_time,
        free_asset_net_limit: p.free_asset_net_limit,
        public_free_asset_net_limit: p.public_free_asset_net_limit,
        frozen_supply: p
            .frozen_supply
            .into_iter()
            .map(|f| proto::asset_issue_contract::FrozenSupply {
                frozen_amount: f.frozen_amount,
                frozen_days: f.frozen_days,
            })
            .collect(),
        ..Default::default()
    }
}

pub fn transfer_asset_to_proto(p: TransferAssetContract) -> proto::TransferAssetContract {
    proto::TransferAssetContract {
        // After the ALLOW_SAME_TOKEN_NAME proposal, asset_name holds the numeric ID as bytes.
        asset_name: str_bytes(p.token_id),
        owner_address: addr_bytes(p.owner_address),
        to_address: addr_bytes(p.to_address),
        amount: p.amount,
    }
}

pub fn participate_asset_issue_to_proto(
    p: ParticipateAssetIssueContract,
) -> proto::ParticipateAssetIssueContract {
    proto::ParticipateAssetIssueContract {
        owner_address: addr_bytes(p.owner_address),
        to_address: addr_bytes(p.to_address),
        // After ALLOW_SAME_TOKEN_NAME the asset_name field holds the numeric ID as bytes.
        asset_name: str_bytes(p.token_id),
        amount: p.amount,
    }
}

pub fn unfreeze_asset_to_proto(p: UnfreezeAssetContract) -> proto::UnfreezeAssetContract {
    proto::UnfreezeAssetContract { owner_address: addr_bytes(p.owner_address) }
}

pub fn update_asset_to_proto(p: UpdateAssetContract) -> proto::UpdateAssetContract {
    proto::UpdateAssetContract {
        owner_address: addr_bytes(p.owner_address),
        description: str_bytes(p.description),
        url: str_bytes(p.url),
        new_limit: p.new_limit,
        new_public_limit: p.new_public_limit,
    }
}

pub fn create_account_to_proto(p: CreateAccountContract) -> proto::AccountCreateContract {
    proto::AccountCreateContract {
        owner_address: addr_bytes(p.owner_address),
        account_address: addr_bytes(p.account_address),
        r#type: 0, // Normal account
    }
}

pub fn vote_witness_to_proto(p: VoteWitnessContract) -> proto::VoteWitnessContract {
    proto::VoteWitnessContract {
        owner_address: addr_bytes(p.owner_address),
        votes: p
            .votes
            .into_iter()
            .map(|v| proto::vote_witness_contract::Vote {
                vote_address: addr_bytes(v.vote_address),
                vote_count: v.vote_count,
            })
            .collect(),
        support: false,
    }
}

pub fn update_account_to_proto(p: UpdateAccountContract) -> proto::AccountUpdateContract {
    proto::AccountUpdateContract {
        account_name: str_bytes(p.name),
        owner_address: addr_bytes(p.owner_address),
    }
}

/// Returns `Ok(None)` when the token was not found (empty `id` field).
pub fn asset_info_from_proto(
    a: proto::AssetIssueContract,
) -> Result<Option<AssetInfo>, ResponseError> {
    if a.id.is_empty() {
        return Ok(None);
    }
    let owner = addr(a.owner_address)?;
    Ok(Some(AssetInfo {
        id: a.id,
        name: String::from_utf8_lossy(&a.name).into_owned(),
        abbr: String::from_utf8_lossy(&a.abbr).into_owned(),
        decimals: a.precision,
        owner,
        total_supply: a.total_supply,
        url: String::from_utf8_lossy(&a.url).into_owned(),
    }))
}

pub fn delegated_resource_index_from_proto(
    idx: proto::DelegatedResourceAccountIndex,
) -> Result<DelegatedResourceIndex, ResponseError> {
    Ok(DelegatedResourceIndex {
        account: addr(idx.account)?,
        from_accounts: idx.from_accounts.into_iter().filter_map(|b| addr(b).ok()).collect(),
        to_accounts: idx.to_accounts.into_iter().filter_map(|b| addr(b).ok()).collect(),
    })
}

pub fn proposal_create_to_proto(p: ProposalCreateContract) -> proto::ProposalCreateContract {
    proto::ProposalCreateContract {
        owner_address: addr_bytes(p.owner_address),
        parameters: p.parameters,
    }
}

pub fn proposal_approve_to_proto(p: ProposalApproveContract) -> proto::ProposalApproveContract {
    proto::ProposalApproveContract {
        owner_address: addr_bytes(p.owner_address),
        proposal_id: p.proposal_id,
        is_add_approval: p.is_add_approval,
    }
}

pub fn proposal_delete_to_proto(p: ProposalDeleteContract) -> proto::ProposalDeleteContract {
    proto::ProposalDeleteContract {
        owner_address: addr_bytes(p.owner_address),
        proposal_id: p.proposal_id,
    }
}

pub fn proposal_from_proto(p: proto::Proposal) -> ProposalInfo {
    let proposer_address = if p.proposer_address.is_empty() {
        None
    } else {
        Address::from_slice(&p.proposer_address).ok()
    };
    let approvals = p.approvals.into_iter().filter_map(|b| Address::from_slice(&b).ok()).collect();
    ProposalInfo {
        proposal_id: p.proposal_id,
        proposer_address,
        parameters: p.parameters,
        expiration_time: p.expiration_time,
        create_time: p.create_time,
        approvals,
        state: ProposalState::from(p.state),
    }
}

pub fn create_witness_to_proto(p: CreateWitnessContract) -> proto::WitnessCreateContract {
    proto::WitnessCreateContract {
        owner_address: addr_bytes(p.owner_address),
        url: str_bytes(p.url),
    }
}

pub fn update_witness_to_proto(p: UpdateWitnessContract) -> proto::WitnessUpdateContract {
    proto::WitnessUpdateContract {
        owner_address: addr_bytes(p.owner_address),
        update_url: str_bytes(p.update_url),
    }
}

pub fn update_brokerage_to_proto(p: UpdateBrokerageContract) -> proto::UpdateBrokerageContract {
    proto::UpdateBrokerageContract {
        owner_address: addr_bytes(p.owner_address),
        brokerage: p.brokerage,
    }
}

pub fn set_account_id_to_proto(p: SetAccountIdContract) -> proto::SetAccountIdContract {
    proto::SetAccountIdContract {
        account_id: str_bytes(p.account_id),
        owner_address: addr_bytes(p.owner_address),
    }
}

pub fn clear_contract_abi_to_proto(p: ClearContractAbiContract) -> proto::ClearAbiContract {
    proto::ClearAbiContract {
        owner_address: addr_bytes(p.owner_address),
        contract_address: addr_bytes(p.contract_address),
    }
}

pub fn update_setting_to_proto(p: UpdateSettingContract) -> proto::UpdateSettingContract {
    proto::UpdateSettingContract {
        owner_address: addr_bytes(p.owner_address),
        contract_address: addr_bytes(p.contract_address),
        consume_user_resource_percent: p.consume_user_resource_percent,
    }
}

pub fn update_energy_limit_to_proto(
    p: UpdateEnergyLimitContract,
) -> proto::UpdateEnergyLimitContract {
    proto::UpdateEnergyLimitContract {
        owner_address: addr_bytes(p.owner_address),
        contract_address: addr_bytes(p.contract_address),
        origin_energy_limit: p.origin_energy_limit,
    }
}

/// Convert a plain `Transaction` proto into a `RawTransaction`.
pub fn raw_from_plain(tx: proto::Transaction) -> Result<RawTransaction, ResponseError> {
    use prost::Message as _;

    RawTransaction::from_node_encoded(tx.encode_to_vec(), &[])
}

pub fn exchange_create_to_proto(p: ExchangeCreateContract) -> proto::ExchangeCreateContract {
    proto::ExchangeCreateContract {
        owner_address: addr_bytes(p.owner_address),
        first_token_id: str_bytes(p.first_token_id),
        first_token_balance: p.first_token_balance,
        second_token_id: str_bytes(p.second_token_id),
        second_token_balance: p.second_token_balance,
    }
}

pub fn exchange_inject_to_proto(p: ExchangeInjectContract) -> proto::ExchangeInjectContract {
    proto::ExchangeInjectContract {
        owner_address: addr_bytes(p.owner_address),
        exchange_id: p.exchange_id,
        token_id: str_bytes(p.token_id),
        quant: p.quant,
    }
}

pub fn exchange_withdraw_to_proto(p: ExchangeWithdrawContract) -> proto::ExchangeWithdrawContract {
    proto::ExchangeWithdrawContract {
        owner_address: addr_bytes(p.owner_address),
        exchange_id: p.exchange_id,
        token_id: str_bytes(p.token_id),
        quant: p.quant,
    }
}

pub fn exchange_transaction_to_proto(
    p: ExchangeTransactionContract,
) -> proto::ExchangeTransactionContract {
    proto::ExchangeTransactionContract {
        owner_address: addr_bytes(p.owner_address),
        exchange_id: p.exchange_id,
        token_id: str_bytes(p.token_id),
        quant: p.quant,
        expected: p.expected,
    }
}

pub fn exchange_info_from_proto(e: proto::Exchange) -> Result<ExchangeInfo, ResponseError> {
    Ok(ExchangeInfo {
        exchange_id: e.exchange_id,
        creator_address: addr(e.creator_address)?,
        create_time: e.create_time,
        first_token_id: String::from_utf8_lossy(&e.first_token_id).into_owned(),
        first_token_balance: e.first_token_balance,
        second_token_id: String::from_utf8_lossy(&e.second_token_id).into_owned(),
        second_token_balance: e.second_token_balance,
    })
}

pub fn market_sell_asset_to_proto(p: MarketSellAssetContract) -> proto::MarketSellAssetContract {
    proto::MarketSellAssetContract {
        owner_address: addr_bytes(p.owner_address),
        sell_token_id: str_bytes(p.sell_token_id),
        sell_token_quantity: p.sell_token_quantity,
        buy_token_id: str_bytes(p.buy_token_id),
        buy_token_quantity: p.buy_token_quantity,
    }
}

pub fn market_cancel_order_to_proto(
    p: MarketCancelOrderContract,
) -> proto::MarketCancelOrderContract {
    proto::MarketCancelOrderContract {
        owner_address: addr_bytes(p.owner_address),
        order_id: p.order_id.as_slice().to_vec(),
    }
}

pub fn market_order_from_proto(o: proto::MarketOrder) -> Result<MarketOrderInfo, ResponseError> {
    let state = match o.state {
        1 => MarketOrderState::Inactive,
        2 => MarketOrderState::Canceled,
        _ => MarketOrderState::Active,
    };
    let order_id: [u8; 32] = o
        .order_id
        .try_into()
        .map_err(|_| ResponseError::Malformed("market order id must be 32 bytes".into()))?;
    Ok(MarketOrderInfo {
        order_id: B256::from(order_id),
        owner_address: addr(o.owner_address)?,
        create_time: o.create_time,
        sell_token_id: String::from_utf8_lossy(&o.sell_token_id).into_owned(),
        sell_token_quantity: o.sell_token_quantity,
        buy_token_id: String::from_utf8_lossy(&o.buy_token_id).into_owned(),
        buy_token_quantity: o.buy_token_quantity,
        sell_token_quantity_remain: o.sell_token_quantity_remain,
        sell_token_quantity_return: o.sell_token_quantity_return,
        state,
    })
}

pub fn market_order_pair_from_proto(p: proto::MarketOrderPair) -> MarketOrderPair {
    MarketOrderPair {
        sell_token_id: String::from_utf8_lossy(&p.sell_token_id).into_owned(),
        buy_token_id: String::from_utf8_lossy(&p.buy_token_id).into_owned(),
    }
}

pub fn market_price_from_proto(p: proto::MarketPrice) -> MarketPrice {
    MarketPrice {
        sell_token_quantity: p.sell_token_quantity,
        buy_token_quantity: p.buy_token_quantity,
    }
}

pub fn sign_weight_from_proto(
    w: proto::TransactionSignWeight,
) -> Result<SignWeight, ResponseError> {
    let approved_list = w
        .approved_list
        .into_iter()
        .map(|bytes| {
            Address::from_slice(&bytes)
                .map_err(|e| ResponseError::Malformed(format!("bad address: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let required_weight = w.permission.as_ref().map(|p| p.threshold).unwrap_or(0);

    let result = w.result.as_ref().map(|r| r.message.clone()).unwrap_or_default();

    Ok(SignWeight { approved_list, current_weight: w.current_weight, required_weight, result })
}

pub fn node_info_from_proto(info: proto::NodeInfo) -> NodeInfo {
    NodeInfo {
        block: info.block,
        solidity_block: info.solidity_block,
        peer_num: info.current_connect_count,
    }
}

/// The reachable peers from a node list, skipping entries with no address.
pub fn node_addresses_from_proto(list: proto::NodeList) -> Vec<NodeAddress> {
    list.nodes
        .into_iter()
        .filter_map(|n| {
            n.address.map(|a| NodeAddress {
                host: String::from_utf8_lossy(&a.host).into_owned(),
                port: a.port,
            })
        })
        .collect()
}

/// `DynamicProperties` carries only the last solidified block number, so the
/// head fields it has nothing to say about are left empty rather than guessed at.
pub fn chain_properties_from_proto(props: proto::DynamicProperties) -> ChainProperties {
    ChainProperties {
        head_block_id: String::new(),
        head_block_num: props.last_solidity_block_num,
        head_block_time_stamp: 0,
    }
}

/// `AccountNetMessage` covers bandwidth only; the energy fields belong to
/// `AccountResourceMessage` and stay zero here.
pub fn account_net_from_proto(msg: proto::AccountNetMessage) -> AccountNet {
    AccountNet {
        free_net_used: msg.free_net_used,
        free_net_limit: msg.free_net_limit,
        net_used: msg.net_used,
        net_limit: msg.net_limit,
        total_net_weight: msg.total_net_weight,
        energy_used: 0,
        energy_limit: 0,
        total_energy_weight: 0,
    }
}

/// Addresses from a witness or delegate listing.
pub fn addresses_from_proto(raw: Vec<Vec<u8>>) -> Result<Vec<Address>, ResponseError> {
    raw.into_iter()
        .map(|bytes| {
            Address::from_slice(&bytes)
                .map_err(|e| ResponseError::Malformed(format!("bad address: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use tronz_abi::{TronAbi, TronAbiEntry, TronAbiEntryType, TronAbiStateMutability};
    use tronz_primitives::{Address, Bytes, Trx};

    use super::*;
    use crate::types::{AccountPermissions, ContractKind};

    #[test]
    fn deploy_contract_includes_typed_abi() {
        let abi = TronAbi {
            entries: vec![TronAbiEntry {
                entry_type: TronAbiEntryType::Function,
                name: "totalSupply".into(),
                constant: true,
                state_mutability: TronAbiStateMutability::View,
                ..Default::default()
            }],
        };
        let owner = Address::from_slice(&[0x41; 21]).unwrap();
        let request = CreateSmartContract {
            owner_address: owner,
            bytecode: Bytes::from_static(&[0x60, 0x00]),
            abi,
            call_value: Trx::ZERO,
            consume_user_resource_percent: 100,
            origin_energy_limit: 10_000_000,
            name: "Token".into(),
        };

        let proto = create_smart_contract_to_proto(request);
        let stored = proto.new_contract.unwrap().abi.unwrap();
        assert_eq!(stored.entrys.len(), 1);
        assert_eq!(stored.entrys[0].name, "totalSupply");
    }

    #[test]
    fn receipt_failure_overrides_default_top_level_result() {
        let info = proto::TransactionInfo {
            id: vec![7; 32],
            result: 0,
            receipt: Some(proto::ResourceReceipt { result: 2, ..Default::default() }),
            ..Default::default()
        };

        let decoded = transaction_info_from_proto(info).unwrap().unwrap();
        assert_eq!(decoded.status, TxStatus::Failed);
        assert_eq!(decoded.contract_result, ContractResult::Revert);
        assert!(!decoded.is_success());
    }

    #[test]
    fn transaction_info_maps_all_public_fields() {
        let contract_address = Address::from_evm_bytes([2; 20]);
        let log_address = Address::from_evm_bytes([3; 20]);
        let topic = B256::from([4; 32]);
        let info = proto::TransactionInfo {
            id: vec![1; 32],
            block_number: 123,
            block_time_stamp: 456,
            contract_address: contract_address.as_bytes().to_vec(),
            receipt: Some(proto::ResourceReceipt {
                energy_fee: 10,
                energy_usage_total: 20,
                net_usage: 30,
                net_fee: 40,
                result: 2,
                ..Default::default()
            }),
            log: vec![proto::transaction_info::Log {
                address: log_address.as_evm_bytes().to_vec(),
                topics: vec![topic.as_slice().to_vec()],
                data: vec![5, 6, 7].into(),
            }],
            result: 1,
            res_message: b"execution reverted".to_vec(),
            ..Default::default()
        };

        let decoded = transaction_info_from_proto(info).unwrap().unwrap();
        assert_eq!(decoded.tx_id, TxId::from([1; 32]));
        assert_eq!(decoded.block_number, 123);
        assert_eq!(decoded.block_timestamp, 456);
        assert_eq!(decoded.status, TxStatus::Failed);
        assert_eq!(decoded.energy_usage_total, 20);
        assert_eq!(decoded.energy_fee, Trx::from_sun_unchecked(10));
        assert_eq!(decoded.net_usage, 30);
        assert_eq!(decoded.net_fee, Trx::from_sun_unchecked(40));
        assert_eq!(decoded.contract_result, ContractResult::Revert);
        assert_eq!(decoded.contract_address, Some(contract_address));
        assert_eq!(decoded.logs.len(), 1);
        assert_eq!(decoded.logs[0].address, log_address);
        assert_eq!(decoded.logs[0].topics(), [topic]);
        assert_eq!(decoded.logs[0].data.as_ref(), &[5, 6, 7]);
        assert_eq!(decoded.revert_reason.as_deref(), Some("execution reverted"));
    }

    #[test]
    fn the_nested_receipt_keeps_the_energy_figures_apart() {
        let info = proto::TransactionInfo {
            id: vec![1; 32],
            fee: 1_500,
            receipt: Some(proto::ResourceReceipt {
                energy_usage: 20,
                origin_energy_usage: 70,
                energy_fee: 10,
                energy_usage_total: 100,
                energy_penalty_total: 5,
                ..Default::default()
            }),
            ..Default::default()
        };

        let decoded = transaction_info_from_proto(info).unwrap().unwrap();

        assert_eq!(decoded.fee, Trx::from_sun_unchecked(1_500));
        assert_eq!(decoded.energy_usage_total, 100, "the flat field stays the total");
        assert_eq!(decoded.energy_usage_total, decoded.receipt.energy_usage_total);
        assert_eq!(decoded.receipt.energy_usage, 20);
        assert_eq!(decoded.receipt.origin_energy_usage, 70);
        assert_eq!(decoded.receipt.energy_fee, Trx::from_sun_unchecked(10));
        assert_eq!(decoded.receipt.energy_usage_total, 100);
        assert_eq!(decoded.receipt.energy_penalty_total, 5);
    }

    #[test]
    fn internal_transactions_carry_every_asset_moved_and_whether_they_stuck() {
        let caller = tron_addr(1);
        let callee = tron_addr(2);
        let info = proto::TransactionInfo {
            id: vec![1; 32],
            internal_transactions: vec![proto::InternalTransaction {
                hash: vec![9; 32],
                caller_address: caller.as_bytes().to_vec(),
                transfer_to_address: callee.as_bytes().to_vec(),
                call_value_info: vec![
                    proto::internal_transaction::CallValueInfo {
                        call_value: 100,
                        token_id: String::new(),
                    },
                    proto::internal_transaction::CallValueInfo {
                        call_value: 7,
                        token_id: "1000001".into(),
                    },
                ],
                note: b"call".to_vec(),
                rejected: true,
                extra: String::new(),
            }],
            ..Default::default()
        };

        let decoded = transaction_info_from_proto(info).unwrap().unwrap();

        let [internal] = decoded.internal_transactions.as_slice() else {
            panic!("expected exactly one internal transaction")
        };
        assert_eq!(internal.hash, TxId::from([9; 32]));
        assert_eq!(internal.caller_address, Some(caller));
        assert_eq!(internal.transfer_to_address, Some(callee));
        assert_eq!(internal.note, "call");
        assert!(internal.rejected);
        assert_eq!(internal.call_values.len(), 2);
        assert_eq!(internal.call_values[0].amount, 100);
        assert!(internal.call_values[0].token_id.is_empty(), "empty token id means TRX");
        assert_eq!(internal.call_values[1].token_id, "1000001");
    }

    #[test]
    fn an_internal_transaction_hash_of_the_wrong_size_is_malformed() {
        let info = proto::TransactionInfo {
            id: vec![1; 32],
            internal_transactions: vec![proto::InternalTransaction {
                hash: vec![9; 31],
                ..Default::default()
            }],
            ..Default::default()
        };

        assert!(matches!(transaction_info_from_proto(info), Err(ResponseError::Malformed(_))));
    }

    #[test]
    fn default_receipt_result_preserves_system_contract_success() {
        let info = proto::TransactionInfo {
            id: vec![7; 32],
            result: 0,
            receipt: Some(proto::ResourceReceipt::default()),
            ..Default::default()
        };

        let decoded = transaction_info_from_proto(info).unwrap().unwrap();
        assert_eq!(decoded.status, TxStatus::Success);
        assert_eq!(decoded.contract_result, ContractResult::Default);
        assert!(decoded.is_success());
    }
    fn tron_addr(fill: u8) -> Address {
        let mut b = [fill; 21];
        b[0] = 0x41;
        Address::from_slice(&b).unwrap()
    }

    #[test]
    fn an_empty_transaction_message_means_the_node_has_no_such_transaction() {
        assert!(signed_tx_lookup(proto::Transaction::default(), &[]).unwrap().is_none());
    }

    #[test]
    fn a_transaction_with_signatures_but_no_raw_data_is_broken_not_missing() {
        let tx = proto::Transaction { signature: vec![vec![7; 65].into()], ..Default::default() };

        assert!(matches!(signed_tx_lookup(tx, &[]), Err(ResponseError::Malformed(_))));
    }

    #[test]
    fn transaction_lookup_checks_the_id_the_caller_requested() {
        let tx = proto::Transaction {
            raw_data: Some(proto::transaction::Raw {
                contract: vec![proto::transaction::Contract::default()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let actual = RawTransaction::from_node_encoded(tx.encode_to_vec(), &[]).unwrap().tx_id();

        assert!(signed_tx_lookup(tx.clone(), actual.as_slice()).unwrap().is_some());
        assert!(matches!(
            signed_tx_lookup(tx, &[7; 32]),
            Err(ResponseError::Malformed(message)) if message.contains("txid")
        ));
    }

    #[test]
    fn check_return_accepts_none_and_success() {
        assert!(check_return(None).is_ok());
        assert!(check_return(Some(proto::Return { result: true, ..Default::default() })).is_ok());
    }

    #[test]
    fn check_return_maps_failure_to_node_error() {
        let ret =
            proto::Return { result: false, message: b"bad sig".to_vec(), ..Default::default() };
        match check_return(Some(ret)) {
            Err(ResponseError::NodeError(msg)) => assert_eq!(msg, "bad sig"),
            other => panic!("expected NodeError, got {other:?}"),
        }
    }

    #[test]
    fn b256_requires_exactly_32_bytes() {
        assert_eq!(b256(vec![9; 32]), B256::from([9u8; 32]));
        assert_eq!(b256(vec![1, 2, 3]), B256::ZERO);
        assert_eq!(b256(vec![9; 33]), B256::ZERO);
        assert_eq!(b256(Vec::new()), B256::ZERO);
    }

    #[test]
    fn opt_addr_handles_empty_valid_and_malformed() {
        assert_eq!(opt_addr(Vec::new()), None);
        assert_eq!(opt_addr(tron_addr(0x11).as_bytes().to_vec()), Some(tron_addr(0x11)));
        assert_eq!(opt_addr(vec![0x99; 21]), None);
        assert_eq!(opt_addr(vec![0x41; 5]), None);
    }

    #[test]
    fn log_addr_accepts_both_evm_and_tron_layouts() {
        assert_eq!(log_addr(vec![7; 20]).unwrap(), Address::from_evm_bytes([7; 20]));
        let a = tron_addr(0x22);
        assert_eq!(log_addr(a.as_bytes().to_vec()).unwrap(), a);
        assert!(log_addr(vec![1, 2, 3]).is_err());
    }

    #[test]
    fn account_falls_back_to_queried_address_when_not_activated() {
        let queried = tron_addr(0x33);
        let decoded = account_from_proto(proto::Account::default(), queried).unwrap();
        assert_eq!(decoded.address, queried);
        assert!(!decoded.is_activated);
    }

    #[test]
    fn account_decodes_resources_votes_and_skips_bad_entries() {
        let addr = tron_addr(0x41);
        let voter = tron_addr(0x51);
        let account = proto::Account {
            address: addr.as_bytes().to_vec(),
            balance: 5_000,
            account_name: b"alice".to_vec(),
            asset_v2: [("1000001".to_string(), 42i64)].into_iter().collect(),
            frozen_v2: vec![
                proto::account::FreezeV2 { r#type: 1, amount: 100 },
                proto::account::FreezeV2 { r#type: 99, amount: 7 },
            ],
            unfrozen_v2: vec![proto::account::UnFreezeV2 {
                r#type: 0,
                unfreeze_amount: 200,
                unfreeze_expire_time: 999,
            }],
            votes: vec![
                proto::Vote { vote_address: voter.as_bytes().to_vec(), vote_count: 7 },
                proto::Vote { vote_address: vec![1, 2, 3], vote_count: 9 },
            ],
            ..Default::default()
        };

        let decoded = account_from_proto(account, tron_addr(0x01)).unwrap();
        assert!(decoded.is_activated);
        assert_eq!(decoded.address, addr);
        assert_eq!(decoded.balance, Trx::from_sun_unchecked(5_000));
        assert_eq!(decoded.name, "alice");
        assert_eq!(decoded.trc10_balances.get("1000001"), Some(&42));
        assert_eq!(decoded.frozen_v2.len(), 1);
        assert_eq!(decoded.frozen_v2[0].resource, tronz_primitives::ResourceCode::Energy);
        assert_eq!(decoded.frozen_v2[0].amount, Trx::from_sun_unchecked(100));
        assert_eq!(decoded.unfrozen_v2.len(), 1);
        assert_eq!(decoded.unfrozen_v2[0].expire_time_ms, 999);
        assert_eq!(decoded.votes.len(), 1);
        assert_eq!(decoded.votes[0].vote_address, voter);
        assert_eq!(decoded.votes[0].vote_count, 7);
    }

    #[test]
    fn account_resource_scales_tron_power_to_sun() {
        let msg = proto::AccountResourceMessage {
            free_net_used: 1,
            free_net_limit: 2,
            net_used: 3,
            net_limit: 4,
            energy_used: 5,
            energy_limit: 6,
            tron_power_used: 5,
            tron_power_limit: 10,
            ..Default::default()
        };
        let r = account_resource_from_proto(msg);
        assert_eq!(r.free_bandwidth_used, 1);
        assert_eq!(r.energy_limit, 6);
        assert_eq!(r.tron_power_used.as_sun(), 5_000_000);
        assert_eq!(r.tron_power_limit.as_sun(), 10_000_000);
    }

    #[test]
    fn transaction_info_returns_none_for_unindexed_tx() {
        assert!(transaction_info_from_proto(proto::TransactionInfo::default()).unwrap().is_none());
    }

    #[test]
    fn transaction_info_maps_contract_result_codes() {
        let cases = [
            (1, ContractResult::Success, TxStatus::Success),
            (2, ContractResult::Revert, TxStatus::Failed),
            (10, ContractResult::OutOfEnergy, TxStatus::Failed),
            (7, ContractResult::StackTooLarge, TxStatus::Failed),
            (15, ContractResult::InvalidCode, TxStatus::Failed),
            (99, ContractResult::Other(99), TxStatus::Failed),
            (0, ContractResult::Default, TxStatus::Success),
        ];
        for (code, expected_result, expected_status) in cases {
            let info = proto::TransactionInfo {
                id: vec![1; 32],
                result: 0,
                receipt: Some(proto::ResourceReceipt { result: code, ..Default::default() }),
                ..Default::default()
            };
            let decoded = transaction_info_from_proto(info).unwrap().unwrap();
            assert_eq!(decoded.contract_result, expected_result, "code {code}");
            assert_eq!(decoded.status, expected_status, "code {code}");
        }
    }

    #[test]
    fn transaction_info_empty_res_message_is_none() {
        let info = proto::TransactionInfo { id: vec![1; 32], ..Default::default() };
        assert_eq!(transaction_info_from_proto(info).unwrap().unwrap().revert_reason, None);
    }

    #[test]
    fn constant_result_without_return_has_no_revert() {
        let ext = proto::TransactionExtention {
            constant_result: vec![vec![1, 2, 3].into()],
            energy_used: 50,
            result: None,
            ..Default::default()
        };
        let r = constant_result_from_extention(ext).unwrap();
        assert_eq!(r.output.as_ref(), &[1, 2, 3]);
        assert_eq!(r.energy_used, 50);
        assert_eq!(r.revert_reason, None);
    }

    #[test]
    fn constant_result_success_return_has_no_revert() {
        let ext = proto::TransactionExtention {
            constant_result: vec![vec![9].into()],
            result: Some(proto::Return { result: true, ..Default::default() }),
            ..Default::default()
        };
        assert_eq!(constant_result_from_extention(ext).unwrap().revert_reason, None);
    }

    #[test]
    fn constant_result_failure_with_no_output_is_node_error() {
        let ext = proto::TransactionExtention {
            constant_result: vec![],
            result: Some(proto::Return {
                result: false,
                message: b"boom".to_vec(),
                ..Default::default()
            }),
            ..Default::default()
        };
        match constant_result_from_extention(ext) {
            Err(ResponseError::NodeError(msg)) => assert_eq!(msg, "boom"),
            other => panic!("expected NodeError, got {other:?}"),
        }
    }

    #[test]
    fn constant_result_revert_with_output_keeps_reason_and_data() {
        let ext = proto::TransactionExtention {
            constant_result: vec![vec![0xde, 0xad].into()],
            result: Some(proto::Return {
                result: false,
                message: b"reverted".to_vec(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let r = constant_result_from_extention(ext).unwrap();
        assert_eq!(r.revert_reason.as_deref(), Some("reverted"));
        assert_eq!(r.output.as_ref(), &[0xde, 0xad]);
    }

    #[test]
    fn witness_requires_address() {
        assert!(witness_from_proto(proto::Witness::default()).is_none());
        let w = proto::Witness {
            address: tron_addr(0x61).as_bytes().to_vec(),
            vote_count: 42,
            url: "https://sr.example".into(),
            total_produced: 100,
            total_missed: 3,
            is_jobs: true,
            ..Default::default()
        };
        let info = witness_from_proto(w).unwrap();
        assert_eq!(info.vote_count, 42);
        assert!(info.is_active);
    }

    #[test]
    fn delegated_resource_maps_fields_and_rejects_bad_address() {
        let d = proto::DelegatedResource {
            from: tron_addr(0x71).as_bytes().to_vec(),
            to: tron_addr(0x72).as_bytes().to_vec(),
            frozen_balance_for_bandwidth: 111,
            frozen_balance_for_energy: 222,
            expire_time_for_bandwidth: 333,
            expire_time_for_energy: 444,
        };
        let r = delegated_resource_from_proto(d).unwrap();
        assert_eq!(r.bandwidth_amount, Trx::from_sun_unchecked(111));
        assert_eq!(r.energy_amount, Trx::from_sun_unchecked(222));
        assert_eq!(r.energy_expire_time_ms, 444);

        let bad = proto::DelegatedResource {
            to: tron_addr(0x72).as_bytes().to_vec(),
            ..Default::default()
        };
        assert!(delegated_resource_from_proto(bad).is_err());
    }

    #[test]
    fn delegated_resource_index_filters_bad_accounts() {
        let idx = proto::DelegatedResourceAccountIndex {
            account: tron_addr(0x81).as_bytes().to_vec(),
            from_accounts: vec![tron_addr(0x82).as_bytes().to_vec(), vec![1, 2, 3]],
            to_accounts: vec![vec![], tron_addr(0x83).as_bytes().to_vec()],
            ..Default::default()
        };
        let r = delegated_resource_index_from_proto(idx).unwrap();
        assert_eq!(r.from_accounts, vec![tron_addr(0x82)]);
        assert_eq!(r.to_accounts, vec![tron_addr(0x83)]);

        let bad = proto::DelegatedResourceAccountIndex::default();
        assert!(delegated_resource_index_from_proto(bad).is_err());
    }

    #[test]
    fn asset_info_returns_none_without_id() {
        assert!(asset_info_from_proto(proto::AssetIssueContract::default()).unwrap().is_none());
        let a = proto::AssetIssueContract {
            id: "1000001".into(),
            name: b"BitTorrent".to_vec(),
            abbr: b"BTT".to_vec(),
            precision: 6,
            owner_address: tron_addr(0x91).as_bytes().to_vec(),
            total_supply: 1_000_000,
            url: b"https://bt.io".to_vec(),
            ..Default::default()
        };
        let info = asset_info_from_proto(a).unwrap().unwrap();
        assert_eq!(info.id, "1000001");
        assert_eq!(info.name, "BitTorrent");
        assert_eq!(info.decimals, 6);
        assert_eq!(info.owner, tron_addr(0x91));
    }

    #[test]
    fn proposal_decodes_state_and_filters_addresses() {
        let p = proto::Proposal {
            proposal_id: 12,
            proposer_address: vec![], // empty → None
            approvals: vec![tron_addr(0xa1).as_bytes().to_vec(), vec![0xff; 3]],
            state: 2, // Approved
            ..Default::default()
        };
        let info = proposal_from_proto(p);
        assert_eq!(info.proposal_id, 12);
        assert_eq!(info.proposer_address, None);
        assert_eq!(info.approvals, vec![tron_addr(0xa1)]);
        assert_eq!(info.state, ProposalState::Approved);
    }

    #[test]
    fn market_order_maps_state_variants() {
        for (code, expected) in [
            (1, MarketOrderState::Inactive),
            (2, MarketOrderState::Canceled),
            (0, MarketOrderState::Active),
        ] {
            let o = proto::MarketOrder {
                order_id: vec![3; 32],
                owner_address: tron_addr(0xb1).as_bytes().to_vec(),
                state: code,
                ..Default::default()
            };
            assert_eq!(market_order_from_proto(o).unwrap().state, expected, "code {code}");
        }
    }

    #[test]
    fn market_order_rejects_non_32_byte_id() {
        let o = proto::MarketOrder {
            order_id: vec![3; 16],
            owner_address: tron_addr(0xb1).as_bytes().to_vec(),
            ..Default::default()
        };
        assert!(market_order_from_proto(o).is_err());
    }

    #[test]
    fn exchange_info_maps_fields_and_rejects_bad_creator() {
        let e = proto::Exchange {
            exchange_id: 5,
            creator_address: tron_addr(0xc1).as_bytes().to_vec(),
            create_time: 100,
            first_token_id: b"_".to_vec(),
            first_token_balance: 1_000,
            second_token_id: b"1000001".to_vec(),
            second_token_balance: 2_000,
        };
        let info = exchange_info_from_proto(e).unwrap();
        assert_eq!(info.exchange_id, 5);
        assert_eq!(info.creator_address, tron_addr(0xc1));
        assert_eq!(info.first_token_id, "_");
        assert_eq!(info.second_token_balance, 2_000);

        let bad = proto::Exchange { creator_address: vec![], ..Default::default() };
        assert!(exchange_info_from_proto(bad).is_err());
    }

    #[test]
    fn sign_weight_defaults_when_permission_and_result_absent() {
        let w = proto::TransactionSignWeight {
            approved_list: vec![tron_addr(0xd1).as_bytes().to_vec()],
            current_weight: 3,
            permission: None,
            result: None,
            ..Default::default()
        };
        let sw = sign_weight_from_proto(w).unwrap();
        assert_eq!(sw.approved_list, vec![tron_addr(0xd1)]);
        assert_eq!(sw.current_weight, 3);
        assert_eq!(sw.required_weight, 0);
        assert_eq!(sw.result, "");
        let bad = proto::TransactionSignWeight {
            approved_list: vec![vec![1, 2, 3]],
            ..Default::default()
        };
        assert!(sign_weight_from_proto(bad).is_err());
    }

    #[test]
    fn transfer_to_proto_maps_addresses_and_amount() {
        let p = TransferContract {
            owner_address: tron_addr(0xe1),
            to_address: tron_addr(0xe2),
            amount: Trx::from_sun_unchecked(1_500),
        };
        let out = transfer_to_proto(p);
        assert_eq!(out.owner_address, tron_addr(0xe1).as_bytes().to_vec());
        assert_eq!(out.to_address, tron_addr(0xe2).as_bytes().to_vec());
        assert_eq!(out.amount, 1_500);
    }

    #[test]
    fn account_permission_update_grants_only_the_listed_operations() {
        use proto::permission::PermissionType;

        let perm = |id: i32, operations: &[ContractKind]| Permission {
            id,
            permission_name: format!("p{id}"),
            threshold: 1,
            keys: vec![PermissionKey { address: tron_addr(0xf1), weight: 1 }],
            operations: OperationSet::try_from(operations).unwrap(),
        };
        let contract = AccountPermissionUpdateContract {
            owner_address: tron_addr(0xf0),
            owner: Some(perm(0, &[])),
            witness: Some(perm(1, &[])),
            actives: vec![perm(2, &[ContractKind::Transfer, ContractKind::TriggerSmartContract])],
        };

        let out = account_permission_update_to_proto(contract);
        assert_eq!(out.owner_address, tron_addr(0xf0).as_bytes().to_vec());

        let owner = out.owner.unwrap();
        assert_eq!(owner.r#type, PermissionType::Owner as i32);
        assert!(owner.operations.is_empty());
        let witness = out.witness.unwrap();
        assert_eq!(witness.r#type, PermissionType::Witness as i32);
        assert!(witness.operations.is_empty());

        let active = &out.actives[0];
        assert_eq!(active.r#type, PermissionType::Active as i32);
        assert_eq!(active.operations.len(), 32);
        assert_eq!(active.keys[0].address, tron_addr(0xf1).as_bytes().to_vec());
        assert_eq!(&active.operations[0..4], &[0b10, 0x00, 0x00, 0x80]);
        assert!(active.operations[4..].iter().all(|&b| b == 0));
    }

    #[test]
    fn permissions_report_the_operations_a_node_granted_them() {
        let proto_perm = |id: i32, operations: Vec<u8>| proto::Permission {
            id,
            permission_name: format!("p{id}"),
            threshold: 1,
            keys: vec![proto::Key { address: tron_addr(0xf1).as_bytes().to_vec(), weight: 1 }],
            operations,
            ..Default::default()
        };
        let mut active_ops = vec![0u8; 32];
        active_ops[0] = 0b10;
        active_ops[5] = 1 << 6;
        active_ops[25] = 1;

        let owner = permission_from_proto(proto_perm(0, Vec::new())).unwrap();
        assert!(owner.operations.is_empty());

        let active = permission_from_proto(proto_perm(2, active_ops)).unwrap();
        assert_eq!(
            active.operations.iter().collect::<Vec<_>>(),
            vec![
                ContractKind::Transfer,
                ContractKind::AccountPermissionUpdate,
                ContractKind::Unknown(200),
            ]
        );
        let permissions =
            AccountPermissions { owner: Some(owner), witness: None, actives: vec![active] };
        assert!(permissions.allows(0, ContractKind::AccountPermissionUpdate));
        assert!(permissions.allows(2, ContractKind::Transfer));
        assert!(!permissions.allows(2, ContractKind::TriggerSmartContract));
    }

    #[test]
    fn vote_witness_to_proto_maps_votes() {
        let p = VoteWitnessContract {
            owner_address: tron_addr(0x12),
            votes: vec![crate::types::SrVote { vote_address: tron_addr(0x13), vote_count: 99 }],
        };
        let out = vote_witness_to_proto(p);
        assert_eq!(out.owner_address, tron_addr(0x12).as_bytes().to_vec());
        assert_eq!(out.votes.len(), 1);
        assert_eq!(out.votes[0].vote_address, tron_addr(0x13).as_bytes().to_vec());
        assert_eq!(out.votes[0].vote_count, 99);
        assert!(!out.support);
    }

    #[test]
    fn smart_contract_wrapper_attaches_runtime_bytecode_when_present() {
        let wrapper = proto::SmartContractDataWrapper {
            smart_contract: Some(proto::SmartContract {
                contract_address: tron_addr(0x14).as_bytes().to_vec(),
                name: "Token".into(),
                ..Default::default()
            }),
            runtimecode: vec![0x60, 0x00].into(),
            ..Default::default()
        };
        let info = smart_contract_info_from_wrapper(wrapper);
        assert_eq!(info.name, "Token");
        assert_eq!(info.runtime_bytecode.as_ref().map(|b| b.as_ref()), Some(&[0x60, 0x00][..]));
        let empty = proto::SmartContractDataWrapper {
            smart_contract: Some(proto::SmartContract::default()),
            runtimecode: Vec::<u8>::new().into(),
            ..Default::default()
        };
        assert!(smart_contract_info_from_wrapper(empty).runtime_bytecode.is_none());
    }

    #[test]
    fn raw_from_plain_rejects_a_transaction_with_no_raw_data() {
        let err = raw_from_plain(proto::Transaction::default()).unwrap_err();
        assert!(matches!(err, ResponseError::Malformed(ref m) if m.contains("raw_data")));
        let tx = proto::Transaction {
            raw_data: Some(proto::transaction::Raw {
                expiration: 10,
                timestamp: 20,
                ..Default::default()
            }),
            ..Default::default()
        };
        let raw = raw_from_plain(tx).unwrap();
        assert_ne!(raw.tx_id().as_slice(), &[0u8; 32]);
        assert_eq!((raw.expiration, raw.timestamp), (10, 20));
    }
    const USDT_MAINNET: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";
    fn fresh_unused_address() -> Address {
        Address::from_evm_bytes([
            0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
        ])
    }
    fn fixture(name: &str) -> Option<Vec<u8>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/transport/grpc/fixtures")
            .join(name);
        std::fs::read(path).ok()
    }

    #[test]
    fn replay_tx_info_success() {
        use prost::Message as _;
        let Some(bytes) = fixture("tx_info_success.bin") else { return };
        let info = proto::TransactionInfo::decode(&bytes[..]).unwrap();
        let decoded = transaction_info_from_proto(info).unwrap().unwrap();
        assert!(decoded.is_success(), "captured success tx should decode as Success");
        assert_ne!(decoded.tx_id, TxId::from([0u8; 32]));
    }

    #[test]
    fn replay_tx_info_reverted() {
        use prost::Message as _;
        let Some(bytes) = fixture("tx_info_reverted.bin") else { return };
        let info = proto::TransactionInfo::decode(&bytes[..]).unwrap();
        let decoded = transaction_info_from_proto(info).unwrap().unwrap();
        assert!(!decoded.is_success(), "captured reverted tx should decode as Failed");
    }

    #[test]
    fn replay_account_activated() {
        use prost::Message as _;
        let Some(bytes) = fixture("account_activated.bin") else { return };
        let queried = Address::from_base58(USDT_MAINNET).unwrap();
        let account = proto::Account::decode(&bytes[..]).unwrap();
        let decoded = account_from_proto(account, queried).unwrap();
        assert!(decoded.is_activated);
        assert_eq!(decoded.address, queried);
    }

    #[test]
    fn replay_account_never_activated() {
        use prost::Message as _;
        let Some(bytes) = fixture("account_never_activated.bin") else { return };
        let queried = fresh_unused_address();
        let account = proto::Account::decode(&bytes[..]).unwrap();
        let decoded = account_from_proto(account, queried).unwrap();
        assert!(!decoded.is_activated);
        assert_eq!(decoded.address, queried);
    }

    #[test]
    fn replay_constant_call_balanceof() {
        use prost::Message as _;
        let Some(bytes) = fixture("constant_call_balanceof.bin") else { return };
        let ext = proto::TransactionExtention::decode(&bytes[..]).unwrap();
        let result = constant_result_from_extention(ext).unwrap();
        assert_eq!(result.output.len(), 32);
    }
}
