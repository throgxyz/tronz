//! tonic-backed gRPC transport for `protocol.WalletSolidity`.

use tronz_primitives::{Address, ResourceCode, Trx, TxId};

use super::{GrpcCore, GrpcTransportConfig, RetryConfig, codec, light_block};
use crate::{
    error::TransportErrorKind,
    proto::{self, EmptyMessage},
    transport::SolidityTransport,
    types::{
        AccountInfo, BlockInfo, ConstantCallResult, DelegatedResource, DelegatedResourceIndex,
        SignedTransaction, TransactionInfo, TriggerSmartContract, WitnessInfo,
    },
};

macro_rules! solidity_unary {
    ($self:ident, $method:ident, $req:expr) => {{
        let req = $req;
        $self
            .core
            .call_with_retry(|| {
                let mut client = $self.core.wallet_solidity_client();
                let req = req.clone();
                async move { Ok(client.$method(req).await?.into_inner()) }
            })
            .await
    }};
}

/// gRPC transport targeting TRON's SolidityNode `protocol.WalletSolidity` service.
#[derive(Clone)]
pub struct SolidityGrpcTransport {
    core: GrpcCore,
}

impl SolidityGrpcTransport {
    /// Connect with the default transport configuration.
    ///
    /// Use [`builder`](Self::builder) to customize it.
    pub async fn connect(uri: impl AsRef<str>) -> Result<Self, TransportErrorKind> {
        Self::connect_with_config(uri, GrpcTransportConfig::default()).await
    }

    /// Start a pre-connect [`SolidityGrpcTransportBuilder`].
    pub fn builder() -> SolidityGrpcTransportBuilder {
        SolidityGrpcTransportBuilder::default()
    }

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
}

/// Pre-connect builder for [`SolidityGrpcTransport`].
#[derive(Clone, Debug, Default)]
pub struct SolidityGrpcTransportBuilder {
    config: GrpcTransportConfig,
}

impl SolidityGrpcTransportBuilder {
    /// Override the connect (handshake) timeout.
    pub fn with_connect_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Override the per-call request timeout.
    pub fn with_request_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    /// Override the retry policy.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.config.retry = retry;
        self
    }

    /// Add equivalent SolidityNode endpoints for client-side failover.
    pub fn with_endpoints(mut self, endpoints: Vec<String>) -> Self {
        self.config.endpoints = endpoints;
        self
    }

    /// Optionally set the TronGrid API key.
    pub fn maybe_api_key(mut self, key: Option<impl Into<String>>) -> Self {
        self.config.api_key = key.map(Into::into);
        self
    }

    /// Connect using the accumulated configuration.
    pub async fn connect(
        self,
        uri: impl AsRef<str>,
    ) -> Result<SolidityGrpcTransport, TransportErrorKind> {
        SolidityGrpcTransport::connect_with_config(uri, self.config).await
    }
}

impl crate::transport::private::Sealed for SolidityGrpcTransport {}

impl SolidityTransport for SolidityGrpcTransport {
    type Error = TransportErrorKind;

    async fn get_now_block(&self) -> Result<BlockInfo, Self::Error> {
        let block: light_block::BlockSummaryProto = self
            .core
            .unary(
                EmptyMessage::default(),
                "/protocol.WalletSolidity/GetNowBlock2",
                "protocol.WalletSolidity",
                "GetNowBlock2",
            )
            .await?;
        block.into_block_info(None)
    }

    async fn get_block_by_number(&self, num: i64) -> Result<BlockInfo, Self::Error> {
        let block: light_block::BlockSummaryProto = self
            .core
            .unary(
                proto::NumberMessage { num },
                "/protocol.WalletSolidity/GetBlockByNum2",
                "protocol.WalletSolidity",
                "GetBlockByNum2",
            )
            .await?;
        block.into_block_info(None)
    }

    async fn get_account(&self, address: Address) -> Result<AccountInfo, Self::Error> {
        let req = proto::Account { address: address.as_bytes().to_vec(), ..Default::default() };
        let account = solidity_unary!(self, get_account, req)?;
        codec::account_from_proto(account, address)
    }

