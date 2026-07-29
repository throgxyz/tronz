//! Wire-compatible protobuf views used by block-summary RPCs.
//!
//! TRON's block endpoints return every transaction in the block, while the
//! public provider methods in this crate return only [`BlockInfo`]. Omitting
//! protobuf field `1` from these views lets prost skip the transaction payload
//! without allocating or decoding nested messages.

use prost::Message;
use tronz_primitives::{B256, Bytes};

use crate::{error::ResponseError, types::BlockInfo};

#[derive(Clone, PartialEq, Message)]
pub struct BlockSummaryProto {
    #[prost(message, optional, tag = "2")]
    block_header: Option<BlockHeaderSummaryProto>,
    #[prost(bytes = "bytes", tag = "3")]
    block_id: prost::bytes::Bytes,
}

#[derive(Clone, PartialEq, Message)]
struct BlockHeaderSummaryProto {
    #[prost(message, optional, tag = "1")]
    raw_data: Option<BlockHeaderRawSummaryProto>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct BlockHeaderRawSummaryProto {
    #[prost(int64, tag = "1")]
    timestamp: i64,
    #[prost(int64, tag = "7")]
    number: i64,
}

#[derive(Clone, PartialEq, Message)]
pub struct BlockSummaryListProto {
    #[prost(message, repeated, tag = "1")]
    pub blocks: Vec<BlockSummaryProto>,
}

impl BlockSummaryProto {
    /// A block the caller asked for by height or hash, absent if the chain has no
    /// such block.
    ///
    /// TRON answers a lookup that matched nothing with an entirely empty message
    /// rather than an error. Only that counts as absent: an answer carrying a block
    /// id but no header is a broken one, not a missing block.
    pub fn into_block_lookup(
        self,
        fallback_hash: Option<B256>,
    ) -> Result<Option<BlockInfo>, ResponseError> {
        if self.block_header.is_none() {
            if self.block_id.is_empty() {
                return Ok(None);
            }

            return Err(ResponseError::Malformed("block has an id but no block_header".into()));
        }

        self.into_block_info(fallback_hash).map(Some)
    }

    pub fn into_block_info(self, fallback_hash: Option<B256>) -> Result<BlockInfo, ResponseError> {
        let header = self
            .block_header
            .ok_or_else(|| ResponseError::Malformed("missing block_header".into()))?;
        let raw = header
            .raw_data
            .ok_or_else(|| ResponseError::Malformed("missing block_header.raw_data".into()))?;

        let hash = if self.block_id.is_empty() {
            fallback_hash.ok_or_else(|| ResponseError::Malformed("missing blockid".into()))?
        } else {
            let bytes = Bytes::from(self.block_id);
            let block_id: [u8; 32] = bytes
                .as_ref()
                .try_into()
                .map_err(|_| ResponseError::Malformed("blockid must be 32 bytes".into()))?;
            B256::from(block_id)
        };

        Ok(BlockInfo { number: raw.number, hash, timestamp: raw.timestamp })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto;

    fn header(number: i64, timestamp: i64) -> proto::BlockHeader {
        proto::BlockHeader {
            raw_data: Some(proto::block_header::Raw {
                number,
                timestamp,
                tx_trie_root: vec![1; 32],
                parent_hash: vec![2; 32],
                witness_address: vec![3; 21],
                ..Default::default()
            }),
            witness_signature: vec![4; 65],
        }
    }

    #[test]
    fn decodes_extension_without_materializing_transactions() {
        let full = proto::BlockExtention {
            transactions: vec![proto::TransactionExtention {
                txid: vec![5; 32],
                constant_result: vec![vec![6; 1024].into()],
                ..Default::default()
            }],
            block_header: Some(header(42, 1234)),
            blockid: vec![7; 32],
        };

        let light = BlockSummaryProto::decode(full.encode_to_vec().as_slice()).unwrap();
        let info = light.into_block_info(None).unwrap();

        assert_eq!(info.number, 42);
        assert_eq!(info.timestamp, 1234);
        assert_eq!(info.hash, B256::from([7; 32]));
    }

    #[test]
    fn plain_block_uses_requested_hash() {
        let full = proto::Block {
            transactions: vec![proto::Transaction::default()],
            block_header: Some(header(9, 5678)),
        };
        let expected_hash = B256::from([8; 32]);

        let light = BlockSummaryProto::decode(full.encode_to_vec().as_slice()).unwrap();
        let info = light.into_block_info(Some(expected_hash)).unwrap();

        assert_eq!(info.number, 9);
        assert_eq!(info.timestamp, 5678);
        assert_eq!(info.hash, expected_hash);
    }
    #[test]
    fn replay_now_block() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fixtures/now_block.bin");
        let Ok(bytes) = std::fs::read(path) else { return };
        let light = BlockSummaryProto::decode(bytes.as_slice()).unwrap();
        let info = light.into_block_info(None).unwrap();
        assert!(info.number > 0, "captured block should have a positive height");
        assert_ne!(info.hash, B256::ZERO);
        assert!(info.timestamp > 0);
    }

    #[test]
    fn an_empty_block_message_means_the_chain_has_no_such_block() {
        let empty = BlockSummaryProto::default();

        assert!(empty.into_block_lookup(None).unwrap().is_none());
    }

    #[test]
    fn a_block_with_an_id_but_no_header_is_broken_not_missing() {
        let partial = BlockSummaryProto { block_header: None, block_id: vec![7; 32].into() };

        assert!(matches!(partial.into_block_lookup(None), Err(ResponseError::Malformed(_))));
    }
}
