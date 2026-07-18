//! [`Interface`] — a parsed JSON ABI with O(1) selector lookup.

use std::collections::HashMap;

use alloy_dyn_abi::{DecodedEvent, DynSolValue, EventExt as _, FunctionExt as _, JsonAbiExt as _};
use alloy_json_abi::{Event, Function, JsonAbi};
use alloy_primitives::{B256, Selector};
use tronz_primitives::{Address, Bytes, Log};

use crate::{
    error::{ContractError, Result},
    instance::ContractInstance,
};

/// A parsed JSON ABI used for dynamic function encoding and decoding.
///
/// Functions and events are indexed by their selectors on construction for O(1)
/// lookup.
///
/// Construct from a [`JsonAbi`]:
///
/// ```ignore
/// use alloy_json_abi::JsonAbi;
/// use tronz_contract::Interface;
///
/// let abi: JsonAbi = serde_json::from_str(r#"[...]"#).unwrap();
/// let interface = Interface::new(abi);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Interface {
    abi: JsonAbi,
    /// selector → (function_name, overload_index)
    functions: HashMap<Selector, (String, usize)>,
    /// topic0 → (event_name, overload_index)
    events: HashMap<B256, (String, usize)>,
}

impl Interface {
    /// Create an interface from a parsed [`JsonAbi`].
    ///
    /// All function selectors and event topic hashes are pre-computed and stored
    /// for O(1) lookup.
    pub fn new(abi: JsonAbi) -> Self {
        let functions = build_selector_map(&abi);
        let events = build_event_map(&abi);
        Self { abi, functions, events }
    }

    /// Create an empty interface (no functions, no events).
    ///
    /// Used by static-ABI wrappers like [`Trc20Instance`](crate::trc20::Trc20Instance) that
    /// encode calldata themselves and only need raw call/send infrastructure.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The underlying [`JsonAbi`].
    pub fn abi(&self) -> &JsonAbi {
        &self.abi
    }

    /// Consume the interface, returning the inner [`JsonAbi`].
    pub fn into_abi(self) -> JsonAbi {
        self.abi
    }

    // ── encode ────────────────────────────────────────────────────────────────

    /// ABI-encode the inputs for the function named `fn_name`.
    pub fn encode_input(&self, fn_name: &str, args: &[DynSolValue]) -> Result<Bytes> {
        Ok(self.get_by_name(fn_name)?.abi_encode_input(args)?.into())
    }

    /// ABI-encode the inputs for the function with the given `selector`.
    pub fn encode_input_with_selector(
        &self,
        selector: &Selector,
        args: &[DynSolValue],
    ) -> Result<Bytes> {
        Ok(self.get_by_selector(selector)?.abi_encode_input(args)?.into())
    }

    // ── decode input ──────────────────────────────────────────────────────────

    /// ABI-decode the calldata (without the 4-byte selector) for `fn_name`.
    pub fn decode_input(&self, fn_name: &str, data: &[u8]) -> Result<Vec<DynSolValue>> {
        Ok(self.get_by_name(fn_name)?.abi_decode_input(data)?)
    }

    /// ABI-decode the calldata for the function with the given `selector`.
    pub fn decode_input_with_selector(
        &self,
        selector: &Selector,
        data: &[u8],
    ) -> Result<Vec<DynSolValue>> {
        Ok(self.get_by_selector(selector)?.abi_decode_input(data)?)
    }

    // ── decode output ─────────────────────────────────────────────────────────

    /// ABI-decode the return data for `fn_name`.
    pub fn decode_output(&self, fn_name: &str, data: &[u8]) -> Result<Vec<DynSolValue>> {
        self.get_by_name(fn_name)?
            .abi_decode_output(data)
            .map_err(|e| ContractError::decode_err(fn_name, data, e))
    }

    /// ABI-decode the return data for the function with the given `selector`.
    pub fn decode_output_with_selector(
        &self,
        selector: &Selector,
        data: &[u8],
    ) -> Result<Vec<DynSolValue>> {
        let f = self.get_by_selector(selector)?;
        let name = f.name.as_str();
        f.abi_decode_output(data).map_err(|e| ContractError::decode_err(name, data, e))
    }

