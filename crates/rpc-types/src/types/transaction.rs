//! Transaction request / raw / signed types.

use std::time::Duration;

use tronz_primitives::{Address, Bytes, RecoverableSignature, Trx, TxId};

use crate::{
    ResponseError,
    types::{
        BlockInfo,
        contract::{ContractKind, ContractType, OwnerField},
    },
};

/// Builder-stage transaction: all fields optional, filled progressively by
/// fillers before being finalized into a [`RawTransaction`].
#[derive(Clone, Debug, Default)]
pub struct TransactionRequest {
    /// The contract (operation) being performed.
    pub contract: Option<ContractType>,
    /// Maximum fee (energy + bandwidth) the sender will pay.
    pub fee_limit: Option<Trx>,
    /// Optional memo / note (`raw.data`).
    pub memo: Option<Bytes>,
    /// Permission id for multisig (`Contract.Permission_id`).
    pub permission_id: Option<i32>,
    /// Last 2 bytes of the reference block number.
    pub ref_block_bytes: Option<[u8; 2]>,
    /// Bytes 8..16 of the reference block hash.
    pub ref_block_hash: Option<[u8; 8]>,
    /// Expiration timestamp (unix ms).
    pub expiration: Option<i64>,
    /// Creation timestamp (unix ms).
    pub timestamp: Option<i64>,
}

impl TransactionRequest {
    /// Whether the contained contract type requires a `fee_limit`.
    pub fn contract_needs_fee_limit(&self) -> bool {
        self.contract.as_ref().is_some_and(|c| c.needs_fee_limit())
    }

    /// Set the contract (operation) to perform.
    pub fn with_contract(mut self, contract: ContractType) -> Self {
        self.contract = Some(contract);
        self
    }

    /// Set the maximum fee (energy + bandwidth) the sender will pay.
    pub fn with_fee_limit(mut self, fee_limit: Trx) -> Self {
        self.fee_limit = Some(fee_limit);
        self
    }

    /// Attach a memo / note.
    pub fn with_memo(mut self, memo: impl Into<Bytes>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Set the permission id for multisig transactions.
    pub fn with_permission_id(mut self, id: i32) -> Self {
        self.permission_id = Some(id);
        self
    }

    /// Fill TAPOS fields directly from a known block, bypassing `TaposFiller`.
    ///
    /// Use this when the caller already has a [`BlockInfo`] in hand — for
    /// example, an indexer that fetched the block to process it — so that no
    /// additional `get_now_block` network call is needed. `TaposFiller` will
    /// detect that the fields are already set and skip its own fetch.
    pub fn with_tapos(mut self, block: &BlockInfo, expiry: Duration) -> Self {
        self.ref_block_bytes = Some(block.ref_block_bytes());
        self.ref_block_hash = Some(block.ref_block_hash());
        self.timestamp = Some(block.timestamp);
        self.expiration = Some(block.timestamp + expiry.as_millis() as i64);
        self
    }
}

/// A fully-populated, node-built, ready-to-sign transaction.
///
/// Obtained from the gRPC transport after it calls a tx-building endpoint
/// (e.g. `freeze_balance_v2`). The node fills TAPOS, encodes `Transaction.raw`
/// as protobuf, and returns the hash (`txid`) and the raw protobuf bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTransaction {
    /// Expiration timestamp (unix ms).
    pub expiration: i64,
    /// Creation timestamp (unix ms).
    pub timestamp: i64,
    /// `sha256(prost_encode(Transaction.raw))` — the exact bytes to sign.
    ///
    /// Private, and derived rather than assigned: see [`Self::from_node_encoded`].
    tx_id: TxId,
    /// Prost-encoded `Transaction` (no signatures yet). Used to build the
    /// broadcast message by appending signatures.
    raw_proto: Bytes,
}

