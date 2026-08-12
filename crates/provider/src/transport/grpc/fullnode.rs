//! tonic-backed transport for the TRON FullNode gRPC services.

use std::{collections::HashMap, future::Future, time::Duration};

use async_trait::async_trait;
use futures::future::try_join_all;
use prost::Message as _;
use tronz_primitives::{Address, B256, ResourceCode, Trx, TxId};

use super::{
    GrpcCore, GrpcTransportConfig, RetryConfig, codec,
    core::{DatabaseClientI, WalletClientI, WalletExtensionClientI},
    light_block,
};
use crate::{
    error::{RpcStatusCode, TransportErrorKind, TransportResult},
    proto::{self, EmptyMessage, transaction::contract::ContractType as ContractKind},
    transport::TronTransport,
    types::{
        AccountInfo, AccountNet, AccountPermissionUpdateContract, AccountResource, AssetInfo,
        AssetIssueContract, BlockInfo, CancelAllUnfreezeV2Contract, ChainProperties,
        ClearContractAbiContract, ConstantCallResult, CreateAccountContract, CreateSmartContract,
        CreateWitnessContract, DelegateResourceContract, DelegatedResource, DelegatedResourceIndex,
        ExchangeCreateContract, ExchangeInfo, ExchangeInjectContract, ExchangeTransactionContract,
        ExchangeWithdrawContract, FreezeBalanceV1Contract, FreezeBalanceV2Contract,
        MarketCancelOrderContract, MarketOrderInfo, MarketOrderPair, MarketPrice,
        MarketSellAssetContract, NodeAddress, NodeInfo, ParticipateAssetIssueContract,
        ProposalApproveContract, ProposalCreateContract, ProposalDeleteContract, ProposalInfo,
        RawTransaction, SetAccountIdContract, SignWeight, SignedTransaction, SmartContractInfo,
        TransactionInfo, TransferAssetContract, TransferContract, TriggerSmartContract,
        UnDelegateResourceContract, UnfreezeAssetContract, UnfreezeBalanceV1Contract,
        UnfreezeBalanceV2Contract, UpdateAccountContract, UpdateAssetContract,
        UpdateBrokerageContract, UpdateEnergyLimitContract, UpdateSettingContract,
        UpdateWitnessContract, VoteWitnessContract, WithdrawBalanceContract,
        WithdrawExpireUnfreezeContract, WitnessInfo,
    },
};

/// Pre-connect builder for [`GrpcTransport`].
///
/// Accumulates a [`GrpcTransportConfig`] via chainable `with_*` setters, then
/// [`connect`](Self::connect)s. This is the advanced entry point;
/// [`ProviderBuilder`](crate::ProviderBuilder) is the primary one.
#[derive(Clone, Debug, Default)]
pub struct GrpcTransportBuilder {
    config: GrpcTransportConfig,
}

impl GrpcTransportBuilder {
    /// Override the connect (handshake) timeout.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Override the per-call request timeout.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    /// Override the retry policy.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.config.retry = retry;
        self
    }

    /// Add equivalent node endpoints for client-side failover / load balancing.
    ///
    /// These join the primary `uri` passed to [`connect`](Self::connect); two or
    /// more total endpoints switch the channel to load balancing.
    pub fn with_endpoints<I, S>(mut self, endpoints: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.endpoints = endpoints.into_iter().map(Into::into).collect();
        self
    }

    /// Wrap every call in `middleware`, outermost first.
    ///
    /// See [`GrpcMiddleware`](super::GrpcMiddleware) for what belongs here rather
    /// than in a [`ProviderLayer`](crate::ProviderLayer).
    pub fn with_middleware(
        mut self,
        middleware: std::sync::Arc<dyn super::GrpcMiddleware>,
    ) -> Self {
        self.config.middleware.push(middleware);
        self
    }

    /// Optionally set the TronGrid API key.
    pub fn maybe_api_key(mut self, key: Option<impl Into<String>>) -> Self {
        self.config.api_key = key.map(Into::into);
        self
    }

    /// Connect using the accumulated configuration.
    pub async fn connect(self, uri: impl AsRef<str>) -> Result<GrpcTransport, TransportErrorKind> {
        GrpcTransport::connect_with_config(uri, self.config).await
    }
}

/// gRPC transport targeting TRON's FullNode `protocol.Wallet` service.
#[derive(Clone)]
pub struct GrpcTransport {
    core: GrpcCore,
}

impl GrpcTransport {
    /// Connect to a TRON gRPC node with default timeouts and retry policy.
    ///
    /// `uri` may be:
    /// - `"https://grpc.trongrid.io:443"` (TronGrid mainnet, TLS)
    /// - `"http://127.0.0.1:50051"` (local node, plain HTTP/2)
    ///
    /// For custom timeouts / retry / API key use [`builder`](Self::builder).
    pub async fn connect(uri: impl AsRef<str>) -> Result<Self, TransportErrorKind> {
        Self::connect_with_config(uri, GrpcTransportConfig::default()).await
    }

    /// Start an advanced, pre-connect [`GrpcTransportBuilder`] (timeouts, retry,
    /// API key).
    pub fn builder() -> GrpcTransportBuilder {
        GrpcTransportBuilder::default()
    }

    /// Connect with an explicit [`GrpcTransportConfig`].
    pub(crate) async fn connect_with_config(
        uri: impl AsRef<str>,
        cfg: GrpcTransportConfig,
    ) -> Result<Self, TransportErrorKind> {
        Ok(Self { core: GrpcCore::connect_with_config(uri, cfg).await? })
    }

