/// Conversion failures between TRON contract metadata and Alloy's JSON ABI.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TronAbiConversionError {
    /// An ABI parameter contains an invalid Solidity type.
    #[error("invalid ABI type `{0}")]
    InvalidType(String),
    /// An ABI item or parameter contains an invalid Solidity identifier.
    #[error("invalid ABI identifier `{0}")]
    InvalidName(String),
    /// A TRON ABI entry type cannot be represented as a JSON ABI item.
    #[error("unknown TRON ABI entry type {0}")]
    UnknownEntryType(i32),
    /// A TRON state-mutability value cannot be represented by Alloy.
    #[error("invalid TRON ABI state mutability {0}")]
    InvalidMutability(i32),
    /// Legacy mutability flags contain both `constant` and `payable`.
    #[error("TRON ABI entry cannot be both constant and payable")]
    ConflictingMutability,
    /// A tuple parameter does not contain enough information to recover its components.
    #[error("tuple ABI parameter `{0}` does not include component types")]
    IncompleteTuple(String),
    /// An ABI contains more than one singleton item such as a constructor.
    #[error("duplicate ABI {0} entry")]
    DuplicateEntry(&'static str),
}
