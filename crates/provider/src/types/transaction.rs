//! Transaction request / raw / signed types.

use tronz_primitives::{RecoverableSignature, Trx, TxId};

use crate::types::contract::ContractType;

/// Builder-stage transaction: all fields optional, filled progressively by
/// fillers before being finalized into a [`RawTransaction`].
#[derive(Clone, Debug, Default)]
pub struct TransactionRequest {
    /// The contract (operation) being performed.
    pub contract: Option<ContractType>,
    /// Maximum fee (energy + bandwidth) the sender will pay.
    pub fee_limit: Option<Trx>,
    /// Optional memo / note (`raw.data`).
    pub memo: Option<Vec<u8>>,
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
    pub fn with_memo(mut self, memo: impl Into<Vec<u8>>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Set the permission id for multisig transactions.
    pub fn with_permission_id(mut self, id: i32) -> Self {
        self.permission_id = Some(id);
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
    pub(crate) raw_proto: Vec<u8>,
}

impl RawTransaction {
    /// Construct from a `TransactionExtention` returned by the node.
    pub(crate) fn from_proto_extention(
        txid: Vec<u8>,
        raw_proto: Vec<u8>,
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
            raw_proto,
        })
    }

    /// The transaction id — `sha256` of the encoded `Transaction.raw`.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }

    /// Apply `fee_limit`, `memo`, and `permission_id` from a filled
    /// [`TransactionRequest`] to this raw transaction.
    ///
    /// When any field is set, the `Transaction.raw` proto bytes are decoded,
    /// modified, and re-encoded; the `tx_id` (`sha256` of the new raw bytes) is
    /// recomputed so that the signature covers the updated payload.
    pub(crate) fn apply_request_fields(
        &mut self,
        fee_limit_sun: Option<i64>,
        memo: Option<&[u8]>,
        permission_id: Option<i32>,
    ) -> Result<(), crate::error::TransportErrorKind> {
        use prost::Message as _;
        use sha2::{Digest, Sha256};

        if fee_limit_sun.is_none() && memo.is_none() && permission_id.is_none() {
            return Ok(());
        }

        let mut tx = crate::proto::Transaction::decode(self.raw_proto.as_ref())?;

        if let Some(ref mut raw_data) = tx.raw_data {
            if let Some(fl) = fee_limit_sun {
                raw_data.fee_limit = fl;
            }
            if let Some(m) = memo {
                raw_data.data = m.to_vec();
            }
            if let Some(pid) = permission_id {
                if let Some(contract) = raw_data.contract.first_mut() {
                    contract.permission_id = pid;
                }
            }

            // Recompute tx_id = sha256(encoded raw_data)
            let new_tx_id_bytes: [u8; 32] = Sha256::digest(raw_data.encode_to_vec()).into();
            self.tx_id = TxId::from(new_tx_id_bytes);
        } else {
            return Err(crate::error::TransportErrorKind::Malformed(
                "missing raw_data in Transaction".into(),
            ));
        }

        self.raw_proto = tx.encode_to_vec();
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
