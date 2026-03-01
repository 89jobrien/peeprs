set shell := ["bash", "-cu"]

default:
  just --list

# Format code
fmt:
  cargo fmt

# Check formatting without writing changes
fmt-check:
  cargo fmt -- --check

# Fast compile check
check:
  cargo check

# Clippy with warnings treated as errors
lint:
  cargo clippy --all-targets -- -D warnings

# Run all tests
test:
  cargo test

# Run all tests with nextest (requires cargo-nextest)
test-nextest:
  cargo nextest run

# Run a single test (usage: just test-one test_name)
test-one name:
  cargo test {{name}}

# Run an exact test path (usage: just test-exact module::path::test_name)
test-exact path:
  cargo test {{path}} -- --exact

# Run a single test with stdout/stderr (usage: just test-one-verbose test_name)
test-one-verbose name:
  cargo test {{name}} -- --nocapture

# Run a single exact test with nextest (usage: just test-nextest-one module::path::test_name)
test-nextest-one name:
  cargo nextest run -E "test(=\"{{name}}\")"

# Run an integration test target file (usage: just test-integration api_summary)
test-integration name:
  cargo test --test {{name}}

# Run the dashboard server
run:
  cargo run -- --root ~/logs/claude --host 127.0.0.1 --port 8765

# Dev hot reload server (requires cargo-watch)
dev:
  cargo watch -x 'run -- --root ~/logs/claude --host 127.0.0.1 --port 8765'

# Dev hot reload compile check (requires cargo-watch)
dev-check:
  cargo watch -x check

# Install common dev tools
tools:
  cargo install cargo-watch cargo-nextest

# Show CLI help
help:
  cargo run -- --help

# Full local validation sequence
check-all:
  cargo fmt -- --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  cargo run -- --help
