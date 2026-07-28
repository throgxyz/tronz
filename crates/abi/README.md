# tronz-abi

Native TRON smart-contract ABI metadata types for the
[tronz](https://github.com/throgxyz/tronz) SDK.

`TronAbi` mirrors the entry model returned by TRON nodes without exposing
generated protobuf types. It preserves entry order and unknown enum values.

```rust
use tronz_abi::{TronAbi, TronAbiEntry, TronAbiEntryType, TronAbiParam};

let abi: TronAbi = [TronAbiEntry {
    entry_type: TronAbiEntryType::Function,
    name: "transfer".into(),
    inputs: vec![TronAbiParam::new("to", "address"), TronAbiParam::new("amount", "uint256")],
    ..Default::default()
}]
.into_iter()
.collect();

let transfer = abi
    .functions_by_name("transfer")
    .find(|entry| entry.signature().as_deref() == Some("transfer(address,uint256)"))
    .unwrap();
assert_eq!(transfer.inputs().len(), 2);
```

The `alloy` feature enables conversion to and from Alloy's `JsonAbi`:

```rust
# #[cfg(feature = "alloy")]
# fn example() -> Result<(), Box<dyn std::error::Error>> {
use tronz_abi::{JsonAbi, TronAbi};

let json_abi = JsonAbi::new();
let tron_abi = TronAbi::try_from(&json_abi)?;
let json_abi = JsonAbi::try_from(&tron_abi)?;
# Ok(())
# }
```

Tuple component names, `internalType`, and top-level order cannot round-trip
through `JsonAbi`. The `serde` feature serializes the native model; its JSON
shape is not Solidity JSON ABI.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
