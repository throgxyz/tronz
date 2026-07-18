//! Event log decoding for TRON smart contracts.
//!
//! # Static decoding
//!
//! When you have a `sol!`-generated event type, use the free functions:
//!
//! ```ignore
//! use alloy_sol_types::sol;
//! use tronz_contract::event::{decode_log, decode_logs, log_matches};
//!
//! sol! {
//!     event Transfer(address indexed from, address indexed to, uint256 value);
//! }
//!
//! // Check whether a log matches before decoding
//! if log_matches::<Transfer>(&log) {
//!     let Transfer { from, to, value } = decode_log::<Transfer>(&log)?;
//! }
//!
//! // Or decode all matching logs from a receipt in one pass
//! for transfer in decode_logs::<Transfer>(&receipt.logs) {
//!     let Transfer { from, to, value } = transfer?;
//! }
//! ```
//!
//! # Dynamic decoding
//!
//! When the ABI is only known at runtime, use [`Interface::decode_log`].

#[cfg(feature = "provider")]
use {
    crate::error::Result,
    alloy_sol_types::SolEvent,
    tronz_primitives::{B256, Log},
};

/// Decode a single log into a typed [`SolEvent`].
///
/// Returns an error if the log does not match the event signature or if
/// the ABI decoding fails.
#[cfg(feature = "provider")]
pub fn decode_log<E: SolEvent>(log: &Log) -> Result<E> {
    E::decode_raw_log(log.topics.iter().copied(), &log.data).map_err(Into::into)
}

/// Return `true` if `log` could be an instance of `E`.
///
/// For non-anonymous events this checks `topic[0] == E::SIGNATURE_HASH`.
/// Anonymous events always match (no discriminating topic).
#[cfg(feature = "provider")]
pub fn log_matches<E: SolEvent>(log: &Log) -> bool {
    if E::ANONYMOUS {
        return true;
    }
    log.topics.first().is_some_and(|t| *t == E::SIGNATURE_HASH)
}

/// Return an iterator that yields only the logs matching `E`, decoded.
///
/// Logs that do not match the event signature are silently skipped.
/// Logs that match but fail to decode yield an `Err`.
#[cfg(feature = "provider")]
pub fn decode_logs<'a, E: SolEvent + 'a>(logs: &'a [Log]) -> impl Iterator<Item = Result<E>> + 'a {
    logs.iter().filter(|log| log_matches::<E>(log)).map(decode_log::<E>)
}

/// Return all topic-0 hashes present in the log slice (deduplicated).
///
/// Useful for routing logs to the right decoder without decoding them all.
#[cfg(feature = "provider")]
pub fn topic0_set(logs: &[Log]) -> impl Iterator<Item = B256> + '_ {
    let mut seen = std::collections::HashSet::new();
    logs.iter().filter_map(move |log| log.topics.first().copied().filter(|t| seen.insert(*t)))
}
