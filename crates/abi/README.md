# tronz-abi

Native TRON smart-contract ABI metadata types for the
[tronz](https://github.com/throgxyz/tronz) SDK.

TRON nodes store contract ABI metadata in a protobuf-specific entry model,
which is not identical to the Solidity JSON ABI format. This crate exposes that
model without leaking generated protobuf types:

- `TronAbi`
- `TronAbiEntry`
- `TronAbiParam`
- `TronAbiEntryType`
- `TronAbiStateMutability`

## Serialization formats

The `serde` feature serializes the native `TronAbi` data model. Its JSON shape
is intended for persisting TRON metadata and is **not** the standard Solidity
JSON ABI array format. Parse a standard JSON ABI as Alloy's `JsonAbi`, then
convert it explicitly:

```rust,ignore
use tronz_abi::{JsonAbi, TronAbi};

let json_abi: JsonAbi = serde_json::from_str(ABI_JSON)?;
let tron_abi = TronAbi::try_from(&json_abi)?;
```

Enable the `alloy` feature to convert between `TronAbi` and Alloy's `JsonAbi`:

```rust,ignore
use tronz_abi::{JsonAbi, TronAbi};

let json_abi = JsonAbi::new();
let tron_abi = TronAbi::try_from(&json_abi)?;
let json_abi = JsonAbi::try_from(&tron_abi)?;
```

The equivalent `try_from_json_abi` and `try_to_json_abi` convenience methods
are also available. The complete `alloy-json-abi` API is re-exported as
`tronz_abi::json_abi`, so applications can use matching `Param`, `Function`,
and `Event` types without adding another direct dependency.

`JsonAbi` to `TronAbi` conversion preserves canonical tuple component types,
but cannot preserve component names or `internalType` values. Converting node
metadata in the other direction fails when it contains a bare `tuple` without
recoverable component types. `JsonAbi` also groups items by kind and name, so
top-level entry order is not preserved across conversion. The native protobuf
conversion preserves the metadata and ordering returned by the node.

Converting `TronAbi` to `JsonAbi` normalizes each entry to the fields supported
by its Solidity JSON ABI item kind. TRON metadata fields that have no meaning
for that item kind, such as outputs on an event or `anonymous` on a function,
are ignored. Conversion still fails when a value cannot be represented safely,
including unknown entry kinds, invalid identifiers or types, bare tuples, and
duplicate singleton entries.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
