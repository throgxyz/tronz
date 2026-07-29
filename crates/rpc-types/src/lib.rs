#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
extern crate tracing;

pub mod types;

mod error;
pub use error::ResponseError;
pub use types::*;

// The domain <-> protobuf mapping. Public but hidden: it is how a transport
// speaks to a node, not something to build on. It lives here because both sides
// of the mapping do, and because the domain types are `#[non_exhaustive]` —
// nothing outside this crate can construct them.
#[cfg(feature = "test-utils")]
pub mod test_utils;

#[doc(hidden)]
pub mod codec;
#[doc(hidden)]
pub mod light_block;

/// The generated `protocol` protobuf messages.
///
/// Public so the transport crates can speak the wire format, not because it is
/// the API to build on: it tracks the TRON protobuf schema, which changes
/// outside this crate's control and without regard for semver. Work with the
/// domain types in [`types`] instead.
#[doc(hidden)]
#[allow(missing_docs, dead_code, unused_imports, clippy::all, clippy::pedantic)]
pub mod proto {
    include!("generated/protocol.rs");
}