    /// Attach a TronGrid API key (sent as `TRON-PRO-API-KEY` header on each call).
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.core.set_api_key(key.into());
        self
    }

    async fn call_with_retry<F, Fut, T>(
        &self,
        method: &'static str,
        f: F,
    ) -> Result<T, TransportErrorKind>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, TransportErrorKind>>,
    {
        self.core.call_with_retry(method, f).await
    }

    fn wallet_client(&self) -> WalletClientI {
        self.core.wallet_client()
    }

    fn wallet_extension_client(&self) -> WalletExtensionClientI {
        self.core.wallet_extension_client()
    }

    fn database_client(&self) -> DatabaseClientI {
        self.core.database_client()
    }

    /// Calls a Wallet RPC using a custom wire-compatible response type.
    async fn wallet_unary<Req, Res>(
        &self,
        req: Req,
        path: &'static str,
        method: &'static str,
        label: &'static str,
    ) -> Result<Res, TransportErrorKind>
    where
        Req: prost::Message + Default + Clone + Send + Sync + 'static,
        Res: prost::Message + Default + Send + Sync + 'static,
    {
        self.core.unary(req, path, "protocol.Wallet", method, label).await
    }

    /// Extract a [`RawTransaction`] from a [`proto::TransactionExtention`], after
    /// checking the node built the contract we asked it to.
    ///
    /// The node is trusted to fill TAPOS and nothing else. Everything the caller
    /// authorised — who sends, to whom, how much — is compared against the message
    /// we put on the wire, because the signature that follows covers whatever the
    /// node returned, not whatever we asked for. Without this a hostile node could
    /// answer a 1 TRX transfer with a sweep of the account and collect a valid
    /// signature for it.
    fn raw_from_extention<M>(
        ext: proto::TransactionExtention,
        sent: &M,
        kind: ContractKind,
    ) -> Result<RawTransaction, TransportErrorKind>
    where
        M: prost::Message + Default + PartialEq,
    {
        Self::raw_from_extention_with(ext, sent, kind, |_| {})
    }

    /// Extract a [`RawTransaction`] the node was *queried* for rather than asked
    /// to build.
    ///
    /// A transaction out of an account's history has no request to be compared
    /// against, and is never signed — only its id still has to be our own.
    fn raw_from_queried_extention(
        ext: proto::TransactionExtention,
    ) -> Result<RawTransaction, TransportErrorKind> {
        codec::check_return(ext.result)?;

        let tx = ext.transaction.ok_or_else(|| {
            TransportErrorKind::Malformed("missing transaction in extention".into())
        })?;

        Ok(RawTransaction::from_node_encoded(tx.encode_to_vec(), &ext.txid)?)
    }

    /// As [`raw_from_extention`](Self::raw_from_extention), but lets the caller
    /// clear fields the node is entitled to derive before the comparison.
    fn raw_from_extention_with<M>(
        ext: proto::TransactionExtention,
        sent: &M,
        kind: ContractKind,
        allow_derived: impl FnOnce(&mut M),
    ) -> Result<RawTransaction, TransportErrorKind>
    where
        M: prost::Message + Default + PartialEq,
    {
        codec::check_return(ext.result)?;

        let tx = ext.transaction.ok_or_else(|| {
            TransportErrorKind::Malformed("missing transaction in extention".into())
        })?;
        let raw_data = tx
            .raw_data
            .as_ref()
            .ok_or_else(|| TransportErrorKind::Malformed("transaction has no raw_data".into()))?;

        let [contract] = raw_data.contract.as_slice() else {
            return Err(TransportErrorKind::Malformed(format!(
                "node returned {} contracts, expected exactly the one requested",
                raw_data.contract.len()
            )));
        };

        if contract.r#type != kind as i32 {
            let built = ContractKind::try_from(contract.r#type)
                .map_or_else(|_| contract.r#type.to_string(), |k| format!("{k:?}"));
            return Err(TransportErrorKind::Malformed(format!(
                "node built a {built} contract, not the requested {kind:?}"
            )));
        }

        let parameter = contract
            .parameter
            .as_ref()
            .ok_or_else(|| TransportErrorKind::Malformed("contract has no parameter".into()))?;

        let mut built = M::decode(parameter.value.as_ref())?;
        allow_derived(&mut built);
        if built != *sent {
            return Err(TransportErrorKind::Malformed(
                "node built a different contract than the one requested".into(),
            ));
        }

        Ok(RawTransaction::from_node_encoded(tx.encode_to_vec(), &ext.txid)?)
    }
}

/// Decode a [`SignedTransaction`]'s unsigned proto and append its collected
/// signatures, producing the wire `Transaction` used for broadcast and for
/// sign-weight / approved-list queries.
fn signed_to_proto(tx: &SignedTransaction) -> Result<proto::Transaction, TransportErrorKind> {
    Ok(tx.to_proto()?)
}

/// Decode a lowercase hex string into bytes using only the standard library.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
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

/// Route a unary gRPC call through [`GrpcTransport::call_with_retry`].
///
/// - `$client`: client accessor — `wallet_client`, `wallet_extension_client`, or `database_client`.
/// - `$method`: the generated tonic client method.
/// - `$req`: a **`Clone`** request *identifier* (all prost messages are `Clone`).
///
/// Clones a fresh client + request per attempt into an `async move` future, so
/// nothing borrows `self`/`$req` across `.await`. Never use for
/// `broadcast_transaction`.
macro_rules! retry_unary {
    ($self:ident, $client:ident, $method:ident, $req:ident) => {
        $self
                    .call_with_retry(stringify!($method), || {
                        let mut client = $self.$client();
                        let req = $req.clone();
                        async move {
                            Ok(client.$method(req).await.map_err(super::map_status)?.into_inner())
                        }
                    })
                    .await
    };
}

/// Recognise a node that has not enabled `EstimateEnergy`, so a caller can reach for
/// another endpoint rather than treat it as a failed estimate.
///
/// `vm.estimateEnergy` is off by default, and a node in that state either says so in
/// the return message or does not serve the method at all.
fn estimate_energy_unsupported(err: TransportErrorKind) -> TransportErrorKind {
    let unsupported = match &err {
        TransportErrorKind::Rpc { code, .. } => *code == RpcStatusCode::Unimplemented,
        TransportErrorKind::NodeError(msg) => {
            let msg = msg.to_ascii_lowercase();
            msg.contains("not support") || msg.contains("not enabled")
        }
        _ => false,
    };

    if unsupported { TransportErrorKind::Unsupported(err.to_string()) } else { err }
}

