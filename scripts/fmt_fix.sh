#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

echo "Updating Rust toolchain..."
rustup update

if [[ "${1:-}" == "--check" ]]; then
    cargo fmt --all -- --check
else
    cargo fmt --all
fi

