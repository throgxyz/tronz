//! Generic [`ContractInstance`] — a provider-bound handle to any TRON smart contract.

use alloy_dyn_abi::DynSolValue;
use alloy_json_abi::JsonAbi;
use alloy_primitives::Selector;
use tronz_primitives::{Address, Bytes};
use tronz_provider::TronProvider;

use crate::{call::CallBuilder, deploy::DeployBuilder, error::Result, interface::Interface};

/// A handle to a TRON smart contract at a specific address.
///
/// Supports both **dynamic ABI** calls (via an [`Interface`] loaded from JSON) and
/// **raw calldata** calls (for use by static wrappers like [`Trc20Instance`]).
///
/// Construct via [`ContractExt::contract`] on any provider, or directly with
/// [`ContractInstance::new`].
///
/// [`Trc20Instance`]: crate::trc20::Trc20Instance
#[derive(Clone)]
pub struct ContractInstance<P> {
    address: Address,
    provider: P,
    interface: Interface,
}

impl<P> ContractInstance<P> {
    /// Create a contract instance with a dynamic ABI [`Interface`].
    #[inline]
    pub fn new(address: Address, provider: P, interface: Interface) -> Self {
        Self { address, provider, interface }
    }

    /// The contract address.
    #[inline]
    pub fn address(&self) -> Address {
        self.address
    }

    /// Set the contract address in place.
    #[inline]
    pub fn set_address(&mut self, address: Address) {
        self.address = address;
    }

    /// Return a new instance pointing at a different address (same provider and ABI).
    #[inline]
    pub fn at(mut self, address: Address) -> Self {
        self.set_address(address);
        self
    }

    /// The underlying [`JsonAbi`].
    #[inline]
    pub fn abi(&self) -> &JsonAbi {
        self.interface.abi()
    }

    /// Borrow the underlying provider.
    #[inline]
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Borrow the ABI interface.
    #[inline]
    pub fn interface(&self) -> &Interface {
        &self.interface
    }
}

impl<P> std::ops::Deref for ContractInstance<P> {
    type Target = Interface;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.interface
    }
}

impl<P> std::fmt::Debug for ContractInstance<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractInstance").field("address", &self.address).finish()
    }
}

impl<P: TronProvider> ContractInstance<P> {
    /// Create a contract instance without an ABI — only raw calldata calls are available.
    ///
    /// Used internally by static-ABI wrappers like [`Trc20Instance`].
    ///
    /// [`Trc20Instance`]: crate::trc20::Trc20Instance
    #[inline]
    pub fn new_raw(provider: P, address: Address) -> Self {
        Self { address, provider, interface: Interface::empty() }
    }

    // ── raw calldata ──────────────────────────────────────────────────────────

    /// Create a [`CallBuilder`] with pre-encoded `data`.
    ///
    /// Choose `.call().await` for simulation or `.send().await` to broadcast.
    /// Chain `.value(trx)` for payable calls.
    #[inline]
    pub fn call_raw(&self, data: Bytes) -> CallBuilder<P> {
        CallBuilder::new(self.provider.clone(), self.address, data)
    }

    // ── dynamic ABI ───────────────────────────────────────────────────────────

    /// Create a [`CallBuilder`] for the function named `fn_name` with `args`.
    ///
    /// Returns an error if the function is not found in the ABI.
    pub fn function(&self, fn_name: &str, args: &[DynSolValue]) -> Result<CallBuilder<P>> {
        let data = self.encode_input(fn_name, args)?;
        Ok(CallBuilder::new(self.provider.clone(), self.address, data))
    }

    /// Create a [`CallBuilder`] for the function with the given `selector`.
    ///
    /// Returns an error if the selector is not found in the ABI.
    pub fn function_from_selector(
        &self,
        selector: &Selector,
        args: &[DynSolValue],
    ) -> Result<CallBuilder<P>> {
        let data = self.encode_input_with_selector(selector, args)?;
        Ok(CallBuilder::new(self.provider.clone(), self.address, data))
    }

    // ── convenience (dynamic call + immediate decode) ─────────────────────────

    /// Simulate a call by function name and return decoded output values.
    pub async fn call(&self, fn_name: &str, args: &[DynSolValue]) -> Result<Vec<DynSolValue>> {
        let output = self.function(fn_name, args)?.call().await?;
        self.decode_output(fn_name, &output)
    }

    /// Simulate a call by selector and return decoded output values.
    pub async fn call_with_selector(
        &self,
        selector: &Selector,
        args: &[DynSolValue],
    ) -> Result<Vec<DynSolValue>> {
        let output = self.function_from_selector(selector, args)?.call().await?;
        self.decode_output_with_selector(selector, &output)
    }
}

// ── Extension trait ───────────────────────────────────────────────────────────

/// Convenience methods on any [`TronProvider`] for creating contract handles.
pub trait ContractExt: TronProvider + Sized {
    /// Bind to the contract at `address` with a dynamic ABI [`Interface`].
    fn contract(&self, address: Address, interface: Interface) -> ContractInstance<Self> {
        ContractInstance::new(address, self.clone(), interface)
    }

    /// Start building a smart-contract deployment.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use tronz_contract::ContractExt as _;
    /// # async fn run(provider: impl tronz_provider::TronProvider, bytecode: tronz_primitives::Bytes) -> tronz_contract::Result<()> {
    /// let pending = provider
    ///     .deploy(bytecode)
    ///     .abi(b"[]")
    ///     .name("MyToken")
    ///     .send()
    ///     .await?;
    /// # Ok(()) }
    /// ```
    fn deploy(&self, bytecode: impl Into<tronz_primitives::Bytes>) -> DeployBuilder<Self> {
        DeployBuilder::new(self.clone(), bytecode)
    }
}

impl<P: TronProvider> ContractExt for P {}