    // ── decode log ────────────────────────────────────────────────────────────

    /// Decode a [`Log`] dynamically using the ABI.
    ///
    /// The event is identified by `topic[0]` (the Keccak-256 hash of the event
    /// signature). Returns a [`DecodedEvent`] with separate `indexed` and `body`
    /// vectors, plus the matched event name.
    ///
    /// Returns [`ContractError::UnknownEvent`] if no matching event is found in
    /// the ABI.
    pub fn decode_log(&self, log: &Log) -> Result<(String, DecodedEvent)> {
        let topic0 = log.topics.first().copied().unwrap_or_default();
        let event = self.get_event_by_topic(&topic0)?;
        let name = event.name.clone();
        let decoded = event.decode_log_parts(log.topics.iter().copied(), &log.data)?;
        Ok((name, decoded))
    }

    /// Decode every log in `logs` that matches a known event in the ABI.
    ///
    /// Logs whose `topic[0]` is not in the ABI are silently skipped.
    /// Logs that match but fail to decode yield an `Err`.
    pub fn decode_logs<'a>(
        &'a self,
        logs: &'a [Log],
    ) -> impl Iterator<Item = Result<(String, DecodedEvent)>> + 'a {
        logs.iter().filter_map(move |log| {
            let topic0 = log.topics.first().copied()?;
            if !self.events.contains_key(&topic0) {
                return None;
            }
            Some(self.decode_log(log))
        })
    }

    // ── connect ───────────────────────────────────────────────────────────────

    /// Bind this interface to a contract at `address`, returning a [`ContractInstance`].
    ///
    /// Equivalent to `ContractInstance::new(address, provider, interface)`.
    pub fn connect<P>(self, address: Address, provider: P) -> ContractInstance<P>
    where
        P: tronz_provider::TronProvider,
    {
        ContractInstance::new(address, provider, self)
    }

    // ── internal ──────────────────────────────────────────────────────────────

    pub(crate) fn get_by_name(&self, name: &str) -> Result<&Function> {
        self.abi
            .function(name)
            .and_then(|fs| fs.first())
            .ok_or_else(|| ContractError::UnknownFunction(name.to_owned()))
    }

    pub(crate) fn get_by_selector(&self, selector: &Selector) -> Result<&Function> {
        self.functions
            .get(selector)
            .and_then(|(name, idx)| self.abi.functions.get(name)?.get(*idx))
            .ok_or_else(|| ContractError::UnknownSelector(*selector))
    }

    fn get_event_by_topic(&self, topic0: &B256) -> Result<&Event> {
        self.events
            .get(topic0)
            .and_then(|(name, idx)| self.abi.events.get(name)?.get(*idx))
            .ok_or_else(|| ContractError::UnknownEvent(*topic0))
    }
}

impl From<JsonAbi> for Interface {
    fn from(abi: JsonAbi) -> Self {
        Self::new(abi)
    }
}

/// Build a `selector → (name, overload_index)` map from a [`JsonAbi`].
fn build_selector_map(abi: &JsonAbi) -> HashMap<Selector, (String, usize)> {
    abi.functions
        .iter()
        .flat_map(|(name, overloads)| {
            overloads.iter().enumerate().map(move |(idx, f)| (f.selector(), (name.clone(), idx)))
        })
        .collect()
}

/// Build a `topic0 → (name, overload_index)` map from a [`JsonAbi`].
fn build_event_map(abi: &JsonAbi) -> HashMap<B256, (String, usize)> {
    abi.events
        .iter()
        .flat_map(|(name, overloads)| {
            overloads.iter().enumerate().filter_map(move |(idx, e)| {
                // anonymous events have no topic0 discriminator — skip them
                if e.anonymous {
                    return None;
                }
                Some((e.selector(), (name.clone(), idx)))
            })
        })
        .collect()
}