    async fn get_transaction_by_id(&self, tx_id: TxId) -> Result<SignedTransaction, Self::Error> {
        let req = proto::BytesMessage { value: tx_id.as_slice().to_vec() };
        let tx = solidity_unary!(self, get_transaction_by_id, req)?;
        codec::signed_tx_from_proto(tx)
    }

    async fn get_transaction_info(
        &self,
        tx_id: TxId,
    ) -> Result<Option<TransactionInfo>, Self::Error> {
        let req = proto::BytesMessage { value: tx_id.as_slice().to_vec() };
        let info = solidity_unary!(self, get_transaction_info_by_id, req)?;
        codec::transaction_info_from_proto(info)
    }

    async fn get_transaction_info_by_block_num(
        &self,
        block_num: i64,
    ) -> Result<Vec<TransactionInfo>, Self::Error> {
        let req = proto::NumberMessage { num: block_num };
        let list = solidity_unary!(self, get_transaction_info_by_block_num, req)?;
        list.transaction_info
            .into_iter()
            .filter_map(|info| codec::transaction_info_from_proto(info).transpose())
            .collect()
    }

    async fn get_transaction_count_by_block_num(&self, block_num: i64) -> Result<u64, Self::Error> {
        let req = proto::NumberMessage { num: block_num };
        let res = solidity_unary!(self, get_transaction_count_by_block_num, req)?;
        Ok(res.num as u64)
    }

    async fn trigger_constant_contract(
        &self,
        params: TriggerSmartContract,
    ) -> Result<ConstantCallResult, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let ext = solidity_unary!(self, trigger_constant_contract, req)?;
        codec::constant_result_from_extention(ext)
    }

    async fn estimate_energy(&self, params: TriggerSmartContract) -> Result<i64, Self::Error> {
        let req = codec::trigger_smart_contract_to_proto(params);
        let msg = solidity_unary!(self, estimate_energy, req)?;
        codec::check_return(msg.result)?;
        Ok(msg.energy_required)
    }

    async fn list_witnesses(&self) -> Result<Vec<WitnessInfo>, Self::Error> {
        let list = solidity_unary!(self, list_witnesses, EmptyMessage::default())?;
        Ok(list.witnesses.into_iter().filter_map(codec::witness_from_proto).collect())
    }

    async fn get_paginated_now_witness_list(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<WitnessInfo>, Self::Error> {
        let req = proto::PaginatedMessage { offset, limit };
        let list = solidity_unary!(self, get_paginated_now_witness_list, req)?;
        Ok(list.witnesses.into_iter().filter_map(codec::witness_from_proto).collect())
    }

    async fn get_delegated_resource_v1(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Vec<DelegatedResource>, Self::Error> {
        let req = proto::DelegatedResourceMessage {
            from_address: from.as_bytes().to_vec(),
            to_address: to.as_bytes().to_vec(),
        };
        let list = solidity_unary!(self, get_delegated_resource, req)?;
        list.delegated_resource.into_iter().map(codec::delegated_resource_from_proto).collect()
    }

    async fn get_delegated_resource_index_v1(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex, Self::Error> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let idx = solidity_unary!(self, get_delegated_resource_account_index, req)?;
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
        let list = solidity_unary!(self, get_delegated_resource_v2, req)?;
        list.delegated_resource.into_iter().map(codec::delegated_resource_from_proto).collect()
    }

    async fn get_delegated_resource_index(
        &self,
        address: Address,
    ) -> Result<DelegatedResourceIndex, Self::Error> {
        let req = proto::BytesMessage { value: address.as_bytes().to_vec() };
        let idx = solidity_unary!(self, get_delegated_resource_account_index_v2, req)?;
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
        let res = solidity_unary!(self, get_can_delegated_max_size, req)?;
        Ok(Trx::from_sun_unchecked(res.max_size))
    }

    async fn get_available_unfreeze_count(&self, address: Address) -> Result<i64, Self::Error> {
        let req = proto::GetAvailableUnfreezeCountRequestMessage {
            owner_address: address.as_bytes().to_vec(),
        };
        let res = solidity_unary!(self, get_available_unfreeze_count, req)?;
        Ok(res.count)
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
        let res = solidity_unary!(self, get_can_withdraw_unfreeze_amount, req)?;
        Ok(Trx::from_sun_unchecked(res.amount))
    }
}