fn market_order_not_found(err: &TransportErrorKind) -> bool {
    // java-tron reports a missing order as INTERNAL instead of returning an empty response.
    matches!(
        err,
        TransportErrorKind::Rpc { code: RpcStatusCode::Internal, message }
            if message.eq_ignore_ascii_case("order not found in store")
    )
}

impl crate::transport::private::Sealed for GrpcTransport {}

#[async_trait]
impl TronTransport for GrpcTransport {
    async fn get_now_block(&self) -> TransportResult<BlockInfo> {
        let req = EmptyMessage::default();
        let block: light_block::BlockSummaryProto = self
            .wallet_unary(req, "/protocol.Wallet/GetNowBlock2", "GetNowBlock2", "get_now_block")
            .await?;
        Ok(block.into_block_info(None)?)
    }

    async fn get_block_by_number(&self, num: i64) -> TransportResult<Option<BlockInfo>> {
        let req = proto::NumberMessage { num };
        let block: light_block::BlockSummaryProto = self
            .wallet_unary(
                req,
                "/protocol.Wallet/GetBlockByNum2",
                "GetBlockByNum2",
                "get_block_by_number",
            )
            .await?;
        Ok(block.into_block_lookup(None)?)
    }

    async fn get_account(&self, address: Address) -> TransportResult<AccountInfo> {
        let req = proto::Account { address: address.as_bytes().to_vec(), ..Default::default() };
        let account = retry_unary!(self, wallet_client, get_account, req)?;
        Ok(codec::account_from_proto(account, address)?)
    }

    async fn get_account_resource(&self, address: Address) -> TransportResult<AccountResource> {
        let req = proto::Account { address: address.as_bytes().to_vec(), ..Default::default() };
        let res = retry_unary!(self, wallet_client, get_account_resource, req)?;
        Ok(codec::account_resource_from_proto(res))
    }

    async fn broadcast_transaction(&self, tx: &SignedTransaction) -> TransportResult<()> {
        let proto_tx = signed_to_proto(tx)?;

        let ret = self
            .wallet_client()
            .broadcast_transaction(proto_tx)
            .await
            .map_err(super::map_status)?
            .into_inner();
        Ok(codec::check_return(Some(ret))?)
    }

    async fn get_transaction_by_id(
        &self,
        tx_id: TxId,
    ) -> TransportResult<Option<SignedTransaction>> {
        let req = proto::BytesMessage { value: tx_id.as_slice().to_vec() };
        let tx = retry_unary!(self, wallet_client, get_transaction_by_id, req)?;
        Ok(codec::signed_tx_lookup(tx, tx_id.as_slice())?)
    }

    async fn get_transaction_info(&self, tx_id: TxId) -> TransportResult<Option<TransactionInfo>> {
        let req = proto::BytesMessage { value: tx_id.as_slice().to_vec() };
        let info = retry_unary!(self, wallet_client, get_transaction_info_by_id, req)?;
        Ok(codec::transaction_info_from_proto(info)?)
    }

