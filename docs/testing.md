# Testing And Validation

## Local Checks

The main local checks are:

```bash
cargo check --workspace --all-features
cargo check --workspace --examples --all-features
cargo test --workspace --doc
cargo test --workspace --all-features --doc
```

The `Makefile` also includes `fmt`, `clippy`, `test`, `docs`, `typos`, `deny`,
and feature-powerset targets.

## Examples

Read-only examples can be run against Nile without credentials:

```bash
cargo run -p examples-queries --example query
cargo run -p examples-queries --example list_witnesses
cargo run -p examples-trc10 --example trc10_query
```

Write examples require a funded Nile key:

```bash
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx
```

## Live Integration Tests

Provider integration tests are ignored by default because they hit the live Nile
network:

```bash
cargo test -p tronz-provider --test integration -- --ignored
```

Write-path integration tests additionally require:

```bash
export TRON_TEST_KEY=<funded-nile-private-key>
```
