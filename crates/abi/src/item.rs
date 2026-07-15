use crate::TronAbiParam;

/// A TRON ABI entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TronAbiEntry {
    /// The entry category.
    pub entry_type: TronAbiEntryType,
    /// The function, event, or error name. Empty for singleton entries.
    pub name: String,
    /// The input parameters. May be empty.
    pub inputs: Vec<TronAbiParam>,
    /// The output parameters. May be empty.
    pub outputs: Vec<TronAbiParam>,
    /// Whether an event omits its signature from topic 0.
    pub anonymous: bool,
    /// Legacy flag indicating that a function does not modify state.
    pub constant: bool,
    /// Legacy flag indicating that an entry accepts TRX.
    pub payable: bool,
    /// The state mutability stored by the node.
    ///
    /// When this is [`TronAbiStateMutability::Unknown`], the legacy
    /// [`constant`](Self::constant) and [`payable`](Self::payable) flags may
    /// provide the only mutability information.
    pub state_mutability: TronAbiStateMutability,
}

impl TronAbiEntry {
    /// Returns the entry category.
    #[inline]
    pub const fn entry_type(&self) -> TronAbiEntryType {
        self.entry_type
    }

    /// Returns the entry name, or `None` when the stored name is empty.
    #[inline]
    pub fn name(&self) -> Option<&str> {
        (!self.name.is_empty()).then_some(self.name.as_str())
    }

    /// Returns the input parameters.
    #[inline]
    pub fn inputs(&self) -> &[TronAbiParam] {
        &self.inputs
    }

    /// Returns the output parameters.
    #[inline]
    pub fn outputs(&self) -> &[TronAbiParam] {
        &self.outputs
    }
}

/// A TRON ABI entry category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TronAbiEntryType {
    /// The protobuf `UnknownEntryType` value, or an unrecognized future value.
    Unknown(i32),
    /// A contract constructor.
    Constructor,
    /// A callable function.
    Function,
    /// An emitted event.
    Event,
    /// A fallback function.
    Fallback,
    /// A plain-TRX receive function.
    Receive,
    /// A Solidity custom error.
    Error,
}

impl TronAbiEntryType {
    /// Converts a protobuf numeric value without discarding unknown variants.
    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Constructor,
            2 => Self::Function,
            3 => Self::Event,
            4 => Self::Fallback,
            5 => Self::Receive,
            6 => Self::Error,
            other => Self::Unknown(other),
        }
    }

    /// Returns the protobuf numeric representation.
    #[inline]
    pub const fn as_i32(self) -> i32 {
        match self {
            Self::Unknown(value) => value,
            Self::Constructor => 1,
            Self::Function => 2,
            Self::Event => 3,
            Self::Fallback => 4,
            Self::Receive => 5,
            Self::Error => 6,
        }
    }

    /// Returns the ABI type string for a known entry category.
    #[inline]
    pub const fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Unknown(_) => None,
            Self::Constructor => Some("constructor"),
            Self::Function => Some("function"),
            Self::Event => Some("event"),
            Self::Fallback => Some("fallback"),
            Self::Receive => Some("receive"),
            Self::Error => Some("error"),
        }
    }
}

impl Default for TronAbiEntryType {
    #[inline]
    fn default() -> Self {
        Self::Unknown(0)
    }
}

/// State mutability stored in TRON ABI metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TronAbiStateMutability {
    /// The protobuf `UnknownMutabilityType` value, or an unrecognized future value.
    Unknown(i32),
    /// Pure functions promise not to read from or modify state.
    Pure,
    /// View functions promise not to modify state.
    View,
    /// Nonpayable functions do not accept TRX.
    NonPayable,
    /// Payable functions may accept TRX.
    Payable,
}

impl TronAbiStateMutability {
    /// Converts a protobuf numeric value without discarding unknown variants.
    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Pure,
            2 => Self::View,
            3 => Self::NonPayable,
            4 => Self::Payable,
            other => Self::Unknown(other),
        }
    }

    /// Returns the protobuf numeric representation.
    #[inline]
    pub const fn as_i32(self) -> i32 {
        match self {
            Self::Unknown(value) => value,
            Self::Pure => 1,
            Self::View => 2,
            Self::NonPayable => 3,
            Self::Payable => 4,
        }
    }

    /// Returns the Solidity string for a known state-mutability value.
    #[inline]
    pub const fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Unknown(_) => None,
            Self::Pure => Some("pure"),
            Self::View => Some("view"),
            Self::NonPayable => Some("nonpayable"),
            Self::Payable => Some("payable"),
        }
    }
}

impl Default for TronAbiStateMutability {
    #[inline]
    fn default() -> Self {
        Self::Unknown(0)
    }
}
