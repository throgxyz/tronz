use std::{slice, vec};

use crate::{TronAbiEntry, TronAbiEntryType};

/// TRON's native contract ABI metadata.
///
/// Entries remain in node-provided order, including unknown entry kinds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TronAbi {
    /// The ABI entries in node-provided order.
    pub entries: Vec<TronAbiEntry>,
}

impl TronAbi {
    /// Creates an empty TRON ABI.
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

    /// Returns an iterator over the entries of one category, in node order.
    #[inline]
    pub fn items_of(
        &self,
        entry_type: TronAbiEntryType,
    ) -> impl Iterator<Item = &TronAbiEntry> + '_ {
        self.entries.iter().filter(move |entry| entry.entry_type == entry_type)
    }

    /// Returns an iterator over the function entries, in node order.
    #[inline]
    pub fn functions(&self) -> impl Iterator<Item = &TronAbiEntry> + '_ {
        self.items_of(TronAbiEntryType::Function)
    }

    /// Returns an iterator over the event entries, in node order.
    #[inline]
    pub fn events(&self) -> impl Iterator<Item = &TronAbiEntry> + '_ {
        self.items_of(TronAbiEntryType::Event)
    }

    /// Returns an iterator over the custom-error entries, in node order.
    #[inline]
    pub fn errors(&self) -> impl Iterator<Item = &TronAbiEntry> + '_ {
        self.items_of(TronAbiEntryType::Error)
    }

    /// Returns the constructor entry, if the ABI declares one.
    #[inline]
    pub fn constructor(&self) -> Option<&TronAbiEntry> {
        self.items_of(TronAbiEntryType::Constructor).next()
    }

    /// Returns an iterator over the functions with this name, in node order.
    ///
    /// A name may match more than one overloaded function.
    #[inline]
    pub fn functions_by_name<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a TronAbiEntry> + 'a {
        self.named(TronAbiEntryType::Function, name)
    }

    /// Returns an iterator over the events with this name, in node order.
    #[inline]
    pub fn events_by_name<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a TronAbiEntry> + 'a {
        self.named(TronAbiEntryType::Event, name)
    }

    /// Returns an iterator over the custom errors with this name, in node order.
    #[inline]
    pub fn errors_by_name<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a TronAbiEntry> + 'a {
        self.named(TronAbiEntryType::Error, name)
    }

    fn named<'a>(
        &'a self,
        entry_type: TronAbiEntryType,
        name: &'a str,
    ) -> impl Iterator<Item = &'a TronAbiEntry> + 'a {
        self.items_of(entry_type).filter(move |entry| entry.name == name)
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
    use crate::{TronAbiParam, TronAbiStateMutability};

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

    fn named(entry_type: TronAbiEntryType, name: &str, inputs: &[&str]) -> TronAbiEntry {
        TronAbiEntry {
            entry_type,
            name: name.into(),
            inputs: inputs.iter().map(|ty| TronAbiParam::new("", *ty)).collect(),
            ..Default::default()
        }
    }

    fn sample_abi() -> TronAbi {
        [
            named(TronAbiEntryType::Constructor, "", &["uint256"]),
            named(TronAbiEntryType::Function, "transfer", &["address", "uint256"]),
            named(TronAbiEntryType::Function, "transfer", &["address"]),
            named(TronAbiEntryType::Event, "Transfer", &["address", "address", "uint256"]),
            named(TronAbiEntryType::Error, "InsufficientBalance", &["uint256"]),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn category_iterators_only_yield_their_own_kind() {
        let abi = sample_abi();

        assert_eq!(abi.functions().count(), 2);
        assert_eq!(abi.events().count(), 1);
        assert_eq!(abi.errors().count(), 1);
        assert_eq!(abi.constructor().unwrap().inputs[0].ty, "uint256");
    }

    #[test]
    fn lookup_by_name_yields_every_overload_in_node_order() {
        let abi = sample_abi();

        let signatures: Vec<_> =
            abi.functions_by_name("transfer").filter_map(TronAbiEntry::signature).collect();
        assert_eq!(signatures, ["transfer(address,uint256)", "transfer(address)"]);

        assert_eq!(
            abi.events_by_name("Transfer").next().unwrap().signature().as_deref(),
            Some("Transfer(address,address,uint256)")
        );
        assert_eq!(
            abi.errors_by_name("InsufficientBalance").next().unwrap().signature().as_deref(),
            Some("InsufficientBalance(uint256)")
        );
    }

    #[test]
    fn lookup_does_not_cross_categories() {
        let abi = sample_abi();

        // `Transfer` is an event, `transfer` a function; neither leaks into the other.
        assert_eq!(abi.functions_by_name("Transfer").count(), 0);
        assert_eq!(abi.events_by_name("transfer").count(), 0);
        assert_eq!(abi.errors_by_name("transfer").count(), 0);
    }

    #[test]
    fn only_functions_events_and_errors_have_a_signature() {
        assert!(sample_abi().constructor().unwrap().signature().is_none());

        // A node may name any entry; the category still decides.
        let named_constructor = named(TronAbiEntryType::Constructor, "constructor", &["uint256"]);
        assert!(named_constructor.signature().is_none());

        let named_unknown = named(TronAbiEntryType::Unknown(7), "mystery", &["uint256"]);
        assert!(named_unknown.signature().is_none());
    }
}
