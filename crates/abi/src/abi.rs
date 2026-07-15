use std::{slice, vec};

use crate::TronAbiEntry;

/// TRON's native contract ABI metadata.
///
/// Unlike a Solidity JSON ABI, this representation mirrors the information a
/// TRON node stores and returns without exposing generated protobuf types.
/// Entries remain in node-provided order, including unknown entry kinds.
///
/// # Examples
///
/// ```
/// use tronz_abi::{
///     TronAbi, TronAbiEntry, TronAbiEntryType, TronAbiParam, TronAbiStateMutability,
/// };
///
/// let transfer = TronAbiEntry {
///     entry_type: TronAbiEntryType::Function,
///     name: "transfer".into(),
///     inputs: vec![TronAbiParam::new("to", "address"), TronAbiParam::new("amount", "uint256")],
///     outputs: vec![TronAbiParam::new("", "bool")],
///     state_mutability: TronAbiStateMutability::NonPayable,
///     ..Default::default()
/// };
///
/// let abi: TronAbi = [transfer].into_iter().collect();
/// assert_eq!(abi.len(), 1);
/// assert_eq!(abi.items().next().unwrap().name(), Some("transfer"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TronAbi {
    /// The ABI entries in node-provided order.
    pub entries: Vec<TronAbiEntry>,
}

impl TronAbi {
    /// Creates an empty TRON ABI.
    ///
    /// # Examples
    ///
    /// ```
    /// use tronz_abi::TronAbi;
    ///
    /// let abi = TronAbi::new();
    /// assert!(abi.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Returns the number of entries in the ABI.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the ABI contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns an iterator over the ABI entries.
    #[inline]
    pub fn items(&self) -> slice::Iter<'_, TronAbiEntry> {
        self.entries.iter()
    }

    /// Returns a mutable iterator over the ABI entries.
    #[inline]
    pub fn items_mut(&mut self) -> slice::IterMut<'_, TronAbiEntry> {
        self.entries.iter_mut()
    }

    /// Returns an iterator that takes ownership of the ABI entries.
    #[inline]
    pub fn into_items(self) -> vec::IntoIter<TronAbiEntry> {
        self.entries.into_iter()
    }
}

impl FromIterator<TronAbiEntry> for TronAbi {
    #[inline]
    fn from_iter<T: IntoIterator<Item = TronAbiEntry>>(iter: T) -> Self {
        Self { entries: iter.into_iter().collect() }
    }
}

impl IntoIterator for TronAbi {
    type Item = TronAbiEntry;
    type IntoIter = vec::IntoIter<TronAbiEntry>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.into_items()
    }
}

impl<'a> IntoIterator for &'a TronAbi {
    type Item = &'a TronAbiEntry;
    type IntoIter = slice::Iter<'a, TronAbiEntry>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.items()
    }
}

impl<'a> IntoIterator for &'a mut TronAbi {
    type Item = &'a mut TronAbiEntry;
    type IntoIter = slice::IterMut<'a, TronAbiEntry>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.items_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TronAbiEntryType, TronAbiStateMutability};

    #[test]
    fn collection_helpers_preserve_order() {
        let abi: TronAbi = [
            TronAbiEntry { entry_type: TronAbiEntryType::Constructor, ..Default::default() },
            TronAbiEntry { entry_type: TronAbiEntryType::Function, ..Default::default() },
        ]
        .into_iter()
        .collect();

        assert_eq!(abi.len(), 2);
        assert!(!abi.is_empty());
        assert_eq!(abi.items().nth(1).unwrap().entry_type, TronAbiEntryType::Function);
    }

    #[test]
    fn unknown_numeric_values_round_trip() {
        assert_eq!(TronAbiEntryType::from_i32(99).as_i32(), 99);
        assert_eq!(TronAbiStateMutability::from_i32(98).as_i32(), 98);
    }
}
