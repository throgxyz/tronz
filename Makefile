.PHONY: ci fmt clippy test doctest docs examples typos deny features

# Run all CI checks in the same order as .github/workflows/ci.yml
ci: fmt clippy test doctest docs examples typos deny features

fmt:
	cargo +nightly fmt --all --check

clippy:
	RUSTFLAGS="-Dwarnings" cargo clippy --workspace --all-targets --all-features

test:
	cargo nextest run --workspace

doctest:
	cargo test --workspace --doc
	cargo test --all-features --workspace --doc

docs:
	RUSTDOCFLAGS="--cfg docsrs -D warnings -Zunstable-options --show-type-layout --generate-link-to-definition" \
		cargo +nightly doc --workspace --all-features --no-deps --document-private-items

examples:
	cargo check --workspace --examples --all-features

typos:
	typos

deny:
	cargo deny check

features:
	cargo hack check --feature-powerset --depth 1
