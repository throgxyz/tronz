# Contributing to tronz

Thank you for your interest in contributing!

## Getting started

1. Fork the repository and clone it locally.
2. Make sure you have a recent stable Rust toolchain (`rustup update stable`).
3. Build and test everything:

```bash
cargo build --workspace
cargo test  --workspace
```

### TRE integration tests

CI runs the SDK end-to-end tests against a disposable TronBox Runtime Environment
(TRE) private chain. To run the same check locally:

```bash
docker run --detach --rm --name tronz-tre \
  --publish 50051:50051 --publish 50052:50052 \
  tronbox/tre@sha256:f4332e11df12a9f360639a4546fd046593909630fda48af00b30410c144342f0

# Run both readiness checks after java-tron starts.
cargo test -p tronz --no-default-features \
  --features provider-grpc,contract,signer-local \
  --test local_node full_node_is_ready -- --ignored --exact

cargo test -p tronz --no-default-features \
  --features provider-grpc,contract,signer-local \
  --test local_node solidity_node_is_ready -- --ignored --exact

cargo test -p tronz --no-default-features \
  --features provider-grpc,contract,signer-local \
  --test local_node -- --ignored --nocapture --test-threads=1

docker stop tronz-tre
```

The fixture private keys belong only to TRE's deterministic private chain and
must never be used on a public network.

## Code style

We use `rustfmt` with the configuration in [`rustfmt.toml`](./rustfmt.toml):

```bash
cargo fmt --all
```

Linting via Clippy:

```bash
cargo clippy --workspace --all-features -- -D warnings
```

## Commit messages

We follow [Conventional Commits](https://www.conventionalcommits.org/). Examples:

- `feat(provider): add get_block_by_hash`
- `fix(primitives): correct Address checksum encoding`
- `chore: bump alloy-primitives to 1.1`

## Pull requests

- One logical change per PR.
- Add or update tests for any new behavior.
- Update `CHANGELOG.md` under `[Unreleased]` with a brief description.
- PRs are squash-merged; the PR title becomes the commit message.

## Releasing

`release-plz` creates a release PR after changes land on `main`. Merging that PR
publishes every workspace crate, then creates one `vX.Y.Z` tag and GitHub
release.

One-time repository setup:

1. Enable **Allow GitHub Actions to create and approve pull requests**.
2. Create a `crates-io` GitHub environment.
3. For every published `tronz-*` crate, configure a crates.io trusted publisher
   for `throgxyz/tronz`, workflow `release-plz.yml`, environment `crates-io`.
4. Add a `RELEASE_PLZ_TOKEN` secret from a repository-scoped GitHub App or
   fine-grained PAT so release PRs trigger the normal CI workflow. Grant only
   Contents and Pull requests read/write permissions.

## License

By contributing, you agree that your contributions will be dual-licensed under
[MIT](./LICENSE-MIT) and [Apache-2.0](./LICENSE-APACHE), matching the project license.
