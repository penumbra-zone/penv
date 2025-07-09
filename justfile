# Run cargo check, failing on warnings
check:
  cargo check --all-targets --all-features
  cargo clippy

# Run unit tests
test:
  cargo nextest run

# Run network integration tests
integration:
  cargo nextest run --features network-integration
