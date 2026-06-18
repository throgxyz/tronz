# tronz Documentation

This directory is the guide layer for the tronz workspace. The crate-level
READMEs stay short; these pages explain how the pieces fit together and link to
runnable examples.

## Start Here

- [Getting started](getting-started.md) - install tronz, connect to Nile, and run the first query.
- [Providers](providers.md) - build read-only and signed providers, choose endpoints, and use API keys.
- [Transactions](transactions.md) - build, fill, sign, broadcast, and await receipts.
- [Signers](signers.md) - local keys, BIP-39 mnemonics, and Web3 keystores.
- [Contracts](contracts.md) - TRC20 bindings, dynamic ABI calls, deployment, logs, and reverts.
- [TRC10](trc10.md) - native TRON tokens and asset issue workflows.
- [Staking](staking.md) - Stake 2.0, legacy Stake 1.0, delegation, and unfreezing.
- [Governance and witnesses](governance-witness.md) - proposals, super representatives, and voting.
- [Local nodes](local-node.md) - plain HTTP/2 gRPC endpoints and feature flags.
- [Testing and examples](testing.md) - local CI, live-network tests, and example validation.
- [Design notes](design.md) - workspace architecture and alloy-inspired boundaries.

## Examples

The `examples/*` packages are part of this workspace and use local path
dependencies, so they compile against the current checkout:

```bash
cargo run -p examples-queries --example query
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx
```

See [examples/README.md](../examples/README.md) for the full catalog.
