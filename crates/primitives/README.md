# tronz-primitives

Leaf primitive types for the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

This crate performs **no** network I/O and contains **no** protobuf code.
It only defines the small, widely-shared value types that every other crate in
the workspace depends on.

## Types

| Type | Description |
|------|-------------|
| [`Address`] | 21-byte TRON address (`0x41` prefix + 20-byte body); parses base58check (`T…`) and hex |
| [`Trx`] | Amount denominated in sun (`1 TRX = 1_000_000 sun`); wraps `i64` to match protobuf |
| [`ResourceCode`] | `Bandwidth`, `Energy`, or `TronPower` — the stakeable network resources |
| [`RecoverableSignature`] | 65-byte `r ‖ s ‖ v` secp256k1 signature with embedded recovery id |
| [`Log`] | Smart-contract event log containing an emitter address, indexed topics, and data |

Common byte/arithmetic types (`U256`, `B256`, `Bytes`, `keccak256`) are
re-exported from [`alloy_primitives`] so the rest of the workspace has a single
import surface.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
