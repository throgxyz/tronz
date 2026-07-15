/// A TRON ABI input or output parameter.
///
/// TRON nodes store only the name, Solidity type string, and event indexing
/// flag. Solidity JSON ABI `components` and `internalType` metadata are not
/// available in this representation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TronAbiParam {
    /// Whether an event parameter is stored in a log topic.
    pub indexed: bool,
    /// The parameter name, which may be empty.
    pub name: String,
    /// The Solidity type string exactly as stored by the TRON node.
    pub ty: String,
}

impl TronAbiParam {
    /// Creates a non-indexed ABI parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use tronz_abi::TronAbiParam;
    ///
    /// let owner = TronAbiParam::new("owner", "address");
    /// assert_eq!(owner.name(), "owner");
    /// assert_eq!(owner.ty(), "address");
    /// assert!(!owner.is_indexed());
    ///
    /// let indexed_owner = owner.with_indexed(true);
    /// assert!(indexed_owner.is_indexed());
    /// ```
    #[inline]
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self { indexed: false, name: name.into(), ty: ty.into() }
    }

    /// Sets whether this parameter is indexed in an event.
    #[inline]
    pub const fn with_indexed(mut self, indexed: bool) -> Self {
        self.indexed = indexed;
        self
    }

    /// Returns the parameter name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the Solidity type string stored by the node.
    #[inline]
    pub fn ty(&self) -> &str {
        &self.ty
    }

    /// Returns whether this parameter is indexed in an event.
    #[inline]
    pub const fn is_indexed(&self) -> bool {
        self.indexed
    }
}
