//! Errors raised while reading a node's answers.

/// A node response that cannot be turned into a usable result.
///
/// Transports fold these into their own error type. The three cases stay apart
/// because they call for different reactions: a decode failure names the wire
/// format, a malformed response names what was missing from an otherwise readable
/// message, and a node error is the node itself declining — nothing was wrong
/// with reading it.
#[derive(Debug, thiserror::Error)]
pub enum ResponseError {
    /// The bytes are not a valid protobuf message.
    #[error(transparent)]
    Decode(#[from] prost::DecodeError),

    /// The message decoded, but is not one the domain model can represent: a
    /// field that should have been there, an address of the wrong length, an id
    /// that disagrees with the payload it claims to identify.
    #[error("{0}")]
    Malformed(String),

    /// The node answered `Return { result: false }` — it understood the request
    /// and refused it.
    #[error("{0}")]
    NodeError(String),
}
