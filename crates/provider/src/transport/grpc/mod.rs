//! tonic-backed gRPC transport targeting the TRON full-node WalletClient API.
//!
//! Default endpoint: `https://grpc.trongrid.io:443` (TronGrid mainnet, TLS).
//! For local/private nodes use `http://127.0.0.1:50051` (no TLS).

mod codec;

use std::collections::HashMap;

use futures::future::try_join_all;
use prost::Message as _;
use tonic::{
    metadata::MetadataValue,
    service::Interceptor,
    transport::{Channel, Endpoint},
};
use tronz_primitives::{Address, B256, ResourceCode, Trx, TxId};

use crate::{
    error::TransportErrorKind,
    proto::{
        self, EmptyMessage, database_client::DatabaseClient, wallet_client::WalletClient,
        wallet_extension_client::WalletExtensionClient,
    },
    transport::TronTransport,
    types::{
        AccountInfo, AccountNet, AccountPermissionUpdateContract, AccountResource, AssetInfo,
        AssetIssueContract, BlockInfo, CancelAllUnfreezeV2Contract, ChainProperties,
        ClearContractAbiContract, ConstantCallResult, CreateAccountContract, CreateSmartContract,
        CreateWitnessContract, DelegateResourceContract, DelegatedResource, DelegatedResourceIndex,
        FreezeBalanceV1Contract, FreezeBalanceV2Contract, NodeAddress, NodeInfo,
        ParticipateAssetIssueContract, ProposalApproveContract, ProposalCreateContract,
        ProposalDeleteContract, ProposalInfo, RawTransaction, SetAccountIdContract, SignWeight,
        SignedTransaction, SmartContractInfo, TransactionInfo, TransferAssetContract,
        TransferContract, TriggerSmartContract, UnDelegateResourceContract, UnfreezeAssetContract,
        UnfreezeBalanceV1Contract, UnfreezeBalanceV2Contract, UpdateAccountContract,
        UpdateAssetContract, UpdateBrokerageContract, UpdateEnergyLimitContract,
        UpdateSettingContract, UpdateWitnessContract, VoteWitnessContract, WithdrawBalanceContract,
        WithdrawExpireUnfreezeContract, WitnessInfo,
    },
};

/// TronGrid mainnet gRPC endpoint (TLS).
pub const TRONGRID_MAINNET: &str = "https://grpc.trongrid.io:443";
/// TronGrid Nile testnet gRPC endpoint (plain HTTP/2, no TLS).
///
/// TronGrid's wildcard TLS cert (`*.trongrid.io`) does not cover the
/// three-level hostname `grpc.nile.trongrid.io`, so connect without TLS:
/// ```no_run
/// use tronz_provider::{ProviderBuilder, transport::grpc::TRONGRID_NILE};
/// # async fn run() -> tronz_provider::Result<()> {
/// let provider = ProviderBuilder::new().on_grpc(TRONGRID_NILE).await?;
/// # Ok(()) }
/// ```
pub const TRONGRID_NILE: &str = "http://grpc.nile.trongrid.io:50051";

/// tonic interceptor that injects the TronGrid API key as a request header.
#[derive(Clone)]
struct ApiKeyInterceptor(Option<String>);

impl Interceptor for ApiKeyInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if let Some(ref key) = self.0 {
            match MetadataValue::try_from(key.as_str()) {
                Ok(val) => {
                    req.metadata_mut().insert("tron-pro-api-key", val);
                }
                Err(_) => {
                    // Invalid ASCII — log and continue rather than hard-failing
                    // a potentially valid RPC call.
                    tracing::warn!(
                        "TronGrid API key contains non-ASCII characters; skipping header injection"
                    );
                }
            }
        }
        Ok(req)
    }
}

/// Shorthand for the intercepted wallet client type used throughout this module.
type WalletClientI = WalletClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

/// Shorthand for the intercepted wallet-extension client.
type WalletExtensionClientI =
    WalletExtensionClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

/// Shorthand for the intercepted database client.
type DatabaseClientI =
    DatabaseClient<tonic::codegen::InterceptedService<Channel, ApiKeyInterceptor>>;

