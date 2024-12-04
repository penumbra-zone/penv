# Run unit tests
test:
  cargo test

# Run network integration tests
integration-test:
  cargo test --features network-integration
