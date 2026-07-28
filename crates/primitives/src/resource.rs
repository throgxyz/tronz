//! TRON stakeable resource types.

use serde::{Deserialize, Serialize};

use crate::error::UnknownResourceCode;

/// The kind of network resource obtained by staking TRX.
///
/// Discriminants match the protobuf `ResourceCode` enum so the value can be
/// used directly when building contract parameters.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[repr(i32)]
pub enum ResourceCode {
    /// Network bandwidth (free + staked).
    Bandwidth = 0,
    /// Energy, consumed when executing smart contracts.
    Energy = 1,
    /// TRON Power, the voting weight obtained from staking.
    TronPower = 2,
}

impl ResourceCode {
    /// The protobuf discriminant for this resource.
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Convert from a protobuf discriminant, returning `None` if unknown.
    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(ResourceCode::Bandwidth),
            1 => Some(ResourceCode::Energy),
            2 => Some(ResourceCode::TronPower),
            _ => None,
        }
    }
}

impl From<ResourceCode> for i32 {
    fn from(resource: ResourceCode) -> Self {
        resource.as_i32()
    }
}

impl TryFrom<i32> for ResourceCode {
    type Error = UnknownResourceCode;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_i32(value).ok_or(UnknownResourceCode(value))
    }
}

impl Default for ResourceCode {
    /// Energy is the most commonly staked-for resource and matches the default
    /// used by the staking builders.
    fn default() -> Self {
        ResourceCode::Energy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminants() {
        assert_eq!(ResourceCode::Bandwidth.as_i32(), 0);
        assert_eq!(ResourceCode::Energy.as_i32(), 1);
        assert_eq!(ResourceCode::TronPower.as_i32(), 2);
        assert_eq!(ResourceCode::from_i32(1), Some(ResourceCode::Energy));
        assert_eq!(ResourceCode::from_i32(9), None);
        assert_eq!(ResourceCode::default(), ResourceCode::Energy);
        assert_eq!(i32::from(ResourceCode::TronPower), 2);
        assert_eq!(ResourceCode::try_from(2).unwrap(), ResourceCode::TronPower);
        assert!(ResourceCode::try_from(9).is_err());
    }
}
