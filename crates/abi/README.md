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

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
