#!/usr/bin/env bash
# Host CI: format, lint, test (PRD §32).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
