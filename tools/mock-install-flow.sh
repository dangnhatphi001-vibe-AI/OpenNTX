#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

app_id="${2:-example-app}"

CARGO_BIN="${CARGO:-cargo}"
if ! command -v "${CARGO_BIN}" >/dev/null 2>&1; then
  CARGO_BIN="${HOME}/.cargo/bin/cargo"
fi

cleanup_input=""
if [[ $# -ge 1 ]]; then
  input="$1"
else
  input="$(mktemp --suffix=-setup.exe)"
  cleanup_input="${input}"
  python3 - "${input}" <<'PY'
import sys

path = sys.argv[1]
data = bytearray(512)
data[0:2] = b"MZ"
data[0x3c:0x40] = (0x80).to_bytes(4, "little")
data[0x80:0x84] = b"PE\0\0"
data[0x84:0x86] = (0x8664).to_bytes(2, "little")
data[0x94:0x96] = (0x00f0).to_bytes(2, "little")
data[0x96:0x98] = (0x0002).to_bytes(2, "little")
data[0x98:0x9a] = (0x020b).to_bytes(2, "little")
data[0x98 + 68:0x98 + 70] = (2).to_bytes(2, "little")
with open(path, "wb") as handle:
    handle.write(data)
PY
fi

trap 'if [[ -n "${cleanup_input}" ]]; then rm -f "${cleanup_input}"; fi' EXIT

echo "OpenNTX mock install flow"
echo "Input: ${input}"
echo
echo "+ openntx analyze ${input}"
"${CARGO_BIN}" run -q -p openntx-cli -- analyze "${input}" || true
echo
echo "+ openntx install ${input}"
"${CARGO_BIN}" run -q -p openntx-cli -- install "${input}" || true
echo
echo "+ openntx package ${app_id}"
"${CARGO_BIN}" run -q -p openntx-cli -- package "${app_id}"
echo
echo "Status: demonstration only. Runtime execution and installer capture are not implemented in V0.1."
