# Run cargo check, failing on warnings
check:
  cargo check --all-targets --all-features
  cargo clippy

# Run unit tests
test:
  cargo test

# Run network integration tests
integration-test:
  cargo test --features network-integration
