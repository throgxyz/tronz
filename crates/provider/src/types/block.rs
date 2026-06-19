//! Block types and TAPOS extraction helpers.

use tronz_primitives::B256;

/// Summary information about a block, including the bits needed for TAPOS.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct BlockInfo {
    /// Block height.
    pub number: i64,
    /// Block id / hash.
    pub hash: B256,
    /// Block timestamp (unix ms).
    pub timestamp: i64,
}

impl BlockInfo {
    /// `ref_block_bytes` = last 2 bytes of the big-endian block number.
    pub fn ref_block_bytes(&self) -> [u8; 2] {
        let bytes = self.number.to_be_bytes();
        [bytes[6], bytes[7]]
    }

    /// `ref_block_hash` = bytes 8..16 of the block id.
    ///
    /// The block id is itself `sha256(block_header.raw_data)`, which is already
    /// captured in [`BlockInfo::hash`], so we slice it directly.
    pub fn ref_block_hash(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out.copy_from_slice(&self.hash.as_slice()[8..16]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tapos_extraction() {
        let block = BlockInfo {
            number: 0x0011_2233_4455_6677,
            hash: B256::from([
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31,
            ]),
            timestamp: 0,
        };
        assert_eq!(block.ref_block_bytes(), [0x66, 0x77]);
        assert_eq!(block.ref_block_hash(), [8, 9, 10, 11, 12, 13, 14, 15]);
    }
}
