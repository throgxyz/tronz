//! TRON smart-contract event log type.

use crate::{Address, B256, Bytes};

/// An EVM-style event log emitted during contract execution.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Log {
    /// Emitting contract address.
    pub address: Address,
    topics: Vec<B256>,
    /// Non-indexed data.
    pub data: Bytes,
}

impl Log {
    /// Construct a log without checking the topic count.
    ///
    /// Decoders use this to round-trip node responses even when malformed.
    pub fn new_unchecked(address: Address, topics: Vec<B256>, data: impl Into<Bytes>) -> Self {
        Self { address, topics, data: data.into() }
    }

    /// Construct a log, returning `None` if it carries more than four topics.
    pub fn new(address: Address, topics: Vec<B256>, data: impl Into<Bytes>) -> Option<Self> {
        let log = Self::new_unchecked(address, topics, data);
        log.is_valid().then_some(log)
    }

    /// Returns whether this log has at most four topics, as required by the
    /// EVM-compatible event log format.
    pub fn is_valid(&self) -> bool {
        self.topics.len() <= 4
    }

    /// The indexed topics (topic0 = event signature hash).
    pub fn topics(&self) -> &[B256] {
        &self.topics
    }

    /// The indexed topics, mutably. Grants access to the existing entries
    /// without allowing the list to grow past the topic limit.
    pub fn topics_mut(&mut self) -> &mut [B256] {
        &mut self.topics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_from_shared_primitive_fields() {
        let address = Address::from_evm_bytes([0x11; 20]);
        let topic = B256::from([0x22; 32]);
        let log = Log::new(address, vec![topic], b"payload".to_vec()).unwrap();

        assert_eq!(log.address, address);
        assert_eq!(log.topics(), [topic]);
        assert_eq!(log.data.as_ref(), b"payload");
        assert!(log.is_valid());
    }

    #[test]
    fn rejects_too_many_topics_but_unchecked_preserves_them() {
        let topics = vec![B256::ZERO; 5];
        assert!(Log::new(Address::ZERO, topics.clone(), Bytes::new()).is_none());

        let log = Log::new_unchecked(Address::ZERO, topics.clone(), Bytes::new());
        assert!(!log.is_valid());
        assert_eq!(log.topics(), topics);
    }
}
