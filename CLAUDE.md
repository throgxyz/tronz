# tronz - Claude Code Context

## Project Overview

`tronz` is an async-first Rust SDK for the TRON blockchain, inspired by
alloy's workspace layering and builder/filler ergonomics.

- Status: v0.1.2, active development, not yet production-ready.
- Rust edition: 2024.
- MSRV: 1.90.
- License: MIT OR Apache-2.0.
- Repo: https://github.com/throgxyz/tronz.

## Workspace Layout

```text
tronz/
├── Cargo.toml                  # Workspace root (crates/* + examples/*)
├── docs/                       # User guides and design notes
├── examples/                   # Runnable example packages
├── crates/
│   ├── primitives/             # Address, Trx, ResourceCode, signatures
│   ├── signer/                 # TronSigner, LocalSigner, mnemonic, keystore
│   ├── provider/               # Domain model, gRPC transport, fillers, builders
│   ├── contract/               # TRC20 bindings and dynamic ABI helpers
│   └── tronz/                  # Umbrella re-export crate
└── .github/workflows/ci.yml
```

## Dependency Graph

```text
tronz-primitives
      ^        ^
tronz-signer   tronz-provider
                    ^
              tronz-contract
                    ^
                  tronz
```

## Key Design Decisions

1. Async traits use stable RPITIT instead of `async_trait`.
2. The transport is TRON gRPC via `tonic` and `prost`, not Ethereum JSON-RPC.
3. Generated protobuf types stay private under `tronz-provider`; public APIs use domain types.
4. `ProviderBuilder` composes `TaposFiller`, `FeeLimitFiller`, and `SignerFiller` via `JoinFill`.
5. Native TRON transactions are built locally; smart-contract transactions are built by the node, then signed locally.
6. TRC10, witness, and governance operations are exposed through extension traits.
7. ABI work reuses alloy crates: `alloy-sol-types`, `alloy-sol-macro`, `alloy-dyn-abi`, and `alloy-json-abi`.

See `docs/design.md` for the user-facing architecture notes.

## Feature Flags

```toml
# tronz umbrella
default = ["provider-grpc-tls", "contract", "signer-local"]
full    = ["default"]
provider-grpc-tls = ["tronz-provider/grpc-tls"]
provider-grpc     = ["tronz-provider/grpc"]
contract          = ["dep:tronz-contract"]
signer-local      = []
signer-mnemonic   = ["tronz-signer/mnemonic"]
signer-keystore   = ["tronz-signer/keystore"]

# tronz-provider
default  = ["grpc-tls"]
grpc-tls = ["tonic/tls-native-roots"]
grpc     = []
```

## Examples

There are 42 examples in workspace packages under `examples/*`.

```bash
# Read-only
cargo run -p examples-queries --example query
cargo run -p examples-queries --example list_witnesses

# Write paths on Nile
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx
TRON_PRIVATE_KEY=<hex> cargo run -p examples-staking --example stake
TRON_PRIVATE_KEY=<hex> cargo run -p examples-contracts --example contract_deploy
```

Use `examples/README.md` for the full catalog and environment variables.

## Local Checks

```bash
make ci
make fmt
make clippy
make test
make doctest
make docs
make typos
make deny
make features
```

Example-specific compile check:

```bash
cargo check --workspace --examples --all-features
```

Required tools:

```bash
cargo install cargo-nextest typos-cli cargo-deny cargo-hack
rustup toolchain install 1.90.0
rustup toolchain install nightly
```

## Reference Codebases

Prefer local references when available:

| Repo | Path | Use for |
| --- | --- | --- |
| alloy | `/Users/denniszhou/throgxyz/alloy` | Provider/filler/builder patterns, CI setup |
| gotron-sdk | `/Users/denniszhou/throgxyz/gotron-sdk` | TRON gRPC API coverage, proto definitions |
| tronic | `/Users/denniszhou/throgxyz/tronic` | Another TRON Rust SDK reference |

## Git Commit Style

Do not add AI attribution footers to commit messages.
