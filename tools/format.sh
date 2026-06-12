#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

CARGO_BIN="${CARGO:-cargo}"
if ! command -v "${CARGO_BIN}" >/dev/null 2>&1; then
  CARGO_BIN="${HOME}/.cargo/bin/cargo"
fi

"${CARGO_BIN}" fmt --all
