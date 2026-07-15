//! Transaction request / raw / signed types.

use std::time::Duration;

use tronz_primitives::{Bytes, RecoverableSignature, Trx, TxId};

use crate::types::{BlockInfo, contract::ContractType};

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

    // --- set by TaposFiller (only needed for client-built txs) ---
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

    /// Fill TAPOS fields directly from a known block, bypassing [`TaposFiller`].
    ///
    /// Use this when the caller already has a [`BlockInfo`] in hand — for
    /// example, an indexer that fetched the block to process it — so that no
    /// additional `get_now_block` network call is needed.  [`TaposFiller`] will
    /// detect that the fields are already set and skip its own fetch.
    ///
    /// [`TaposFiller`]: crate::fillers::TaposFiller
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

    // --- internal transport fields ---
    /// `sha256(prost_encode(Transaction.raw))` — the exact bytes to sign.
    pub(crate) tx_id: TxId,
    /// Prost-encoded `Transaction` (no signatures yet). Used to build the
    /// broadcast message by appending signatures.
    pub(crate) raw_proto: Bytes,
}

impl RawTransaction {
    /// Construct from a `TransactionExtention` returned by the node.
    pub(crate) fn from_proto_extention(
        txid: Vec<u8>,
        raw_proto: impl Into<Bytes>,
        expiration: i64,
        timestamp: i64,
    ) -> Result<Self, crate::error::TransportErrorKind> {
        use crate::error::TransportErrorKind;

        let tx_id_bytes: [u8; 32] = txid
            .try_into()
            .map_err(|_| TransportErrorKind::Malformed("txid must be 32 bytes".into()))?;

        Ok(Self {
            expiration,
            timestamp,
            tx_id: TxId::from(tx_id_bytes),
            raw_proto: raw_proto.into(),
        })
    }

    /// The transaction id — `sha256` of the encoded `Transaction.raw`.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Apply fee, memo, permission, and optional TAPOS overrides from a filled
    /// [`TransactionRequest`] to this raw transaction.
    ///
    /// When any field is set, the `Transaction.raw` proto bytes are decoded,
    /// modified, and re-encoded; the `tx_id` (`sha256` of the new raw bytes) is
    /// recomputed so that the signature covers the updated payload.
    pub(crate) fn apply_request_fields(
        &mut self,
        request: &TransactionRequest,
    ) -> Result<(), crate::error::TransportErrorKind> {
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
            if let Some(pid) = request.permission_id
                && let Some(contract) = raw_data.contract.first_mut()
            {
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

            // Keep the public metadata in sync with the protobuf payload that
            // is signed and broadcast.
            self.timestamp = raw_data.timestamp;
            self.expiration = raw_data.expiration;

            // Recompute tx_id = sha256(encoded raw_data)
            let new_tx_id_bytes: [u8; 32] = Sha256::digest(raw_data.encode_to_vec()).into();
            self.tx_id = TxId::from(new_tx_id_bytes);
        } else {
            return Err(crate::error::TransportErrorKind::Malformed(
                "missing raw_data in Transaction".into(),
            ));
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
            // raw_proto is always valid — it is constructed from a node response
            // or re-encoded internally. A decode failure indicates a logic bug.
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

    use super::*;

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
        let mut raw =
            RawTransaction::from_proto_extention(vec![0; 32], tx.encode_to_vec(), 2, 1).unwrap();

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