    async fn transfer_trx(&self, params: TransferContract) -> TransportResult<RawTransaction> {
        let req = codec::transfer_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, create_transaction2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::TransferContract)
    }

    async fn account_permission_update(
        &self,
        params: AccountPermissionUpdateContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::account_permission_update_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, account_permission_update, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::AccountPermissionUpdateContract)
    }

    async fn create_smart_contract(
        &self,
        params: CreateSmartContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::create_smart_contract_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, deploy_contract, req)?;
        // The node derives the contract address, code hash and tx hash it
        // returns; everything the deployer chose still has to match.
        Self::raw_from_extention_with(ext, &sent, ContractKind::CreateSmartContract, |built| {
            if let Some(c) = built.new_contract.as_mut() {
                c.contract_address = Vec::new();
                c.code_hash = Vec::new();
                c.trx_hash = Vec::new();
                c.version = 0;
            }
        })
    }

    async fn trigger_smart_contract(
        &self,
        params: TriggerSmartContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, trigger_contract, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::TriggerSmartContract)
    }

    async fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> TransportResult<ConstantCallResult> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let ext = retry_unary!(self, wallet_client, trigger_constant_contract, req)?;
        Ok(codec::constant_result_from_extention(ext)?)
    }

    async fn estimate_energy(&self, params: TriggerSmartContract) -> TransportResult<i64> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let msg = retry_unary!(self, wallet_client, estimate_energy, req)
            .map_err(estimate_energy_unsupported)?;
        codec::check_return(msg.result).map_err(|e| estimate_energy_unsupported(e.into()))?;
        Ok(msg.energy_required)
    }

    async fn freeze_balance_v1(
        &self,
        params: FreezeBalanceV1Contract,
    ) -> TransportResult<RawTransaction> {
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
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, freeze_balance2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::FreezeBalanceContract)
    }

    async fn unfreeze_balance_v1(
        &self,
        params: UnfreezeBalanceV1Contract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::UnfreezeBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            receiver_address: params
                .receiver_address
                .map(|a| a.as_bytes().to_vec())
                .unwrap_or_default(),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, unfreeze_balance2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UnfreezeBalanceContract)
    }

    async fn freeze_balance_v2(
        &self,
        params: FreezeBalanceV2Contract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::FreezeBalanceV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            frozen_balance: params.frozen_balance.as_sun(),
            resource: params.resource.as_i32(),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, freeze_balance_v2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::FreezeBalanceV2Contract)
    }

    async fn unfreeze_balance_v2(
        &self,
        params: UnfreezeBalanceV2Contract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::UnfreezeBalanceV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            unfreeze_balance: params.unfreeze_balance.as_sun(),
            resource: params.resource.as_i32(),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, unfreeze_balance_v2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UnfreezeBalanceV2Contract)
    }

    async fn delegate_resource(
        &self,
        params: DelegateResourceContract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::DelegateResourceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            balance: params.balance.as_sun(),
            receiver_address: params.receiver_address.as_bytes().to_vec(),
            lock: params.lock_period.is_some(),
            lock_period: params.lock_period.unwrap_or(0),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, delegate_resource, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::DelegateResourceContract)
    }

    async fn undelegate_resource(
        &self,
        params: UnDelegateResourceContract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::UnDelegateResourceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
            resource: params.resource.as_i32(),
            balance: params.balance.as_sun(),
            receiver_address: params.receiver_address.as_bytes().to_vec(),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, un_delegate_resource, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UnDelegateResourceContract)
    }

    async fn withdraw_expire_unfreeze(
        &self,
        params: WithdrawExpireUnfreezeContract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::WithdrawExpireUnfreezeContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, withdraw_expire_unfreeze, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::WithdrawExpireUnfreezeContract)
    }

    async fn cancel_all_unfreeze_v2(
        &self,
        params: CancelAllUnfreezeV2Contract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::CancelAllUnfreezeV2Contract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, cancel_all_unfreeze_v2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::CancelAllUnfreezeV2Contract)
    }

    async fn withdraw_balance(
        &self,
        params: WithdrawBalanceContract,
    ) -> TransportResult<RawTransaction> {
        let req = proto::WithdrawBalanceContract {
            owner_address: params.owner_address.as_bytes().to_vec(),
        };
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, withdraw_balance2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::WithdrawBalanceContract)
    }

    async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> TransportResult<Vec<DelegatedResource>> {
        let req = proto::DelegatedResourceMessage {
            from_address: from.as_bytes().to_vec(),
            to_address: to.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_delegated_resource, req)?;
        list.delegated_resource
            .into_iter()
            .map(codec::delegated_resource_from_proto)
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> TransportResult<DelegatedResourceIndex> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let idx = retry_unary!(self, wallet_client, get_delegated_resource_account_index, req)?;
        Ok(codec::delegated_resource_index_from_proto(idx)?)
    }

    async fn get_delegated_resource(
        &self,
        from: Address,
        to: Address,
    ) -> TransportResult<Vec<DelegatedResource>> {
        let req = proto::DelegatedResourceMessage {
            from_address: from.as_bytes().to_vec(),
            to_address: to.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_delegated_resource_v2, req)?;
        list.delegated_resource
            .into_iter()
            .map(codec::delegated_resource_from_proto)
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> TransportResult<DelegatedResourceIndex> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let idx = retry_unary!(self, wallet_client, get_delegated_resource_account_index_v2, req)?;
        Ok(codec::delegated_resource_index_from_proto(idx)?)
    }

    async fn get_can_delegate_max(
        &self,
        address: Address,
        resource: ResourceCode,
    ) -> TransportResult<Trx> {
        let req = proto::CanDelegatedMaxSizeRequestMessage {
            owner_address: address.as_bytes().to_vec(),
            r#type: resource.as_i32(),
        };
        let res = retry_unary!(self, wallet_client, get_can_delegated_max_size, req)?;
        Ok(Trx::from_sun_unchecked(res.max_size))
    }

    async fn get_reward(&self, address: Address) -> TransportResult<Trx> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let res = retry_unary!(self, wallet_client, get_reward_info, req)?;
        Ok(Trx::from_sun_unchecked(res.num))
    }

    async fn get_chain_parameters(&self) -> TransportResult<HashMap<String, i64>> {
        let req = EmptyMessage::default();
        let params = retry_unary!(self, wallet_client, get_chain_parameters, req)?;
        Ok(params.chain_parameter.into_iter().map(|p| (p.key, p.value)).collect())
    }

    async fn get_contract(&self, address: Address) -> TransportResult<SmartContractInfo> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let contract = retry_unary!(self, wallet_client, get_contract, req)?;
        Ok(codec::smart_contract_from_proto(contract))
    }

    async fn get_contract_info(&self, address: Address) -> TransportResult<SmartContractInfo> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let wrapper = retry_unary!(self, wallet_client, get_contract_info, req)?;
        Ok(codec::smart_contract_info_from_wrapper(wrapper))
    }

    async fn list_witnesses(&self) -> TransportResult<Vec<WitnessInfo>> {
        let req = proto::EmptyMessage::default();
        let list = retry_unary!(self, wallet_client, list_witnesses, req)?;
        Ok(list.witnesses.into_iter().filter_map(codec::witness_from_proto).collect())
    }

    async fn get_paginated_now_witness_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<WitnessInfo>> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = retry_unary!(self, wallet_client, get_paginated_now_witness_list, req)?;
        Ok(list.witnesses.into_iter().filter_map(codec::witness_from_proto).collect())
    }

    async fn proposal_create(
        &self,
        params: ProposalCreateContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::proposal_create_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, proposal_create, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ProposalCreateContract)
    }

    async fn proposal_approve(
        &self,
        params: ProposalApproveContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::proposal_approve_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, proposal_approve, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ProposalApproveContract)
    }

    async fn proposal_delete(
        &self,
        params: ProposalDeleteContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::proposal_delete_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, proposal_delete, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ProposalDeleteContract)
    }

    async fn list_proposals(&self) -> TransportResult<Vec<ProposalInfo>> {
        let req = proto::EmptyMessage::default();
        let list = retry_unary!(self, wallet_client, list_proposals, req)?;
        Ok(list.proposals.into_iter().map(codec::proposal_from_proto).collect())
    }

    async fn get_paginated_proposal_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<ProposalInfo>> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = retry_unary!(self, wallet_client, get_paginated_proposal_list, req)?;
        Ok(list.proposals.into_iter().map(codec::proposal_from_proto).collect())
    }

    async fn get_proposal_by_id(&self, proposal_id: i64) -> TransportResult<ProposalInfo> {
        let req = proto::BytesMessage { value: proposal_id.to_be_bytes().to_vec() };
        let proposal = retry_unary!(self, wallet_client, get_proposal_by_id, req)?;
        Ok(codec::proposal_from_proto(proposal))
    }

    async fn create_asset_issue(
        &self,
        params: AssetIssueContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::asset_issue_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, create_asset_issue2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::AssetIssueContract)
    }

    async fn transfer_asset(
        &self,
        params: TransferAssetContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::transfer_asset_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, transfer_asset2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::TransferAssetContract)
    }

    async fn get_asset_issue_by_id(&self, token_id: &str) -> TransportResult<Option<AssetInfo>> {
        let req = proto::BytesMessage { value: token_id.as_bytes().to_vec() };
        let asset = retry_unary!(self, wallet_client, get_asset_issue_by_id, req)?;
        Ok(codec::asset_info_from_proto(asset)?)
    }

    async fn get_asset_issue_by_account(
        &self,
        address: Address,
    ) -> TransportResult<Vec<AssetInfo>> {
        let req = proto::Account { address: address.as_bytes().to_vec(), ..Default::default() };
        let list = retry_unary!(self, wallet_client, get_asset_issue_by_account, req)?;
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_paginated_asset_issue_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<AssetInfo>> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = retry_unary!(self, wallet_client, get_paginated_asset_issue_list, req)?;
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_asset_issue_by_name(&self, name: &str) -> TransportResult<Option<AssetInfo>> {
        let req = proto::BytesMessage { value: name.as_bytes().to_vec() };
        let asset = retry_unary!(self, wallet_client, get_asset_issue_by_name, req)?;
        Ok(codec::asset_info_from_proto(asset)?)
    }

    async fn get_asset_issue_list_by_name(&self, name: &str) -> TransportResult<Vec<AssetInfo>> {
        let req = proto::BytesMessage { value: name.as_bytes().to_vec() };
        let list = retry_unary!(self, wallet_client, get_asset_issue_list_by_name, req)?;
        list.asset_issue
            .into_iter()
            .filter_map(|a| codec::asset_info_from_proto(a).transpose())
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn participate_asset_issue(
        &self,
        params: ParticipateAssetIssueContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::participate_asset_issue_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, participate_asset_issue2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ParticipateAssetIssueContract)
    }

    async fn unfreeze_asset(
        &self,
        params: UnfreezeAssetContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::unfreeze_asset_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, unfreeze_asset2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UnfreezeAssetContract)
    }

    async fn update_asset(&self, params: UpdateAssetContract) -> TransportResult<RawTransaction> {
        let req = codec::update_asset_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, update_asset2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UpdateAssetContract)
    }

    async fn create_account(
        &self,
        params: CreateAccountContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::create_account_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, create_account2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::AccountCreateContract)
    }

    async fn vote_witness_account(
        &self,
        params: VoteWitnessContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::vote_witness_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, vote_witness_account2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::VoteWitnessContract)
    }

    async fn update_account(
        &self,
        params: UpdateAccountContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::update_account_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, update_account2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::AccountUpdateContract)
    }

    async fn set_account_id(
        &self,
        params: SetAccountIdContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::set_account_id_to_proto(params);
        // SetAccountId only has a v1 endpoint (returns Transaction, not TransactionExtention).
        let tx = retry_unary!(self, wallet_client, set_account_id, req)?;
        Ok(codec::raw_from_plain(tx)?)
    }

    async fn clear_contract_abi(
        &self,
        params: ClearContractAbiContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::clear_contract_abi_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, clear_contract_abi, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ClearAbiContract)
    }

    async fn update_setting(
        &self,
        params: UpdateSettingContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::update_setting_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, update_setting, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UpdateSettingContract)
    }

    async fn update_energy_limit(
        &self,
        params: UpdateEnergyLimitContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::update_energy_limit_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, update_energy_limit, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UpdateEnergyLimitContract)
    }

    async fn get_can_withdraw_unfreeze_amount(
        &self,
        address: Address,
        timestamp_ms: i64,
    ) -> TransportResult<Trx> {
        let req = proto::CanWithdrawUnfreezeAmountRequestMessage {
            owner_address: address.as_bytes().to_vec(),
            timestamp: timestamp_ms,
        };
        let res = retry_unary!(self, wallet_client, get_can_withdraw_unfreeze_amount, req)?;
        Ok(Trx::from_sun_unchecked(res.amount))
    }

    async fn get_available_unfreeze_count(&self, address: Address) -> TransportResult<i64> {
        let req = proto::GetAvailableUnfreezeCountRequestMessage {
            owner_address: address.as_bytes().to_vec(),
        };
        let res = retry_unary!(self, wallet_client, get_available_unfreeze_count, req)?;
        Ok(res.count)
    }

    async fn get_bandwidth_prices(&self) -> TransportResult<String> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_bandwidth_prices, req)?;
        Ok(res.prices)
    }

    async fn get_energy_prices(&self) -> TransportResult<String> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_energy_prices, req)?;
        Ok(res.prices)
    }

    async fn get_memo_fee(&self) -> TransportResult<u64> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_memo_fee, req)?;
        Ok(res.prices.parse::<u64>().unwrap_or(0))
    }

    async fn get_next_maintenance_time(&self) -> TransportResult<i64> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_next_maintenance_time, req)?;
        Ok(res.num)
    }

    async fn get_burn_trx(&self) -> TransportResult<u64> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_burn_trx, req)?;
        Ok(res.num as u64)
    }

    async fn get_total_transactions(&self) -> TransportResult<u64> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, total_transaction, req)?;
        Ok(res.num as u64)
    }

    async fn get_node_info(&self) -> TransportResult<NodeInfo> {
        let req = EmptyMessage::default();
        let info = retry_unary!(self, wallet_client, get_node_info, req)?;
        Ok(codec::node_info_from_proto(info))
    }

    async fn list_nodes(&self) -> TransportResult<Vec<NodeAddress>> {
        let req = EmptyMessage::default();
        let list = retry_unary!(self, wallet_client, list_nodes, req)?;
        Ok(codec::node_addresses_from_proto(list))
    }

    async fn get_dynamic_properties(&self) -> TransportResult<ChainProperties> {
        let req = EmptyMessage::default();
        let props = retry_unary!(self, database_client, get_dynamic_properties, req)?;
        Ok(codec::chain_properties_from_proto(props))
    }

    async fn get_block_by_id(&self, block_id: B256) -> TransportResult<Option<BlockInfo>> {
        let req = proto::BytesMessage { value: block_id.as_slice().to_vec() };
        let block: light_block::BlockSummaryProto = self
            .wallet_unary(req, "/protocol.Wallet/GetBlockById", "GetBlockById", "get_block_by_id")
            .await?;
        Ok(block.into_block_lookup(Some(block_id))?)
    }

    async fn get_blocks_by_latest_num(&self, count: i64) -> TransportResult<Vec<BlockInfo>> {
        let req = proto::NumberMessage { num: count };
        let list: light_block::BlockSummaryListProto = self
            .wallet_unary(
                req,
                "/protocol.Wallet/GetBlockByLatestNum2",
                "GetBlockByLatestNum2",
                "get_blocks_by_latest_num",
            )
            .await?;
        list.blocks
            .into_iter()
            .map(|block| block.into_block_info(None))
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_blocks_by_limit(&self, start: i64, end: i64) -> TransportResult<Vec<BlockInfo>> {
        let req = proto::BlockLimit { start_num: start, end_num: end };
        let list: light_block::BlockSummaryListProto = self
            .wallet_unary(
                req,
                "/protocol.Wallet/GetBlockByLimitNext2",
                "GetBlockByLimitNext2",
                "get_blocks_by_limit",
            )
            .await?;
        list.blocks
            .into_iter()
            .map(|block| block.into_block_info(None))
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_transaction_count_by_block_num(&self, block_num: i64) -> TransportResult<u64> {
        let req = proto::NumberMessage { num: block_num };
        let res = retry_unary!(self, wallet_client, get_transaction_count_by_block_num, req)?;
        Ok(res.num as u64)
    }

    async fn get_transactions_from(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<RawTransaction>> {
        let req = proto::AccountPaginated {
            account: Some(proto::Account {
                address: address.as_bytes().to_vec(),
                ..Default::default()
            }),
            offset,
            limit,
        };
        let list = retry_unary!(self, wallet_extension_client, get_transactions_from_this2, req)?;
        list.transaction.into_iter().map(GrpcTransport::raw_from_queried_extention).collect()
    }

    async fn get_transactions_to(
        &self,
        address: Address,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<RawTransaction>> {
        let req = proto::AccountPaginated {
            account: Some(proto::Account {
                address: address.as_bytes().to_vec(),
                ..Default::default()
            }),
            offset,
            limit,
        };
        let list = retry_unary!(self, wallet_extension_client, get_transactions_to_this2, req)?;
        list.transaction.into_iter().map(GrpcTransport::raw_from_queried_extention).collect()
    }

    async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> TransportResult<Vec<TransactionInfo>> {
        let req = proto::NumberMessage { num: block_num };
        let list = retry_unary!(self, wallet_client, get_transaction_info_by_block_num, req)?;
        list.transaction_info
            .into_iter()
            .filter_map(|info| codec::transaction_info_from_proto(info).transpose())
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_pending_size(&self) -> TransportResult<u64> {
        let req = EmptyMessage::default();
        let res = retry_unary!(self, wallet_client, get_pending_size, req)?;
        Ok(res.num as u64)
    }

    async fn get_transaction_from_pending(&self, tx_id: TxId) -> TransportResult<RawTransaction> {
        let req = proto::BytesMessage { value: tx_id.as_slice().to_vec() };
        let tx = retry_unary!(self, wallet_client, get_transaction_from_pending, req)?;
        Ok(codec::raw_from_plain(tx)?)
    }

    async fn get_pending_transactions(&self) -> TransportResult<Vec<RawTransaction>> {
        // GetTransactionListFromPending returns TransactionIdList (list of tx id hex strings).
        let req = EmptyMessage::default();
        let id_list = retry_unary!(self, wallet_client, get_transaction_list_from_pending, req)?;

        // Fan out all per-ID fetches concurrently (mirrors alloy's try_join_all pattern)
        // rather than issuing N sequential RPC calls.
        let futs = id_list.tx_id.into_iter().map(|tx_id_hex| {
            let transport = self.clone();
            async move {
                let id_bytes = decode_hex(&tx_id_hex)
                    .map_err(|e| TransportErrorKind::Malformed(format!("bad tx id hex: {e}")))?;
                let req = proto::BytesMessage { value: id_bytes };
                let tx = retry_unary!(transport, wallet_client, get_transaction_from_pending, req)?;
                Ok(codec::raw_from_plain(tx)?)
            }
        });
        try_join_all(futs).await
    }

    async fn get_transaction_sign_weight(
        &self,
        tx: &SignedTransaction,
    ) -> TransportResult<SignWeight> {
        let proto_tx = signed_to_proto(tx)?;
        let weight = retry_unary!(self, wallet_client, get_transaction_sign_weight, proto_tx)?;
        Ok(codec::sign_weight_from_proto(weight)?)
    }

    async fn get_transaction_approved_list(
        &self,
        tx: &SignedTransaction,
    ) -> TransportResult<Vec<Address>> {
        let proto_tx = signed_to_proto(tx)?;
        let approved = retry_unary!(self, wallet_client, get_transaction_approved_list, proto_tx)?;
        approved
            .approved_list
            .into_iter()
            .map(|bytes| {
                Address::from_slice(&bytes)
                    .map_err(|e| TransportErrorKind::Malformed(format!("bad address: {e}")))
            })
            .collect()
    }

    async fn get_account_net(&self, address: Address) -> TransportResult<AccountNet> {
        let req = proto::Account { address: address.as_bytes().to_vec(), ..Default::default() };
        let msg = retry_unary!(self, wallet_client, get_account_net, req)?;
        Ok(codec::account_net_from_proto(msg))
    }

    async fn create_witness(
        &self,
        params: CreateWitnessContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::create_witness_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, create_witness2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::WitnessCreateContract)
    }

    async fn update_witness(
        &self,
        params: UpdateWitnessContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::update_witness_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, update_witness2, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::WitnessUpdateContract)
    }

    async fn update_brokerage(
        &self,
        params: UpdateBrokerageContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::update_brokerage_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, update_brokerage, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::UpdateBrokerageContract)
    }

    async fn get_brokerage(&self, address: Address) -> TransportResult<u64> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let res = retry_unary!(self, wallet_client, get_brokerage_info, req)?;
        Ok(res.num as u64)
    }

    async fn get_reward_info(&self, address: Address) -> TransportResult<u64> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let res = retry_unary!(self, wallet_client, get_reward_info, req)?;
        Ok(res.num as u64)
    }

    async fn exchange_create(
        &self,
        params: ExchangeCreateContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::exchange_create_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, exchange_create, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ExchangeCreateContract)
    }

    async fn exchange_inject(
        &self,
        params: ExchangeInjectContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::exchange_inject_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, exchange_inject, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ExchangeInjectContract)
    }

    async fn exchange_withdraw(
        &self,
        params: ExchangeWithdrawContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::exchange_withdraw_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, exchange_withdraw, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ExchangeWithdrawContract)
    }

    async fn exchange_transaction(
        &self,
        params: ExchangeTransactionContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::exchange_transaction_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, exchange_transaction, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::ExchangeTransactionContract)
    }

    async fn list_exchanges(&self) -> TransportResult<Vec<ExchangeInfo>> {
        let req = EmptyMessage {};
        let list = retry_unary!(self, wallet_client, list_exchanges, req)?;
        list.exchanges
            .into_iter()
            .map(codec::exchange_info_from_proto)
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_paginated_exchange_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> TransportResult<Vec<ExchangeInfo>> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = retry_unary!(self, wallet_client, get_paginated_exchange_list, req)?;
        list.exchanges
            .into_iter()
            .map(codec::exchange_info_from_proto)
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_exchange_by_id(&self, exchange_id: i64) -> TransportResult<Option<ExchangeInfo>> {
        let mut id_bytes = [0u8; 8];
        id_bytes.copy_from_slice(&exchange_id.to_be_bytes());
        let req = proto::BytesMessage { value: id_bytes.to_vec() };
        let exchange = retry_unary!(self, wallet_client, get_exchange_by_id, req)?;
        if exchange.exchange_id == 0 && exchange.creator_address.is_empty() {
            return Ok(None);
        }
        Ok(Some(codec::exchange_info_from_proto(exchange)?))
    }

    async fn market_sell_asset(
        &self,
        params: MarketSellAssetContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::market_sell_asset_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, market_sell_asset, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::MarketSellAssetContract)
    }

    async fn market_cancel_order(
        &self,
        params: MarketCancelOrderContract,
    ) -> TransportResult<RawTransaction> {
        let req = codec::market_cancel_order_to_proto(params);
        let sent = req.clone();
        let ext = retry_unary!(self, wallet_client, market_cancel_order, req)?;
        Self::raw_from_extention(ext, &sent, ContractKind::MarketCancelOrderContract)
    }

    async fn get_market_order_by_id(
        &self,
        order_id: B256,
    ) -> TransportResult<Option<MarketOrderInfo>> {
        let req = proto::BytesMessage { value: order_id.as_slice().to_vec() };
        let order = match retry_unary!(self, wallet_client, get_market_order_by_id, req) {
            Ok(order) => order,
            Err(err) if market_order_not_found(&err) => return Ok(None),
            Err(err) => return Err(err),
        };
        if order.order_id.is_empty() {
            return Ok(None);
        }
        Ok(Some(codec::market_order_from_proto(order)?))
    }

    async fn get_market_order_by_account(
        &self,
        address: Address,
    ) -> TransportResult<Vec<MarketOrderInfo>> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let list = retry_unary!(self, wallet_client, get_market_order_by_account, req)?;
        list.orders
            .into_iter()
            .map(codec::market_order_from_proto)
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_market_price_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> TransportResult<Vec<MarketPrice>> {
        let req = proto::MarketOrderPair {
            sell_token_id: sell_token_id.as_bytes().to_vec(),
            buy_token_id: buy_token_id.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_market_price_by_pair, req)?;
        Ok(list.prices.into_iter().map(codec::market_price_from_proto).collect())
    }

    async fn get_market_order_list_by_pair(
        &self,
        sell_token_id: &str,
        buy_token_id: &str,
    ) -> TransportResult<Vec<MarketOrderInfo>> {
        let req = proto::MarketOrderPair {
            sell_token_id: sell_token_id.as_bytes().to_vec(),
            buy_token_id: buy_token_id.as_bytes().to_vec(),
        };
        let list = retry_unary!(self, wallet_client, get_market_order_list_by_pair, req)?;
        list.orders
            .into_iter()
            .map(codec::market_order_from_proto)
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    async fn get_market_pair_list(&self) -> TransportResult<Vec<MarketOrderPair>> {
        let req = EmptyMessage {};
        let list = retry_unary!(self, wallet_client, get_market_pair_list, req)?;
        Ok(list.order_pair.into_iter().map(codec::market_order_pair_from_proto).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transfer(amount: i64) -> proto::TransferContract {
        proto::TransferContract {
            owner_address: vec![0x41; 21],
            to_address: vec![0x42; 21],
            amount,
        }
    }
    fn extention(
        kind: ContractKind,
        built: &impl prost::Message,
        count: usize,
    ) -> proto::TransactionExtention {
        let contract = proto::transaction::Contract {
            r#type: kind as i32,
            parameter: Some(prost_types::Any {
                type_url: String::new(),
                value: built.encode_to_vec(),
            }),
            ..Default::default()
        };
        proto::TransactionExtention {
            transaction: Some(proto::Transaction {
                raw_data: Some(proto::transaction::Raw {
                    contract: vec![contract; count],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn queried_signatures_are_not_appended_twice_when_reencoded() {
        let mut first = vec![1u8; 65];
        first[64] = 0;
        let mut second = vec![2u8; 65];
        second[64] = 1;
        let original = proto::Transaction {
            raw_data: Some(proto::transaction::Raw {
                contract: vec![proto::transaction::Contract::default()],
                ..Default::default()
            }),
            signature: vec![first.into(), second.into()],
            ret: vec![proto::transaction::Result::default()],
        };

        let signed = codec::signed_tx_lookup(original.clone(), &[]).unwrap().unwrap();
        let reencoded = signed_to_proto(&signed).unwrap();

        assert_eq!(reencoded.signature, original.signature);
        assert_eq!(reencoded.signature.len(), 2);
        assert_eq!(reencoded.ret, original.ret);
    }

    #[test]
    fn java_tron_missing_market_order_status_is_recognised() {
        let missing = TransportErrorKind::Rpc {
            code: RpcStatusCode::Internal,
            message: "order not found in store".into(),
        };
        let unrelated = TransportErrorKind::Rpc {
            code: RpcStatusCode::Internal,
            message: "database unavailable".into(),
        };

        assert!(market_order_not_found(&missing));
        assert!(!market_order_not_found(&unrelated));
    }

    #[test]
    fn accepts_the_contract_it_was_asked_to_build() {
        let sent = transfer(1);
        let ext = extention(ContractKind::TransferContract, &sent, 1);

        let raw =
            GrpcTransport::raw_from_extention(ext, &sent, ContractKind::TransferContract).unwrap();
        assert_ne!(raw.tx_id().as_slice(), &[0u8; 32]);
    }

    #[test]
    fn rejects_a_node_that_changes_the_amount() {
        let ext = extention(ContractKind::TransferContract, &transfer(1_000_000), 1);

        let err =
            GrpcTransport::raw_from_extention(ext, &transfer(1), ContractKind::TransferContract)
                .unwrap_err();
        assert!(
            matches!(err, TransportErrorKind::Malformed(ref m) if m.contains("different contract"))
        );
    }

    #[test]
    fn rejects_a_node_that_swaps_the_contract_type() {
        let sent = transfer(1);
        let ext = extention(ContractKind::TransferAssetContract, &sent, 1);

        let err = GrpcTransport::raw_from_extention(ext, &sent, ContractKind::TransferContract)
            .unwrap_err();
        assert!(matches!(err, TransportErrorKind::Malformed(ref m)
                if m.contains("TransferAssetContract") && m.contains("TransferContract")));
    }

    #[test]
    fn rejects_a_node_that_bundles_extra_contracts() {
        let sent = transfer(1);
        let ext = extention(ContractKind::TransferContract, &sent, 2);

        let err = GrpcTransport::raw_from_extention(ext, &sent, ContractKind::TransferContract)
            .unwrap_err();
        assert!(
            matches!(err, TransportErrorKind::Malformed(ref m) if m.contains("expected exactly"))
        );
    }

    #[test]
    fn a_deploy_may_come_back_with_the_address_the_node_derived() {
        let sent = proto::CreateSmartContract {
            owner_address: vec![0x41; 21],
            new_contract: Some(proto::SmartContract {
                origin_address: vec![0x41; 21],
                bytecode: vec![0x60, 0x80].into(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let mut returned = sent.clone();
        let contract = returned.new_contract.as_mut().unwrap();
        contract.contract_address = vec![0x41; 21];
        contract.code_hash = vec![0xab; 32];

        let ext = extention(ContractKind::CreateSmartContract, &returned, 1);
        let raw = GrpcTransport::raw_from_extention_with(
            ext,
            &sent,
            ContractKind::CreateSmartContract,
            |built| {
                if let Some(c) = built.new_contract.as_mut() {
                    c.contract_address = Vec::new();
                    c.code_hash = Vec::new();
                    c.trx_hash = Vec::new();
                    c.version = 0;
                }
            },
        )
        .unwrap();
        assert_ne!(raw.tx_id().as_slice(), &[0u8; 32]);
    }

    #[test]
    fn a_deploy_that_swaps_the_bytecode_is_still_rejected() {
        let sent = proto::CreateSmartContract {
            owner_address: vec![0x41; 21],
            new_contract: Some(proto::SmartContract {
                bytecode: vec![0x60, 0x80].into(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let mut returned = sent.clone();
        returned.new_contract.as_mut().unwrap().bytecode = vec![0xde, 0xad].into();

        let ext = extention(ContractKind::CreateSmartContract, &returned, 1);
        let err = GrpcTransport::raw_from_extention_with(
            ext,
            &sent,
            ContractKind::CreateSmartContract,
            |built| {
                if let Some(c) = built.new_contract.as_mut() {
                    c.contract_address = Vec::new();
                }
            },
        )
        .unwrap_err();
        assert!(
            matches!(err, TransportErrorKind::Malformed(ref m) if m.contains("different contract"))
        );
    }
}
