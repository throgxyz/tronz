# tronz-sol-macro

The `tron_sol!` procedural macro for the
[tronz](https://github.com/throgxyz/tronz) TRON SDK — a TRON-aware superset of
Alloy's `sol!`.

`tron_sol!` forwards its entire input to `alloy_sol_types::sol!` to generate the
Solidity type layer (`…Call` structs, events, errors, custom types,
free-standing `struct`/`enum`/`type` definitions, …) and *additionally*
generates a provider-bound `Instance` for every `contract`/`interface` carrying
`#[sol(rpc)]`, wired to tronz's `ContractReadProvider`.

Use it through the facade as `tronz::tron_sol!`, or as
`tronz_contract::tron_sol!`.

## Inline Solidity

The inline form accepts multiple items in one invocation (several contracts, or
contracts mixed with bare `struct`/`enum`/`error`/`event`/`type` definitions)
and passes attributes through: any attribute other than the TRON-specific ones
below is forwarded verbatim to `sol!`, so `#[derive(…)]`, `#[sol(all_derives)]`,
`#[sol(extra_derives(…))]`, and doc comments all apply to the generated type
layer. Inheritance is not flattened, matching `sol!`: base members stay in the
base contract's module.

```rust,ignore
// Type layer only, same as `sol!` — bare types and multiple items are fine.
tron_sol! {
    struct Foo { uint256 x; }
    enum Bar { A, B }
}

// Type layer plus TRON RPC bindings, with derive passthrough.
tron_sol! {
    #[derive(Debug)]
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address owner) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

let contract = IERC20::new(usdt_addr, provider).caller(owner);
let balance = contract.balanceOf(owner).call().await?;
```

## JSON ABI

A JSON ABI file path or inline JSON is accepted in the same `abigen`-style form
that Alloy's `sol!` supports. Both raw ABI arrays `[...]` and Forge artifacts
`{"abi":[...]}` work, and file paths are tracked so that editing the ABI
re-expands the macro.

```rust,ignore
tron_sol! {
    #[sol(rpc)]
    MyContract, "abi/MyContract.json"
}
```

## TRON-specific attributes

| Attribute | Effect |
|---|---|
| `#[sol(rpc)]` | Also generate a `ContractReadProvider`-bound `Instance` |
| `#[sol(bytecode = "0x…")]` | Embed creation bytecode as `BYTECODE`; with `#[sol(rpc)]`, also generate `deploy_builder` / `deploy` |
| `#[sol(deployed_bytecode = "0x…")]` | Embed the runtime bytecode as `DEPLOYED_BYTECODE` |
| `#[tron_sol(tronz_crate = <path>)]` | Override the runtime crate path (defaults to `::tronz::contract`) |

## Limits of the generated instance layer

The type layer is whatever `sol!` produces. Generating instance methods on top
of it needs to name each parameter's Rust type, which rules out two cases:

- `mapping` parameters, which `sol!` rejects here as well;
- array sizes that are not integer literals, because a Solidity `constant` cannot be evaluated
  during macro expansion.

`#[sol(rename)]` and `#[sol(rename_all)]` are also rejected together with
`#[sol(rpc)]`, because renaming the generated `…Call` types would desync the
instance methods.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
