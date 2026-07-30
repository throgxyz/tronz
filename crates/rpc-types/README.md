# tronz-rpc-types

TRON's domain model: the types a node's answers decode into, and the requests
that go out to it. Also the protobuf messages behind them, because on TRON the
two are inseparable — a transaction *is* its protobuf encoding, and its id is a
hash of those bytes.

Depend on this crate to name TRON's data without pulling in a transport.
[`tronz-provider`] re-exports everything here as `tronz_provider::types`, so
there is no need to add it alongside a provider.

```rust
use tronz_rpc_types::TransactionRequest;
use tronz_primitives::parse_trx;

let fee_limit = parse_trx("20")?;
let request = TransactionRequest::default().with_fee_limit(fee_limit);

assert_eq!(request.fee_limit, Some(fee_limit));
# Ok::<_, tronz_primitives::AmountError>(())
```

## What holds this crate together

`RawTransaction` is the reason the protobuf messages live here rather than in a
transport crate. It carries the encoded `Transaction` and its id, and the id is
never assigned — it is derived from the encoding, every time, including after a
filler changes a fee limit or a permission. A node that returns an id which
disagrees with the transaction it built is rejected rather than signed. Keeping
the type and the code that computes the id in one crate is what makes that
guarantee hold; the fields are private so nothing else can break it. Reading one
goes through `details`, which decodes the encoding it holds on demand, so
inspecting a transaction costs nothing on the path that only sends it.

## What is public but not an API

Three modules are public and hidden from the docs, listed together under `spi`:

| Module | Holds |
| --- | --- |
| `proto` | the generated `protocol` messages |
| `codec` | the domain ↔ protobuf mapping, some 53 functions |
| `light_block` | protobuf views that decode a block without its transactions |

They are public because a transport in another crate has to reach them, and they
are here rather than in a transport crate because the domain types are
`#[non_exhaustive]` — only this crate can construct one, so the mapping onto them
has to live alongside them.

None of the three follows semver. `proto` tracks the TRON protobuf schema, which
changes outside this crate's control; the other two change whenever the mapping
does. Use the domain types and a provider.

The schema in `proto/` is the single copy in the workspace. Both it and the
service clients generated from it are committed, by `cargo xtask codegen`, so no
crate here needs `protoc` to build.

## Feature flags

| Feature | Default | Effect |
| --- | --- | --- |
| `test-utils` | no | Builders for the node-response types, which are `#[non_exhaustive]` and so cannot be constructed outside this crate. For tests in dependent crates; not for production code. |

[`tronz-provider`]: https://docs.rs/tronz-provider
