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
- Add or update tests for any new behaviour.
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
[MIT](./LICENSE-MIT) and [Apache-2.0](./LICENSE-APACHE), matching the project licence.