impl RawTransaction {
    /// Construct from the encoded `Transaction` a node built.
    ///
    /// The id is always computed here, from the bytes that will actually be
    /// broadcast — never taken from the node. A node that also states an id must
    /// agree with that computation, otherwise the signature would cover one
    /// transaction while another went out on the wire, which is exactly the
    /// signature the caller never agreed to give. Pass an empty `claimed_tx_id`
    /// when the response carries no id of its own.
    ///
    /// Public because the transport crates construct these, hidden because they
    /// are the only callers. It cannot be used to smuggle in an id of one's
    /// choosing: the id is derived from `encoded` on every call.
    #[doc(hidden)]
    pub fn from_node_encoded(
        encoded: impl Into<Bytes>,
        claimed_tx_id: &[u8],
    ) -> Result<Self, ResponseError> {
        Self::from_encoded(encoded.into(), claimed_tx_id).map(|decoded| decoded.raw)
    }

    /// Decodes an unsigned, unexecuted protobuf transaction.
    ///
    /// The raw data must round-trip exactly to preserve its transaction id.
    pub fn decode(encoded: impl Into<Bytes>) -> Result<Self, ResponseError> {
        let decoded = Self::from_encoded(encoded.into(), &[])?;

        if !decoded.signatures.is_empty() {
            return Err(ResponseError::Malformed(
                "raw transaction carries signatures; use SignedTransaction::decode".into(),
            ));
        }
        if decoded.has_results {
            return Err(ResponseError::Malformed(
                "raw transaction carries execution results; use SignedTransaction::decode".into(),
            ));
        }
        if !decoded.has_contract {
            return Err(ResponseError::Malformed(
                "transaction has no contract; these may be the bytes of an inner raw_data rather \
                 than of a whole Transaction"
                    .into(),
            ));
        }

        Ok(decoded.raw)
    }