/// gRPC transport wrapping a tonic [`Channel`].
///
/// Cheap to clone — the channel is already `Arc`-backed.
#[derive(Clone)]
pub struct GrpcTransport {
    channel: Channel,
    api_key: Option<String>,
}

impl GrpcTransport {
    /// Connect to a TRON gRPC node.
    ///
    /// `uri` may be:
    /// - `"https://grpc.trongrid.io:443"` (TronGrid mainnet, TLS)
    /// - `"http://127.0.0.1:50051"` (local node, plain HTTP/2)
    pub async fn connect(uri: impl AsRef<str>) -> Result<Self, TransportErrorKind> {
        let endpoint = Endpoint::from_shared(uri.as_ref().to_owned())
            .map_err(|e| TransportErrorKind::Malformed(e.to_string()))?;

        #[cfg(feature = "grpc-tls")]
        let endpoint = endpoint
            .tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
            .map_err(TransportErrorKind::Connect)?;

        let channel = endpoint.connect().await?;
        Ok(Self {
            channel,
            api_key: None,
        })
    }

    /// Attach a TronGrid API key (sent as `TRON-PRO-API-KEY` header on each call).
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    fn wallet_client(&self) -> WalletClientI {
        WalletClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    fn wallet_extension_client(&self) -> WalletExtensionClientI {
        WalletExtensionClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    fn database_client(&self) -> DatabaseClientI {
        DatabaseClient::with_interceptor(
            self.channel.clone(),
            ApiKeyInterceptor(self.api_key.clone()),
        )
    }

    /// Check a `Return` message, converting failures to [`TransportErrorKind::NodeError`].
    fn check_return(ret: Option<proto::Return>) -> Result<(), TransportErrorKind> {
        if let Some(r) = ret {
            if !r.result {
                let msg = String::from_utf8_lossy(&r.message).into_owned();
                return Err(TransportErrorKind::NodeError(msg));
            }
        }
        Ok(())
    }

    /// Extract a [`RawTransaction`] from a [`proto::TransactionExtention`].
    fn raw_from_extention(
        ext: proto::TransactionExtention,
    ) -> Result<RawTransaction, TransportErrorKind> {
        Self::check_return(ext.result)?;

        let tx = ext.transaction.ok_or_else(|| {
            TransportErrorKind::Malformed("missing transaction in extention".into())
        })?;

        let (expiration, timestamp) = tx
            .raw_data
            .as_ref()
            .map(|r| (r.expiration, r.timestamp))
            .unwrap_or((0, 0));

        let raw_proto = tx.encode_to_vec();
        RawTransaction::from_proto_extention(ext.txid, raw_proto, expiration, timestamp)
    }
}

/// Decode a lowercase hex string into bytes using only the standard library.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd number of hex digits".into());
    }
    s.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let hi = hex_digit(chunk[0])?;
            let lo = hex_digit(chunk[1])?;
            Ok((hi << 4) | lo)
        })
        .collect()
}

fn hex_digit(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex character: {}", b as char)),
    }
}

impl TronTransport for GrpcTransport {
    type Error = TransportErrorKind;

    // --- Block ---

    async fn get_now_block(&self) -> Result<BlockInfo, Self::Error> {
        let ext = self
            .wallet_client()
            .get_now_block2(EmptyMessage::default())
            .await?
            .into_inner();
        codec::block_from_extention(ext)
    }

    async fn get_block_by_number(&self, num: i64) -> Result<BlockInfo, Self::Error> {
        let ext = self
            .wallet_client()
            .get_block_by_num2(proto::NumberMessage { num })
            .await?
            .into_inner();
        codec::block_from_extention(ext)
    }

    // --- Account ---

