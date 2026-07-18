//! TRON smart-contract event log type.

use crate::{Address, B256, Bytes};

/// An EVM-style event log emitted during contract execution.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Log {
    /// Emitting contract address.
    pub address: Address,
    /// Indexed topics (topic0 = event signature hash).
    pub topics: Vec<B256>,
    /// Non-indexed data.
    pub data: Bytes,
}

impl Log {
    /// Construct a log from its three fields.
    pub fn new(address: Address, topics: Vec<B256>, data: impl Into<Bytes>) -> Self {
        Self { address, topics, data: data.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_from_shared_primitive_fields() {
        let address = Address::from_evm_bytes([0x11; 20]);
        let topic = B256::from([0x22; 32]);
        let log = Log::new(address, vec![topic], b"payload".to_vec());

        assert_eq!(log.address, address);
        assert_eq!(log.topics, vec![topic]);
        assert_eq!(log.data.as_ref(), b"payload");
    }
}
