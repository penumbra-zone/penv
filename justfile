# Run cargo check, failing on warnings
check:
  # check, failing on warnings
  RUSTFLAGS="-D warnings" cargo check --all-targets --all-features --target-dir=target/check
  # fmt dry-run, failing on any suggestions
  cargo fmt --all -- --check
  # clippy doesn't pass yet
  # cargo clippy

# Run unit tests
test:
  cargo nextest run

# Run network integration tests
integration:
  cargo nextest run --features network-integration
