//! Transaction request / raw / signed types.

use std::time::Duration;

use tronz_primitives::{Bytes, RecoverableSignature, Trx, TxId};

use crate::{
    ResponseError,
    types::{BlockInfo, contract::ContractType},
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
#[derive(Clone, Debug)]
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
        use prost::Message as _;
        use sha2::{Digest, Sha256};

        let raw_proto = encoded.into();
        let raw_data = crate::proto::Transaction::decode(raw_proto.as_ref())?
            .raw_data
            .ok_or_else(|| ResponseError::Malformed("transaction has no raw_data".into()))?;

        let tx_id_bytes: [u8; 32] = Sha256::digest(raw_data.encode_to_vec()).into();

        if !claimed_tx_id.is_empty() && claimed_tx_id != tx_id_bytes {
            return Err(ResponseError::Malformed(
                "node txid does not match the transaction it returned".into(),
            ));
        }

        Ok(Self {
            expiration: raw_data.expiration,
            timestamp: raw_data.timestamp,
            tx_id: TxId::from(tx_id_bytes),
            raw_proto,
        })
    }

    /// The transaction id — `sha256` of the encoded `Transaction.raw`.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// The encoded `Transaction`, without signatures.
    ///
    /// These are the bytes [`Self::tx_id`] is derived from, and the ones a
    /// transport appends signatures to in order to broadcast.
    #[doc(hidden)]
    pub fn encoded(&self) -> &[u8] {
        self.raw_proto.as_ref()
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

/// A signed transaction ready to broadcast.
#[derive(Clone, Debug)]
pub struct SignedTransaction {
    /// The signed raw transaction.
    pub raw: RawTransaction,
    /// One signature per signer (multisig may have more than one).
    pub signatures: Vec<RecoverableSignature>,
}

impl SignedTransaction {
    /// Estimate the bandwidth (bytes) this transaction will consume on-chain.
    ///
    /// Bandwidth equals the byte size of the fully-serialized protobuf
    /// `Transaction` (including all signatures).  This matches the formula
    /// used by the TRON node and trident's `estimateBandwidth`.
    pub fn byte_size(&self) -> u64 {
        use prost::Message as _;

        let mut proto_tx = match crate::proto::Transaction::decode(self.raw.raw_proto.as_ref()) {
            Ok(tx) => tx,
            Err(_) => {
                debug_assert!(false, "SignedTransaction.raw_proto failed to decode");
                return 0;
            }
        };
        for sig in &self.signatures {
            proto_tx.signature.push(sig.to_bytes().to_vec().into());
        }
        proto_tx.encoded_len() as u64
    }
}

#[cfg(test)]
mod tests {
    use prost::Message as _;
    use sha2::Digest as _;

    use super::*;

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