    async fn get_account(&self, address: Address) -> Result<AccountInfo, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let account = self.wallet_client().get_account(req).await?.into_inner();
        codec::account_from_proto(account, address)
    }

    async fn get_account_resource(&self, address: Address) -> Result<AccountResource, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let res = self
            .wallet_client()
            .get_account_resource(req)
            .await?
            .into_inner();
        Ok(codec::account_resource_from_proto(res))
    }

    // --- Transaction ---

    async fn broadcast_transaction(&self, tx: &SignedTransaction) -> Result<(), Self::Error> {
        use proto::Transaction;

        let mut proto_tx = Transaction::decode(tx.raw.raw_proto.as_ref())?;
        for sig in &tx.signatures {
            proto_tx.signature.push(sig.to_bytes().to_vec());
        }

        let ret = self
            .wallet_client()
            .broadcast_transaction(proto_tx)
            .await?
            .into_inner();
        Self::check_return(Some(ret))
    }

    async fn get_transaction_by_id(&self, tx_id: TxId) -> Result<SignedTransaction, Self::Error> {
        let req = proto::BytesMessage {
            value: tx_id.as_slice().to_vec(),
        };
        let tx = self
            .wallet_client()
            .get_transaction_by_id(req)
            .await?
            .into_inner();
        codec::signed_tx_from_proto(tx)
    }

    async fn get_transaction_info(
        &self,
        tx_id: TxId,
    ) -> Result<Option<TransactionInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: tx_id.as_slice().to_vec(),
        };
        let info = self
            .wallet_client()
            .get_transaction_info_by_id(req)
            .await?
            .into_inner();
        codec::transaction_info_from_proto(info)
    }

    // --- Native contracts ---

    async fn transfer_trx(&self, params: TransferContract) -> Result<RawTransaction, Self::Error> {
        let req = codec::transfer_to_proto(params);
        let ext = self
            .wallet_client()
            .create_transaction2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn account_permission_update(
        &self,
        params: AccountPermissionUpdateContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::account_permission_update_to_proto(params);
        let ext = self
            .wallet_client()
            .account_permission_update(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn create_smart_contract(
        &self,
        params: CreateSmartContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::create_smart_contract_to_proto(params);
        let ext = self
            .wallet_client()
            .deploy_contract(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    // --- Smart contracts ---

    async fn trigger_smart_contract(
        &self,
        params: TriggerSmartContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let ext = self
            .wallet_client()
            .trigger_contract(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> Result<ConstantCallResult, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let ext = self
            .wallet_client()
            .trigger_constant_contract(req)
            .await?
            .into_inner();
        codec::constant_result_from_extention(ext)
    }

    async fn estimate_energy(&self, params: TriggerSmartContract) -> Result<i64, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let msg = self
            .wallet_client()
            .estimate_energy(req)
            .await?
            .into_inner();
        Self::check_return(msg.result)?;
        Ok(msg.energy_required)
    }

    // --- Staking ---

    async fn freeze_balance_v1(
        &self,
        params: FreezeBalanceV1Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::FreezeBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            frozen_balance: params.frozen_balance.as_sun(),
            frozen_duration: params.frozen_duration,
            resource: params.resource.as_i32(),
            receiver_address: params
                .receiver_address
                .map(|a| a.as_bytes().to_vec())
                .unwrap_or_default(),
        };
        let ext = self
            .wallet_client()
            .freeze_balance2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn unfreeze_balance_v1(
        &self,
        params: UnfreezeBalanceV1Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::UnfreezeBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            receiver_address: params
                .receiver_address
                .map(|a| a.as_bytes().to_vec())
                .unwrap_or_default(),
        };
        let ext = self
            .wallet_client()
            .unfreeze_balance2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn freeze_balance_v2(
        &self,
        params: FreezeBalanceV2Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::FreezeBalanceV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            frozen_balance: params.frozen_balance.as_sun(),
            resource: params.resource.as_i32(),
        };
        let ext = self
            .wallet_client()
            .freeze_balance_v2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn unfreeze_balance_v2(
        &self,
        params: UnfreezeBalanceV2Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::UnfreezeBalanceV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            unfreeze_balance: params.unfreeze_balance.as_sun(),
            resource: params.resource.as_i32(),
        };
        let ext = self
            .wallet_client()
            .unfreeze_balance_v2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn delegate_resource(
        &self,
        params: DelegateResourceContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::DelegateResourceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            balance: params.balance.as_sun(),
            receiver_address: params.receiver_address.as_bytes().to_vec(),
            lock: params.lock_period.is_some(),
            lock_period: params.lock_period.unwrap_or(0),
        };
        let ext = self
            .wallet_client()
            .delegate_resource(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn undelegate_resource(
        &self,
        params: UnDelegateResourceContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::UnDelegateResourceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            balance: params.balance.as_sun(),
            receiver_address: params.receiver_address.as_bytes().to_vec(),
        };
        let ext = self
            .wallet_client()
            .un_delegate_resource(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn withdraw_expire_unfreeze(
        &self,
        params: WithdrawExpireUnfreezeContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::WithdrawExpireUnfreezeContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let ext = self
            .wallet_client()
            .withdraw_expire_unfreeze(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn cancel_all_unfreeze_v2(
        &self,
        params: CancelAllUnfreezeV2Contract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::CancelAllUnfreezeV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let ext = self
            .wallet_client()
            .cancel_all_unfreeze_v2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn withdraw_balance(
        &self,
        params: WithdrawBalanceContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::WithdrawBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let ext = self
            .wallet_client()
            .withdraw_balance2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    // --- Resource queries ---

    async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>, Self::Error> {
        let req = proto::DelegatedResourceMessage {
            from_address: from.as_bytes().to_vec(),
            to_address: to.as_bytes().to_vec(),
        };
        let list = self
            .wallet_client()
            .get_delegated_resource(req)
            .await?
            .into_inner();
        list.delegated_resource
            .into_iter()
            .map(codec::delegated_resource_from_proto)
            .collect()
    }

    async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let idx = self
            .wallet_client()
            .get_delegated_resource_account_index(req)
            .await?
            .into_inner();
        codec::delegated_resource_index_from_proto(idx)
    }

    async fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>, Self::Error> {
        let req = proto::DelegatedResourceMessage {
            from_address: from.as_bytes().to_vec(),
            to_address: to.as_bytes().to_vec(),
        };
        let list = self
            .wallet_client()
            .get_delegated_resource_v2(req)
            .await?
            .into_inner();
        list.delegated_resource
            .into_iter()
            .map(codec::delegated_resource_from_proto)
            .collect()
    }

    async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let idx = self
            .wallet_client()
            .get_delegated_resource_account_index_v2(req)
            .await?
            .into_inner();
        codec::delegated_resource_index_from_proto(idx)
    }

    async fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> Result<Trx, Self::Error> {
        let req = proto::CanDelegatedMaxSizeRequestMessage {
            owner_address: address.as_bytes().to_vec(),
            r#type: resource.as_i32(),
        };
        let res = self
            .wallet_client()
            .get_can_delegated_max_size(req)
            .await?
            .into_inner();
        Ok(Trx::from_sun_unchecked(res.max_size))
    }

    async fn get_reward(&self, address: Address) -> Result<Trx, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let res = self
            .wallet_client()
            .get_reward_info(req)
            .await?
            .into_inner();
        Ok(Trx::from_sun_unchecked(res.num))
    }

    // --- Network ---

    async fn get_chain_parameters(&self) -> Result<HashMap<String, i64>, Self::Error> {
        let params = self
            .wallet_client()
            .get_chain_parameters(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(params
            .chain_parameter
            .into_iter()
            .map(|p| (p.key, p.value))
            .collect())
    }

    async fn get_contract(&self, address: Address) -> Result<SmartContractInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let contract = self.wallet_client().get_contract(req).await?.into_inner();
        Ok(codec::smart_contract_from_proto(contract))
    }

    async fn get_contract_info(&self, address: Address) -> Result<SmartContractInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let wrapper = self
            .wallet_client()
            .get_contract_info(req)
            .await?
            .into_inner();
        Ok(codec::smart_contract_info_from_wrapper(wrapper))
    }

    async fn list_witnesses(&self) -> Result<Vec<WitnessInfo>, Self::Error> {
        let list = self
            .wallet_client()
            .list_witnesses(proto::EmptyMessage::default())
            .await?
            .into_inner();
        Ok(list
            .witnesses
            .into_iter()
            .filter_map(codec::witness_from_proto)
            .collect())
    }

    // --- Governance ---

    async fn proposal_create(
        &self,
        params: ProposalCreateContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::proposal_create_to_proto(params);
        let ext = self
            .wallet_client()
            .proposal_create(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn proposal_approve(
        &self,
        params: ProposalApproveContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::proposal_approve_to_proto(params);
        let ext = self
            .wallet_client()
            .proposal_approve(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn proposal_delete(
        &self,
        params: ProposalDeleteContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::proposal_delete_to_proto(params);
        let ext = self
            .wallet_client()
            .proposal_delete(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn list_proposals(&self) -> Result<Vec<ProposalInfo>, Self::Error> {
        let list = self
            .wallet_client()
            .list_proposals(proto::EmptyMessage::default())
            .await?
            .into_inner();
        Ok(list
            .proposals
            .into_iter()
            .map(codec::proposal_from_proto)
            .collect())
    }

    async fn get_paginated_proposal_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ProposalInfo>, Self::Error> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = self
            .wallet_client()
            .get_paginated_proposal_list(req)
            .await?
            .into_inner();
        Ok(list
            .proposals
            .into_iter()
            .map(codec::proposal_from_proto)
            .collect())
    }

    async fn get_proposal_by_id(&self, proposal_id: i64) -> Result<ProposalInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: proposal_id.to_be_bytes().to_vec(),
        };
        let proposal = self
            .wallet_client()
            .get_proposal_by_id(req)
            .await?
            .into_inner();
        Ok(codec::proposal_from_proto(proposal))
    }

    // --- TRC10 ---

    async fn create_asset_issue(
        &self,
        params: AssetIssueContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::asset_issue_to_proto(params);
        let ext = self
            .wallet_client()
            .create_asset_issue2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn transfer_asset(
        &self,
        params: TransferAssetContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::transfer_asset_to_proto(params);
        let ext = self
            .wallet_client()
            .transfer_asset2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn get_asset_issue_by_id(
        &self,
        token_id: &str,
    ) -> Result<Option<AssetInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: token_id.as_bytes().to_vec(),
        };
        let asset = self
            .wallet_client()
            .get_asset_issue_by_id(req)
            .await?
            .into_inner();
        codec::asset_info_from_proto(asset)
    }

    async fn get_asset_issue_by_account(
        &self,
        address: Address,
    ) -> Result<Vec<AssetInfo>, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let list = self
            .wallet_client()
            .get_asset_issue_by_account(req)
            .await?
            .into_inner();
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect()
    }

    async fn get_paginated_asset_issue_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<AssetInfo>, Self::Error> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = self
            .wallet_client()
            .get_paginated_asset_issue_list(req)
            .await?
            .into_inner();
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect()
    }

    async fn get_asset_issue_by_name(&self, name: &str) -> Result<Option<AssetInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: name.as_bytes().to_vec(),
        };
        let asset = self
            .wallet_client()
            .get_asset_issue_by_name(req)
            .await?
            .into_inner();
        codec::asset_info_from_proto(asset)
    }

    async fn get_asset_issue_list_by_name(
        &self,
        name: &str,
    ) -> Result<Vec<AssetInfo>, Self::Error> {
        let req = proto::BytesMessage {
            value: name.as_bytes().to_vec(),
        };
        let list = self
            .wallet_client()
            .get_asset_issue_list_by_name(req)
            .await?
            .into_inner();
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect()
    }

    async fn participate_asset_issue(
        &self,
        params: ParticipateAssetIssueContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::participate_asset_issue_to_proto(params);
        let ext = self
            .wallet_client()
            .participate_asset_issue2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn unfreeze_asset(
        &self,
        params: UnfreezeAssetContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::unfreeze_asset_to_proto(params);
        let ext = self
            .wallet_client()
            .unfreeze_asset2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn update_asset(
        &self,
        params: UpdateAssetContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_asset_to_proto(params);
        let ext = self.wallet_client().update_asset2(req).await?.into_inner();
        Self::raw_from_extention(ext)
    }

    async fn create_account(
        &self,
        params: CreateAccountContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::create_account_to_proto(params);
        let ext = self
            .wallet_client()
            .create_account2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn vote_witness_account(
        &self,
        params: VoteWitnessContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::vote_witness_to_proto(params);
        let ext = self
            .wallet_client()
            .vote_witness_account2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn update_account(
        &self,
        params: UpdateAccountContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_account_to_proto(params);
        let ext = self
            .wallet_client()
            .update_account2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn set_account_id(
        &self,
        params: SetAccountIdContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::set_account_id_to_proto(params);
        // SetAccountId only has a v1 endpoint (returns Transaction, not TransactionExtention).
        let tx = self.wallet_client().set_account_id(req).await?.into_inner();
        codec::raw_from_plain(tx)
    }

    async fn clear_contract_abi(
        &self,
        params: ClearContractAbiContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::clear_contract_abi_to_proto(params);
        let ext = self
            .wallet_client()
            .clear_contract_abi(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn update_setting(
        &self,
        params: UpdateSettingContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_setting_to_proto(params);
        let ext = self.wallet_client().update_setting(req).await?.into_inner();
        Self::raw_from_extention(ext)
    }

    async fn update_energy_limit(
        &self,
        params: UpdateEnergyLimitContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_energy_limit_to_proto(params);
        let ext = self
            .wallet_client()
            .update_energy_limit(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> Result<Trx, Self::Error> {
        let req = proto::CanWithdrawUnfreezeAmountRequestMessage {
            owner_address: address.as_bytes().to_vec(),
            timestamp: timestamp_ms,
        };
        let res = self
            .wallet_client()
            .get_can_withdraw_unfreeze_amount(req)
            .await?
            .into_inner();
        Ok(Trx::from_sun_unchecked(res.amount))
    }

    async fn get_available_unfreeze_count(&self, address: Address) -> Result<i64, Self::Error> {
        let req = proto::GetAvailableUnfreezeCountRequestMessage {
            owner_address: address.as_bytes().to_vec(),
        };
        let res = self
            .wallet_client()
            .get_available_unfreeze_count(req)
            .await?
            .into_inner();
        Ok(res.count)
    }

    // --- Pricing / fees ---

    async fn get_bandwidth_prices(&self) -> Result<String, Self::Error> {
        let res = self
            .wallet_client()
            .get_bandwidth_prices(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(res.prices)
    }

    async fn get_energy_prices(&self) -> Result<String, Self::Error> {
        let res = self
            .wallet_client()
            .get_energy_prices(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(res.prices)
    }

    async fn get_memo_fee(&self) -> Result<u64, Self::Error> {
        let res = self
            .wallet_client()
            .get_memo_fee(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(res.prices.parse::<u64>().unwrap_or(0))
    }

    // --- Network / chain ---

    async fn get_next_maintenance_time(&self) -> Result<i64, Self::Error> {
        let res = self
            .wallet_client()
            .get_next_maintenance_time(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(res.num)
    }

    async fn get_burn_trx(&self) -> Result<u64, Self::Error> {
        let res = self
            .wallet_client()
            .get_burn_trx(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(res.num as u64)
    }

    async fn get_total_transactions(&self) -> Result<u64, Self::Error> {
        let res = self
            .wallet_client()
            .total_transaction(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(res.num as u64)
    }

    async fn get_node_info(&self) -> Result<NodeInfo, Self::Error> {
        let info = self
            .wallet_client()
            .get_node_info(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(NodeInfo {
            block: info.block,
            solidity_block: info.solidity_block,
            peer_num: info.current_connect_count,
        })
    }

    async fn list_nodes(&self) -> Result<Vec<NodeAddress>, Self::Error> {
        let list = self
            .wallet_client()
            .list_nodes(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(list
            .nodes
            .into_iter()
            .filter_map(|n| {
                n.address.map(|a| NodeAddress {
                    host: String::from_utf8_lossy(&a.host).into_owned(),
                    port: a.port,
                })
            })
            .collect())
    }

    async fn get_dynamic_properties(&self) -> Result<ChainProperties, Self::Error> {
        let props = self
            .database_client()
            .get_dynamic_properties(EmptyMessage::default())
            .await?
            .into_inner();
        // DynamicProperties only has last_solidity_block_num; use block ref for head info.
        // Return what the proto gives us directly.
        Ok(ChainProperties {
            head_block_id: String::new(),
            head_block_num: props.last_solidity_block_num,
            head_block_time_stamp: 0,
        })
    }

    // --- Block queries ---

    async fn get_block_by_id(&self, block_id: B256) -> Result<BlockInfo, Self::Error> {
        let req = proto::BytesMessage {
            value: block_id.as_slice().to_vec(),
        };
        let block = self
            .wallet_client()
            .get_block_by_id(req)
            .await?
            .into_inner();
        codec::block_from_plain(block)
    }

    async fn get_blocks_by_latest_num(&self, count: i64) -> Result<Vec<BlockInfo>, Self::Error> {
        let req = proto::NumberMessage { num: count };
        let list = self
            .wallet_client()
            .get_block_by_latest_num2(req)
            .await?
            .into_inner();
        list.block
            .into_iter()
            .map(codec::block_from_extention)
            .collect()
    }

    async fn get_blocks_by_limit(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<BlockInfo>, Self::Error> {
        let req = proto::BlockLimit {
            start_num: start,
            end_num: end,
        };
        let list = self
            .wallet_client()
            .get_block_by_limit_next2(req)
            .await?
            .into_inner();
        list.block
            .into_iter()
            .map(codec::block_from_extention)
            .collect()
    }

    async fn get_transaction_count_by_block_num(&self, block_num: i64) -> Result<u64, Self::Error> {
        let req = proto::NumberMessage { num: block_num };
        let res = self
            .wallet_client()
            .get_transaction_count_by_block_num(req)
            .await?
            .into_inner();
        Ok(res.num as u64)
    }

    // --- Transaction history ---

    async fn get_transactions_from(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<RawTransaction>, Self::Error> {
        let req = proto::AccountPaginated {
            account: Some(proto::Account {
                address: address.as_bytes().to_vec(),
                ..Default::default()
            }),
            offset,
            limit,
        };
        let list = self
            .wallet_extension_client()
            .get_transactions_from_this2(req)
            .await?
            .into_inner();
        list.transaction
            .into_iter()
            .map(GrpcTransport::raw_from_extention)
            .collect()
    }

    async fn get_transactions_to(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<RawTransaction>, Self::Error> {
        let req = proto::AccountPaginated {
            account: Some(proto::Account {
                address: address.as_bytes().to_vec(),
                ..Default::default()
            }),
            offset,
            limit,
        };
        let list = self
            .wallet_extension_client()
            .get_transactions_to_this2(req)
            .await?
            .into_inner();
        list.transaction
            .into_iter()
            .map(GrpcTransport::raw_from_extention)
            .collect()
    }

    async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> Result<Vec<TransactionInfo>, Self::Error> {
        let req = proto::NumberMessage { num: block_num };
        let list = self
            .wallet_client()
            .get_transaction_info_by_block_num(req)
            .await?
            .into_inner();
        list.transaction_info
            .into_iter()
            .filter_map(|info| codec::transaction_info_from_proto(info).transpose())
            .collect()
    }

    // --- Pending pool ---

    async fn get_pending_size(&self) -> Result<u64, Self::Error> {
        let res = self
            .wallet_client()
            .get_pending_size(EmptyMessage::default())
            .await?
            .into_inner();
        Ok(res.num as u64)
    }

    async fn get_transaction_from_pending(
        &self,
        tx_id: TxId,
    ) -> Result<RawTransaction, Self::Error> {
        let req = proto::BytesMessage {
            value: tx_id.as_slice().to_vec(),
        };
        let tx = self
            .wallet_client()
            .get_transaction_from_pending(req)
            .await?
            .into_inner();
        codec::raw_from_plain(tx)
    }

    async fn get_pending_transactions(&self) -> Result<Vec<RawTransaction>, Self::Error> {
        // GetTransactionListFromPending returns TransactionIdList (list of tx id hex strings).
        let id_list = self
            .wallet_client()
            .get_transaction_list_from_pending(EmptyMessage::default())
            .await?
            .into_inner();

        // Fan out all per-ID fetches concurrently (mirrors alloy's try_join_all pattern)
        // rather than issuing N sequential RPC calls.
        let futs = id_list.tx_id.into_iter().map(|tx_id_hex| {
            let transport = self.clone();
            async move {
                let id_bytes = decode_hex(&tx_id_hex)
                    .map_err(|e| TransportErrorKind::Malformed(format!("bad tx id hex: {e}")))?;
                let req = proto::BytesMessage { value: id_bytes };
                let tx = transport
                    .wallet_client()
                    .get_transaction_from_pending(req)
                    .await?
                    .into_inner();
                codec::raw_from_plain(tx)
            }
        });
        try_join_all(futs).await
    }

    // --- Multi-sig ---

    async fn get_transaction_sign_weight(
        &self,
        tx: &RawTransaction,
    ) -> Result<SignWeight, Self::Error> {
        use prost::Message as _;
        let proto_tx = proto::Transaction::decode(tx.raw_proto.as_ref())?;
        let weight = self
            .wallet_client()
            .get_transaction_sign_weight(proto_tx)
            .await?
            .into_inner();
        codec::sign_weight_from_proto(weight)
    }

    async fn get_transaction_approved_list(
        &self,
        tx: &RawTransaction,
    ) -> Result<Vec<Address>, Self::Error> {
        use prost::Message as _;
        let proto_tx = proto::Transaction::decode(tx.raw_proto.as_ref())?;
        let approved = self
            .wallet_client()
            .get_transaction_approved_list(proto_tx)
            .await?
            .into_inner();
        approved
            .approved_list
            .into_iter()
            .map(|bytes| {
                Address::from_slice(&bytes)
                    .map_err(|e| TransportErrorKind::Malformed(format!("bad address: {e}")))
            })
            .collect()
    }

    // --- Account net ---

    async fn get_account_net(&self, address: Address) -> Result<AccountNet, Self::Error> {
        let req = proto::Account {
            address: address.as_bytes().to_vec(),
            ..Default::default()
        };
        let msg = self
            .wallet_client()
            .get_account_net(req)
            .await?
            .into_inner();
        Ok(AccountNet {
            free_net_used: msg.free_net_used,
            free_net_limit: msg.free_net_limit,
            net_used: msg.net_used,
            net_limit: msg.net_limit,
            total_net_weight: msg.total_net_weight,
            energy_used: 0,
            energy_limit: 0,
            total_energy_weight: 0,
        })
    }

    // --- Witness ---

    async fn create_witness(
        &self,
        params: CreateWitnessContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::create_witness_to_proto(params);
        let ext = self
            .wallet_client()
            .create_witness2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn update_witness(
        &self,
        params: UpdateWitnessContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_witness_to_proto(params);
        let ext = self
            .wallet_client()
            .update_witness2(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn update_brokerage(
        &self,
        params: UpdateBrokerageContract,
    ) -> Result<RawTransaction, Self::Error> {
        let req = codec::update_brokerage_to_proto(params);
        let ext = self
            .wallet_client()
            .update_brokerage(req)
            .await?
            .into_inner();
        Self::raw_from_extention(ext)
    }

    async fn get_brokerage(&self, address: Address) -> Result<u64, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let res = self
            .wallet_client()
            .get_brokerage_info(req)
            .await?
            .into_inner();
        Ok(res.num as u64)
    }

    async fn get_reward_info(&self, address: Address) -> Result<u64, Self::Error> {
        let req = proto::BytesMessage {
            value: address.as_bytes().to_vec(),
        };
        let res = self
            .wallet_client()
            .get_reward_info(req)
            .await?
            .into_inner();
        Ok(res.num as u64)
    }
}