    fn from_encoded(
        raw_proto: Bytes,
        claimed_tx_id: &[u8],
    ) -> Result<DecodedTransaction, ResponseError> {
        use prost::Message as _;
        use sha2::{Digest, Sha256};

        let encoded_raw_data = extract_raw_data(raw_proto.as_ref())?;
        let mut proto_tx = crate::proto::Transaction::decode(raw_proto.as_ref())?;
        let signatures = proto_tx
            .signature
            .drain(..)
            .map(|signature| {
                RecoverableSignature::from_bytes(&signature).map_err(|e| {
                    ResponseError::Malformed(format!("bad transaction signature: {e}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let has_results = !proto_tx.ret.is_empty();
        let raw_data = proto_tx
            .raw_data
            .as_ref()
            .ok_or_else(|| ResponseError::Malformed("transaction has no raw_data".into()))?;
        if raw_data.encode_to_vec() != encoded_raw_data {
            return Err(ResponseError::Malformed(
                "raw_data contains fields or encoding this build cannot preserve".into(),
            ));
        }

        let tx_id_bytes: [u8; 32] = Sha256::digest(encoded_raw_data).into();
        let expiration = raw_data.expiration;
        let timestamp = raw_data.timestamp;
        let has_contract = !raw_data.contract.is_empty();

        if !claimed_tx_id.is_empty() && claimed_tx_id != tx_id_bytes {
            return Err(ResponseError::Malformed(
                "node txid does not match the transaction it returned".into(),
            ));
        }

        let raw = Self {
            expiration,
            timestamp,
            tx_id: TxId::from(tx_id_bytes),
            raw_proto: proto_tx.encode_to_vec().into(),
        };

        Ok(DecodedTransaction { raw, signatures, has_results, has_contract })
    }

    /// The transaction id — `sha256` of the encoded `Transaction.raw`.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Returns the encoded protobuf transaction without signatures.
    #[doc(hidden)]
    pub fn encoded(&self) -> &[u8] {
        self.raw_proto.as_ref()
    }

    /// Decodes the transaction contents.
    pub fn details(&self) -> Result<TransactionDetails, ResponseError> {
        use prost::Message as _;

        let raw_data = crate::proto::Transaction::decode(self.raw_proto.as_ref())?
            .raw_data
            .ok_or_else(|| ResponseError::Malformed("transaction has no raw_data".into()))?;

        let contracts = raw_data
            .contract
            .into_iter()
            .map(contract_details)
            .collect::<Result<Vec<_>, ResponseError>>()?;

        Ok(TransactionDetails {
            fee_limit: Trx::from_sun_unchecked(raw_data.fee_limit),
            memo: Bytes::from(raw_data.data),
            contracts,
        })
    }

    /// Apply fee, memo, permission, and optional TAPOS overrides from a filled
    /// [`TransactionRequest`] to this raw transaction, and re-derive its id.
    ///
    /// When any field is set, the `Transaction.raw` proto bytes are decoded,
    /// modified, and re-encoded; the `tx_id` (`sha256` of the new raw bytes) is
    /// recomputed so that the signature covers the updated payload.
    ///
    /// Checking that the node built the contract that was asked for is the job of
    /// the transport that understands the response, not of this method.
    #[doc(hidden)]
    pub fn apply_request_fields(
        &mut self,
        request: &TransactionRequest,
    ) -> Result<(), ResponseError> {
        use prost::Message as _;
        use sha2::{Digest, Sha256};

        if request.fee_limit.is_none()
            && request.memo.is_none()
            && request.permission_id.is_none()
            && request.ref_block_bytes.is_none()
            && request.ref_block_hash.is_none()
            && request.timestamp.is_none()
            && request.expiration.is_none()
        {
            return Ok(());
        }

        let mut tx = crate::proto::Transaction::decode(self.raw_proto.as_ref())?;

        if let Some(ref mut raw_data) = tx.raw_data {
            if let Some(value) = request.fee_limit {
                raw_data.fee_limit = value.as_sun();
            }
            if let Some(memo) = &request.memo {
                raw_data.data = memo.clone().into();
            }
            if let Some(pid) = request.permission_id {
                let contract = raw_data.contract.first_mut().ok_or_else(|| {
                    ResponseError::Malformed(
                        "node returned a transaction with no contract to set permission_id on"
                            .into(),
                    )
                })?;
                contract.permission_id = pid;
            }
            if let Some(bytes) = request.ref_block_bytes {
                raw_data.ref_block_bytes = bytes.to_vec();
            }
            if let Some(hash) = request.ref_block_hash {
                raw_data.ref_block_hash = hash.to_vec();
            }
            if let Some(value) = request.timestamp {
                raw_data.timestamp = value;
            }
            if let Some(value) = request.expiration {
                raw_data.expiration = value;
            }

            self.timestamp = raw_data.timestamp;
            self.expiration = raw_data.expiration;

            let new_tx_id_bytes: [u8; 32] = Sha256::digest(raw_data.encode_to_vec()).into();
            self.tx_id = TxId::from(new_tx_id_bytes);
        } else {
            return Err(ResponseError::Malformed("missing raw_data in Transaction".into()));
        }

        self.raw_proto = tx.encode_to_vec().into();
        Ok(())
    }
}

fn extract_raw_data(encoded: &[u8]) -> Result<&[u8], ResponseError> {
    let mut cursor = 0;
    let mut raw_data = None;
    while cursor < encoded.len() {
        let key = read_varint(encoded, &mut cursor)?;
        let field = key >> 3;
        let wire = key & 7;
        match wire {
            0 => {
                read_varint(encoded, &mut cursor)?;
            }
            1 => skip(encoded, &mut cursor, 8)?,
            2 => {
                let len = usize::try_from(read_varint(encoded, &mut cursor)?)
                    .map_err(|_| ResponseError::Malformed("protobuf field is too large".into()))?;
                let start = cursor;
                skip(encoded, &mut cursor, len)?;
                if field == 1 {
                    if raw_data.is_some() {
                        return Err(ResponseError::Malformed(
                            "transaction has repeated raw_data fields".into(),
                        ));
                    }
                    raw_data = Some(&encoded[start..cursor]);
                }
            }
            5 => skip(encoded, &mut cursor, 4)?,
            _ => {
                return Err(ResponseError::Malformed(format!(
                    "unsupported protobuf wire type {wire}"
                )));
            }
        }
    }
    raw_data.ok_or_else(|| ResponseError::Malformed("transaction has no raw_data".into()))
}

fn read_varint(encoded: &[u8], cursor: &mut usize) -> Result<u64, ResponseError> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *encoded
            .get(*cursor)
            .ok_or_else(|| ResponseError::Malformed("truncated protobuf varint".into()))?;
        *cursor += 1;
        if shift == 63 && byte > 1 {
            return Err(ResponseError::Malformed("protobuf varint overflows u64".into()));
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(ResponseError::Malformed("protobuf varint is too long".into()))
}

fn skip(encoded: &[u8], cursor: &mut usize, len: usize) -> Result<(), ResponseError> {
    let end = cursor
        .checked_add(len)
        .filter(|end| *end <= encoded.len())
        .ok_or_else(|| ResponseError::Malformed("truncated protobuf field".into()))?;
    *cursor = end;
    Ok(())
}

/// Decoded transaction contents.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TransactionDetails {
    /// Maximum fee paid by the sender.
    pub fee_limit: Trx,
    /// The memo (`raw_data.data`), empty if there is none.
    pub memo: Bytes,
    /// The operations carried by the transaction.
    pub contracts: Vec<TransactionContractDetails>,
}

/// One operation within a [`TransactionDetails`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TransactionContractDetails {
    /// Which native contract this is.
    pub kind: ContractKind,
    /// The account the operation acts for, if known.
    pub owner: Option<Address>,
}

/// A signed transaction ready to broadcast.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedTransaction {
    /// The signed raw transaction.
    pub raw: RawTransaction,
    /// One signature per signer (multisig may have more than one).
    pub signatures: Vec<RecoverableSignature>,
}

impl SignedTransaction {
    pub(crate) fn from_node_encoded(
        encoded: impl Into<Bytes>,
        claimed_tx_id: &[u8],
    ) -> Result<Self, ResponseError> {
        let decoded = RawTransaction::from_encoded(encoded.into(), claimed_tx_id)?;
        Ok(Self { raw: decoded.raw, signatures: decoded.signatures })
    }

    /// Decodes a signed protobuf transaction.
    pub fn decode(encoded: impl Into<Bytes>) -> Result<Self, ResponseError> {
        let decoded = RawTransaction::from_encoded(encoded.into(), &[])?;
        if !decoded.has_contract {
            return Err(ResponseError::Malformed("transaction has no contract".into()));
        }
        Ok(Self { raw: decoded.raw, signatures: decoded.signatures })
    }

    /// Builds the protobuf transaction.
    #[doc(hidden)]
    pub fn to_proto(&self) -> Result<crate::proto::Transaction, ResponseError> {
        use prost::Message as _;

        let mut tx = crate::proto::Transaction::decode(self.raw.raw_proto.as_ref())?;
        tx.signature
            .extend(self.signatures.iter().map(|signature| signature.to_bytes().to_vec().into()));
        Ok(tx)
    }

    /// Encodes the complete protobuf transaction.
    pub fn encode(&self) -> Result<Bytes, ResponseError> {
        use prost::Message as _;

        Ok(self.to_proto()?.encode_to_vec().into())
    }

    /// Returns the encoded length, including signatures.
    pub fn encoded_len(&self) -> u64 {
        self.raw.raw_proto.len() as u64 + self.signatures.len() as u64 * 67
    }

    /// Returns the encoded length, including signatures.
    #[deprecated(since = "0.6.0", note = "use `encoded_len`")]
    pub fn byte_size(&self) -> u64 {
        self.encoded_len()
    }
}

struct DecodedTransaction {
    raw: RawTransaction,
    signatures: Vec<RecoverableSignature>,
    has_results: bool,
    has_contract: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
struct OwnerAtField1 {
    #[prost(bytes = "vec", tag = "1")]
    owner_address: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct OwnerAtField2 {
    #[prost(bytes = "vec", tag = "2")]
    owner_address: Vec<u8>,
}

fn contract_details(
    contract: crate::proto::transaction::Contract,
) -> Result<TransactionContractDetails, ResponseError> {
    use prost::Message as _;

    let kind = ContractKind::from(contract.r#type);

    let owner_bytes = match (kind.owner_field(), contract.parameter) {
        (Some(OwnerField::First), Some(any)) => {
            OwnerAtField1::decode(any.value.as_ref())?.owner_address
        }
        (Some(OwnerField::Second), Some(any)) => {
            OwnerAtField2::decode(any.value.as_ref())?.owner_address
        }
        _ => Vec::new(),
    };

    let owner =
        if owner_bytes.is_empty() {
            None
        } else {
            Some(Address::from_slice(&owner_bytes).map_err(|e| {
                ResponseError::Malformed(format!("bad contract owner address: {e}"))
            })?)
        };

    Ok(TransactionContractDetails { kind, owner })
}

/// Bytes reserved by java-tron for transaction results.
pub const MAX_RESULT_SIZE_IN_TX: u64 = 64;

#[cfg(test)]
mod tests {
    use prost::Message as _;
    use sha2::Digest as _;

    use super::*;

    const RAW_DATA_A: &str = concat!(
        "0a02688722088ceac61f2c0e6dc84080e8a0fdfa335a65080112610a2d747970652e676f6f676c65617069732e",
        "636f6d2f70726f746f636f6c2e5472616e73666572436f6e747261637412300a15419105de9072d7b0d461661d",
        "f9a1053e1079ede9a512154146d1e5d0c14d65bda777fa406a8c9f0fad17fe7f180170e9a59dfdfa33",
    );
    const SIGNATURE_A: &str = concat!(
        "f6cdd999bbe3a05d0476f302bb149c9473644fd62067493cd560dffa475efbc7068805e4cb52dd5e240ac4eee8",
        "41839f065ce5b10e8bfbf83e1f9176a374b73101",
    );

    const RAW_DATA_B: &str = concat!(
        "0a02688722088ceac61f2c0e6dc840a4faa0fdfa335a68080112640a2d747970652e676f6f676c65617069732e",
        "636f6d2f70726f746f636f6c2e5472616e73666572436f6e747261637412330a1541a346f2bd7d43a5d90b7c57",
        "a18196be96b2840e611215414f8ba145a47f917234be64dcdbf86f956479a1b41897b5d13f70b0ed99fdfa3390",
        "01c0c39307",
    );
    const SIGNATURE_B: &str = concat!(
        "62239f5743155e244df3bf6d3a41cfcc9c373b95dc9a6278fc49ab29210bdbfd7ed52743ecd41f7ca919145f1d",
        "2fc9ee41416c2889345600af7bdf32857f174100",
    );

    fn signed(raw_data: &str, signature: &str) -> SignedTransaction {
        let raw_data =
            crate::proto::transaction::Raw::decode(hex::decode(raw_data).unwrap().as_slice())
                .unwrap();
        let encoded = crate::proto::Transaction { raw_data: Some(raw_data), ..Default::default() }
            .encode_to_vec();

        SignedTransaction {
            raw: RawTransaction::from_node_encoded(encoded, &[]).unwrap(),
            signatures: vec![
                RecoverableSignature::from_bytes(&hex::decode(signature).unwrap()).unwrap(),
            ],
        }
    }

    #[test]
    fn bandwidth_charged_is_the_wire_size_plus_the_result_allowance() {
        for (raw_data, signature, charged) in
            [(RAW_DATA_A, SIGNATURE_A, 265), (RAW_DATA_B, SIGNATURE_B, 274)]
        {
            let tx = signed(raw_data, signature);

            assert_eq!(tx.encoded_len() + MAX_RESULT_SIZE_IN_TX, charged);
        }
    }

    #[test]
    fn every_signature_costs_the_same_number_of_bytes() {
        let mut tx = signed(RAW_DATA_A, SIGNATURE_A);
        let one = tx.encoded_len();
        tx.signatures.push(tx.signatures[0]);

        assert_eq!(tx.encoded_len() - one, 67);
    }

    fn node_tx(expiration: i64, timestamp: i64) -> Vec<u8> {
        crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw {
                expiration,
                timestamp,
                contract: vec![crate::proto::transaction::Contract::default()],
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec()
    }

    #[test]
    fn the_id_is_computed_from_the_bytes_that_will_be_broadcast() {
        let raw = RawTransaction::from_node_encoded(node_tx(9, 8), &[]).unwrap();

        let raw_data =
            crate::proto::Transaction::decode(raw.raw_proto.as_ref()).unwrap().raw_data.unwrap();
        let expected: [u8; 32] = sha2::Sha256::digest(raw_data.encode_to_vec()).into();

        assert_eq!(raw.tx_id().as_slice(), expected);
        assert_eq!((raw.expiration, raw.timestamp), (9, 8));
    }

    #[test]
    fn decoding_a_transaction_reproduces_the_id_the_chain_knows_it_by() {
        let raw_data =
            crate::proto::transaction::Raw::decode(hex::decode(RAW_DATA_A).unwrap().as_slice())
                .unwrap();
        let encoded = crate::proto::Transaction { raw_data: Some(raw_data), ..Default::default() }
            .encode_to_vec();

        let tx = RawTransaction::decode(encoded).unwrap();

        assert_eq!(
            hex::encode(tx.tx_id().as_slice()),
            "478b4dd129d8e03af095b3967bd8d09b9be89dbddc0332f74a66f134a993b343"
        );
    }

    #[test]
    fn raw_decode_rejects_signatures_but_signed_decode_separates_them() {
        let mut proto_tx = crate::proto::Transaction::decode(node_tx(9, 8).as_slice()).unwrap();
        let mut signature = vec![1u8; 65];
        signature[64] = 0;
        proto_tx.signature.push(signature.into());
        let encoded = proto_tx.encode_to_vec();

        assert!(matches!(
            RawTransaction::decode(encoded.clone()),
            Err(ResponseError::Malformed(message)) if message.contains("signatures")
        ));

        let signed = SignedTransaction::decode(encoded).unwrap();
        assert_eq!(signed.signatures.len(), 1);
        let raw_proto = crate::proto::Transaction::decode(signed.raw.encoded()).unwrap();
        assert!(raw_proto.signature.is_empty());
        let complete =
            crate::proto::Transaction::decode(signed.encode().unwrap().as_ref()).unwrap();
        assert_eq!(complete.signature.len(), 1);
    }

    #[test]
    fn raw_decode_rejects_results_while_signed_decode_preserves_them() {
        let mut proto_tx = crate::proto::Transaction::decode(node_tx(9, 8).as_slice()).unwrap();
        proto_tx.ret.push(crate::proto::transaction::Result::default());
        let encoded = proto_tx.encode_to_vec();

        assert!(matches!(
            RawTransaction::decode(encoded.clone()),
            Err(ResponseError::Malformed(message)) if message.contains("execution results")
        ));

        let signed = SignedTransaction::decode(encoded).unwrap();
        let raw_proto = crate::proto::Transaction::decode(signed.raw.encoded()).unwrap();
        assert_eq!(raw_proto.ret.len(), 1);
        let complete =
            crate::proto::Transaction::decode(signed.encode().unwrap().as_ref()).unwrap();
        assert_eq!(complete.ret.len(), 1);
    }

    #[test]
    fn signed_decode_rejects_one_bad_signature_instead_of_dropping_it() {
        let mut proto_tx = crate::proto::Transaction::decode(node_tx(9, 8).as_slice()).unwrap();
        proto_tx.signature.push(vec![1u8; 64].into());

        assert!(matches!(
            SignedTransaction::decode(proto_tx.encode_to_vec()),
            Err(ResponseError::Malformed(message)) if message.contains("signature")
        ));
    }

    #[test]
    fn decode_rejects_a_raw_field_this_build_would_drop() {
        let raw = crate::proto::transaction::Raw {
            contract: vec![crate::proto::transaction::Contract::default()],
            ..Default::default()
        };
        let mut raw_bytes = raw.encode_to_vec();
        raw_bytes.extend_from_slice(&[0xa0, 0x06, 0x01]); // unknown field 100 = 1
        assert!(raw_bytes.len() < 128);
        let mut encoded = vec![0x0a, raw_bytes.len() as u8];
        encoded.extend_from_slice(&raw_bytes);

        assert!(matches!(
            SignedTransaction::decode(encoded),
            Err(ResponseError::Malformed(message)) if message.contains("cannot preserve")
        ));
    }

    #[test]
    fn the_bytes_of_an_inner_raw_data_are_not_a_transaction() {
        assert!(RawTransaction::decode(hex::decode(RAW_DATA_A).unwrap()).is_err());
    }

    #[test]
    fn a_transaction_with_no_contract_is_not_something_a_caller_can_have_meant() {
        let encoded = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw {
                expiration: 9,
                timestamp: 8,
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();

        let err = RawTransaction::decode(encoded.clone()).unwrap_err();
        assert!(matches!(err, ResponseError::Malformed(ref m) if m.contains("no contract")));

        assert!(RawTransaction::from_node_encoded(encoded, &[]).is_ok());
    }

    fn details_of(contract: crate::proto::transaction::Contract) -> TransactionDetails {
        let encoded = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw {
                fee_limit: 150_000_000,
                data: b"memo".to_vec().into(),
                contract: vec![contract],
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();

        RawTransaction::from_node_encoded(encoded, &[]).unwrap().details().unwrap()
    }

    fn parameter(message: &impl prost::Message) -> crate::proto::transaction::Contract {
        crate::proto::transaction::Contract {
            parameter: Some(prost_types::Any {
                type_url: String::new(),
                value: message.encode_to_vec(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn details_read_the_kind_owner_fee_limit_and_memo_of_a_transfer() {
        let owner = crate::proto::TransferContract {
            owner_address: hex::decode("419105de9072d7b0d461661df9a1053e1079ede9a5").unwrap(),
            to_address: hex::decode("4146d1e5d0c14d65bda777fa406a8c9f0fad17fe7f").unwrap(),
            amount: 1,
        };
        let contract = crate::proto::transaction::Contract { r#type: 1, ..parameter(&owner) };

        let details = details_of(contract);

        assert_eq!(details.fee_limit, Trx::from_sun_unchecked(150_000_000));
        assert_eq!(details.memo.as_ref(), b"memo");
        let [only] = details.contracts.as_slice() else { panic!("expected one contract") };
        assert_eq!(only.kind, ContractKind::Transfer);
        assert_eq!(
            only.owner.map(|a| a.to_hex()),
            Some("419105de9072d7b0d461661df9a1053e1079ede9a5".to_string())
        );
    }

    #[test]
    fn a_trc10_transfers_asset_name_is_not_mistaken_for_its_owner() {
        let asset_name = hex::decode("41ffffffffffffffffffffffffffffffffffffffff").unwrap();
        let owner_hex = "419105de9072d7b0d461661df9a1053e1079ede9a5";
        let transfer = crate::proto::TransferAssetContract {
            asset_name,
            owner_address: hex::decode(owner_hex).unwrap(),
            to_address: hex::decode("4146d1e5d0c14d65bda777fa406a8c9f0fad17fe7f").unwrap(),
            amount: 1,
        };
        let contract = crate::proto::transaction::Contract { r#type: 2, ..parameter(&transfer) };

        let details = details_of(contract);

        let [only] = details.contracts.as_slice() else { panic!("expected one contract") };
        assert_eq!(only.kind, ContractKind::TransferAsset);
        assert_eq!(only.owner.map(|a| a.to_hex()), Some(owner_hex.to_string()));
    }

    #[test]
    fn a_contract_type_this_build_does_not_know_still_decodes() {
        let contract = crate::proto::transaction::Contract {
            r#type: 4242,
            ..parameter(&crate::proto::TransferContract::default())
        };

        let details = details_of(contract);

        let [only] = details.contracts.as_slice() else { panic!("expected one contract") };
        assert_eq!(only.kind, ContractKind::Unknown(4242));
        assert_eq!(only.owner, None, "where the owner sits is not knowable");
        assert_eq!(only.kind.to_string(), "Unknown(4242)");
    }

    #[test]
    fn a_node_claiming_an_id_it_did_not_build_is_rejected() {
        let err = RawTransaction::from_node_encoded(node_tx(9, 8), &[7u8; 32]).unwrap_err();
        assert!(matches!(err, ResponseError::Malformed(ref m) if m.contains("txid")));
    }

    #[test]
    fn a_matching_claimed_id_is_accepted() {
        let raw = RawTransaction::from_node_encoded(node_tx(9, 8), &[]).unwrap();
        let again =
            RawTransaction::from_node_encoded(node_tx(9, 8), raw.tx_id().as_slice()).unwrap();
        assert_eq!(raw.tx_id(), again.tx_id());
    }

    #[test]
    fn a_transaction_without_raw_data_has_nothing_to_sign() {
        let empty = crate::proto::Transaction::default().encode_to_vec();
        let err = RawTransaction::from_node_encoded(empty, &[]).unwrap_err();
        assert!(matches!(err, ResponseError::Malformed(ref m) if m.contains("raw_data")));
    }

    #[test]
    fn permission_id_is_never_dropped_on_the_floor() {
        let tx = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw::default()),
            ..Default::default()
        };
        let mut raw = RawTransaction::from_node_encoded(tx.encode_to_vec(), &[]).unwrap();

        let request = TransactionRequest { permission_id: Some(2), ..Default::default() };
        let err = raw.apply_request_fields(&request).unwrap_err();
        assert!(matches!(err, ResponseError::Malformed(ref m) if m.contains("permission_id")));
    }

    #[test]
    fn applies_explicit_tapos_fields_to_node_built_transaction() {
        let tx = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw {
                timestamp: 1,
                expiration: 2,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut raw = RawTransaction::from_node_encoded(tx.encode_to_vec(), &[]).unwrap();

        let request = TransactionRequest {
            memo: Some(Bytes::from_static(b"memo")),
            ref_block_bytes: Some([0xaa, 0xbb]),
            ref_block_hash: Some([1, 2, 3, 4, 5, 6, 7, 8]),
            timestamp: Some(10),
            expiration: Some(20),
            ..Default::default()
        };
        raw.apply_request_fields(&request).unwrap();

        let decoded = crate::proto::Transaction::decode(raw.raw_proto.as_ref()).unwrap();
        let data = decoded.raw_data.unwrap();
        assert_eq!(data.ref_block_bytes, vec![0xaa, 0xbb]);
        assert_eq!(data.ref_block_hash, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(data.data.as_ref(), b"memo");
        assert_eq!(data.timestamp, 10);
        assert_eq!(data.expiration, 20);
        assert_eq!(raw.timestamp, 10);
        assert_eq!(raw.expiration, 20);
        assert_ne!(raw.tx_id(), TxId::from([0; 32]));
    }
}
